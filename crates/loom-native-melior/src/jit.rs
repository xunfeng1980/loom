#[cfg(feature = "melior")]
use loom_core::arrow_buffer_lowering::{
    lower_arrow_raw_copy_to_standard_mlir, ArrowColumnBufferPlan, PrimitiveArrowType,
};
use loom_core::arrow_buffer_lowering::{
    plan_arrow_buffers_from_decode_dialect, reference_zeroed_value_bytes, ArrowTableBufferPlan,
};
use loom_core::full_verifier::FullVerificationReport;
use loom_core::l2_core::L2CoreProgram;
use loom_core::native_lowering::{
    execute_supported_copy_i32, LoweringDiagnosticCode, LoweringSupportReport,
};

use crate::backend::{
    NativeBackendCancellation, NativeBackendDiagnostic, NativeBackendDiagnosticCode,
    NativeBackendReport, NativeBackendStatus,
};
use crate::builder::{build_melior_module, MeliorModuleArtifact};
#[cfg(feature = "melior")]
use crate::pipeline::LLVM_LOWERING_PIPELINE;
use crate::pipeline::{validate_translation_to_llvm_ir, MlirValidationOptions};
use crate::report::{MeliorBackendDiagnosticCode, MeliorBackendReport, ENTRY_SYMBOL};
use crate::toolchain::probe_toolchain;

pub const PRODUCTION_JIT_ENTRY_SYMBOL: &str = "loom_decode_build_buffers";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductionJitOptions {
    pub require_compatible_toolchain: bool,
    pub input_value_buffers: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionJitOutput {
    pub entry_symbol: String,
    pub row_count: u64,
    pub column_count: usize,
    pub value_buffers: Vec<Vec<u8>>,
}

pub fn execute_prepared_production_jit(
    report: &NativeBackendReport,
    cancellation: &NativeBackendCancellation,
    options: ProductionJitOptions,
) -> Result<ProductionJitOutput, NativeBackendReport> {
    if cancellation.cancelled {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::Cancelled,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::Cancelled,
                "$.cancellation",
                cancellation
                    .reason
                    .clone()
                    .unwrap_or_else(|| "production JIT request was cancelled".to_string()),
            ),
        ));
    }

    if report.status != NativeBackendStatus::Accepted || !report.diagnostics.is_empty() {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.backend_report.status",
                "production JIT requires an accepted backend report",
            ),
        ));
    }

    let Some(artifact) = report.artifact.as_ref() else {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.backend_report.artifact",
                "production JIT requires a prepared backend artifact",
            ),
        ));
    };

    if artifact.entry_symbol.as_deref() != Some(PRODUCTION_JIT_ENTRY_SYMBOL) {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::JitSymbolMissing,
                "$.backend_report.artifact.entry_symbol",
                format!("JIT entry symbol '{PRODUCTION_JIT_ENTRY_SYMBOL}' was not found"),
            ),
        ));
    }

    let buffers = plan_arrow_buffers_from_decode_dialect(&artifact.lowering_facts);
    let Some(table) = buffers.table() else {
        let message = buffers
            .first_error()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "production JIT requires supported primitive buffers".to_string());
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::FailClosed,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                "$.backend_report.artifact.lowering_facts",
                message,
            ),
        ));
    };

    let toolchain = probe_toolchain();
    if !toolchain.compatible {
        let explicit_skip = std::env::var("LOOM_ALLOW_NATIVE_TOOL_SKIP")
            .map(|value| value == "1")
            .unwrap_or(false);
        let (status, code, message) = if explicit_skip && !options.require_compatible_toolchain {
            (
                NativeBackendStatus::SkippedToolchain,
                NativeBackendDiagnosticCode::ToolchainSkipped,
                "production JIT skipped by explicit LOOM_ALLOW_NATIVE_TOOL_SKIP=1",
            )
        } else {
            (
                NativeBackendStatus::FailClosed,
                NativeBackendDiagnosticCode::ToolchainFailed,
                "compatible MLIR/LLVM toolchain is required before production JIT execution",
            )
        };
        return Err(report_with_diagnostic(
            report,
            status,
            NativeBackendDiagnostic::new(code, "$.toolchain", message),
        ));
    }

    if cancellation.cancelled {
        return Err(report_with_diagnostic(
            report,
            NativeBackendStatus::Cancelled,
            NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::Cancelled,
                "$.cancellation",
                "production JIT request was cancelled before execution",
            ),
        ));
    }

    let value_buffers =
        execute_raw_copy_mlir(table, &options.input_value_buffers).map_err(|diagnostic| {
            report_with_diagnostic(report, NativeBackendStatus::FailClosed, diagnostic)
        })?;
    Ok(ProductionJitOutput {
        entry_symbol: PRODUCTION_JIT_ENTRY_SYMBOL.to_string(),
        row_count: table.row_count,
        column_count: table.columns.len(),
        value_buffers,
    })
}

pub fn compare_production_jit_output(
    report: &NativeBackendReport,
    expected: &[Vec<u8>],
    output: &ProductionJitOutput,
) -> Result<(), NativeBackendReport> {
    if expected == output.value_buffers.as_slice() {
        return Ok(());
    }

    Err(report_with_diagnostic(
        report,
        NativeBackendStatus::FailClosed,
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::NativeOutputMismatch,
            "$.jit.output",
            "production JIT output did not match interpreter/reference output",
        ),
    ))
}

fn execute_raw_copy_mlir(
    table: &ArrowTableBufferPlan,
    input_value_buffers: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, NativeBackendDiagnostic> {
    if input_value_buffers.is_empty() {
        return Ok(table
            .columns
            .iter()
            .map(reference_zeroed_value_bytes)
            .collect());
    }

    if input_value_buffers.len() != table.columns.len() {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.input_value_buffers",
            format!(
                "production JIT expected {} input value buffer(s), got {}",
                table.columns.len(),
                input_value_buffers.len()
            ),
        ));
    }
    for (idx, (column, buffer)) in table.columns.iter().zip(input_value_buffers).enumerate() {
        let expected_len = column.primitive.value_buffer_bytes as usize;
        if buffer.len() != expected_len {
            return Err(NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::InvalidBackendArtifact,
                format!("$.jit.input_value_buffers[{idx}]"),
                format!(
                    "production JIT input buffer has {} bytes, expected exactly {}",
                    buffer.len(),
                    expected_len
                ),
            ));
        }
    }

    execute_raw_copy_mlir_backend(table, input_value_buffers)
}

#[cfg(feature = "melior")]
fn execute_raw_copy_mlir_backend(
    table: &ArrowTableBufferPlan,
    input_value_buffers: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, NativeBackendDiagnostic> {
    use melior::dialect::DialectRegistry;
    use melior::ir::Module;
    use melior::pass;
    use melior::utility::{
        parse_pass_pipeline, register_all_dialects, register_all_llvm_translations,
        register_all_passes,
    };
    use melior::{Context, ExecutionEngine};

    let context = Context::new();
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    register_all_llvm_translations(&context);
    register_all_passes();

    let mlir_text = lower_arrow_raw_copy_to_standard_mlir(table).map_err(|report| {
        let message = report
            .first_error()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "production JIT raw-copy MLIR lowering failed".to_string());
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.mlir",
            message,
        )
    })?;
    let mut module = Module::parse(&context, &mlir_text).ok_or_else(|| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.mlir.parse",
            "production JIT failed to parse raw-copy MLIR module",
        )
    })?;

    let pass_manager = pass::PassManager::new(&context);
    parse_pass_pipeline(
        pass_manager.as_operation_pass_manager(),
        LLVM_LOWERING_PIPELINE,
    )
    .map_err(|err| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.mlir.pass_pipeline",
            format!("production JIT failed to parse LLVM lowering pipeline: {err:?}"),
        )
    })?;
    pass_manager.run(&mut module).map_err(|err| {
        NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitUnavailable,
            "$.jit.mlir.lower_to_llvm",
            format!("production JIT failed to lower MLIR module to LLVM: {err:?}"),
        )
    })?;

    let engine = ExecutionEngine::new(&module, 2, &[], false, false);
    if engine.lookup(PRODUCTION_JIT_ENTRY_SYMBOL).is_null() {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::JitSymbolMissing,
            "$.jit.symbol",
            format!("JIT entry symbol '{PRODUCTION_JIT_ENTRY_SYMBOL}' was not found"),
        ));
    }

    let mut columns = table
        .columns
        .iter()
        .zip(input_value_buffers)
        .map(|(column, input)| JitColumnStorage::new(column, input, table.row_count))
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = table.row_count as isize;
    let mut args = Vec::with_capacity(columns.len() * 2 + 1);
    for column in columns.iter_mut() {
        args.push(column.input_descriptor_ptr());
    }
    for column in columns.iter_mut() {
        args.push(column.output_descriptor_ptr());
    }
    args.push(&mut rows as *mut isize as *mut ());

    unsafe {
        engine
            .invoke_packed(PRODUCTION_JIT_ENTRY_SYMBOL, &mut args)
            .map_err(|err| {
                NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::JitUnavailable,
                    "$.jit.invoke",
                    format!("production JIT ExecutionEngine invocation failed: {err:?}"),
                )
            })?;
    }

    Ok(columns
        .into_iter()
        .map(JitColumnStorage::into_bytes)
        .collect())
}

#[cfg(not(feature = "melior"))]
fn execute_raw_copy_mlir_backend(
    _table: &ArrowTableBufferPlan,
    _input_value_buffers: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>, NativeBackendDiagnostic> {
    Err(NativeBackendDiagnostic::new(
        NativeBackendDiagnosticCode::JitUnavailable,
        "$.jit",
        "production JIT requires the loom-native-melior melior feature",
    ))
}

#[cfg(feature = "melior")]
#[repr(C)]
struct MemRef1D<T> {
    allocated: *mut T,
    aligned: *mut T,
    offset: isize,
    size0: isize,
    stride0: isize,
}

#[cfg(feature = "melior")]
impl<T> MemRef1D<T> {
    fn new(values: &mut [T]) -> Self {
        let ptr = values.as_mut_ptr();
        Self {
            allocated: ptr,
            aligned: ptr,
            offset: 0,
            size0: values.len() as isize,
            stride0: 1,
        }
    }
}

#[cfg(feature = "melior")]
enum JitColumnStorage {
    I32 {
        input: Vec<i32>,
        output: Vec<i32>,
        input_desc: MemRef1D<i32>,
        output_desc: MemRef1D<i32>,
        input_arg: *mut MemRef1D<i32>,
        output_arg: *mut MemRef1D<i32>,
    },
    I64 {
        input: Vec<i64>,
        output: Vec<i64>,
        input_desc: MemRef1D<i64>,
        output_desc: MemRef1D<i64>,
        input_arg: *mut MemRef1D<i64>,
        output_arg: *mut MemRef1D<i64>,
    },
    F32 {
        input: Vec<f32>,
        output: Vec<f32>,
        input_desc: MemRef1D<f32>,
        output_desc: MemRef1D<f32>,
        input_arg: *mut MemRef1D<f32>,
        output_arg: *mut MemRef1D<f32>,
    },
    F64 {
        input: Vec<f64>,
        output: Vec<f64>,
        input_desc: MemRef1D<f64>,
        output_desc: MemRef1D<f64>,
        input_arg: *mut MemRef1D<f64>,
        output_arg: *mut MemRef1D<f64>,
    },
}

#[cfg(feature = "melior")]
impl JitColumnStorage {
    fn new(
        column: &ArrowColumnBufferPlan,
        input_bytes: &[u8],
        row_count: u64,
    ) -> Result<Self, NativeBackendDiagnostic> {
        match column.primitive.primitive_type {
            PrimitiveArrowType::Int32 => {
                let mut input = bytes_to_i32(input_bytes)?;
                let mut output = vec![0i32; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::I32 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
            PrimitiveArrowType::Int64 => {
                let mut input = bytes_to_i64(input_bytes)?;
                let mut output = vec![0i64; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::I64 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
            PrimitiveArrowType::Float32 => {
                let mut input = bytes_to_f32(input_bytes)?;
                let mut output = vec![0f32; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::F32 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
            PrimitiveArrowType::Float64 => {
                let mut input = bytes_to_f64(input_bytes)?;
                let mut output = vec![0f64; row_count as usize];
                let input_desc = MemRef1D::new(&mut input);
                let output_desc = MemRef1D::new(&mut output);
                Ok(Self::F64 {
                    input,
                    output,
                    input_desc,
                    output_desc,
                    input_arg: std::ptr::null_mut(),
                    output_arg: std::ptr::null_mut(),
                })
            }
        }
    }

    fn input_descriptor_ptr(&mut self) -> *mut () {
        match self {
            Self::I32 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<i32>;
                input_arg as *mut *mut MemRef1D<i32> as *mut ()
            }
            Self::I64 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<i64>;
                input_arg as *mut *mut MemRef1D<i64> as *mut ()
            }
            Self::F32 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<f32>;
                input_arg as *mut *mut MemRef1D<f32> as *mut ()
            }
            Self::F64 {
                input_desc,
                input_arg,
                ..
            } => {
                *input_arg = input_desc as *mut MemRef1D<f64>;
                input_arg as *mut *mut MemRef1D<f64> as *mut ()
            }
        }
    }

    fn output_descriptor_ptr(&mut self) -> *mut () {
        match self {
            Self::I32 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<i32>;
                output_arg as *mut *mut MemRef1D<i32> as *mut ()
            }
            Self::I64 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<i64>;
                output_arg as *mut *mut MemRef1D<i64> as *mut ()
            }
            Self::F32 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<f32>;
                output_arg as *mut *mut MemRef1D<f32> as *mut ()
            }
            Self::F64 {
                output_desc,
                output_arg,
                ..
            } => {
                *output_arg = output_desc as *mut MemRef1D<f64>;
                output_arg as *mut *mut MemRef1D<f64> as *mut ()
            }
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::I32 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(i32::to_le_bytes).collect()
            }
            Self::I64 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(i64::to_le_bytes).collect()
            }
            Self::F32 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(f32::to_le_bytes).collect()
            }
            Self::F64 { input, output, .. } => {
                let _keepalive = input;
                output.into_iter().flat_map(f64::to_le_bytes).collect()
            }
        }
    }
}

#[cfg(feature = "melior")]
fn bytes_to_i32(bytes: &[u8]) -> Result<Vec<i32>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, i32::from_le_bytes, "Int32")
}

#[cfg(feature = "melior")]
fn bytes_to_i64(bytes: &[u8]) -> Result<Vec<i64>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, i64::from_le_bytes, "Int64")
}

#[cfg(feature = "melior")]
fn bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, f32::from_le_bytes, "Float32")
}

#[cfg(feature = "melior")]
fn bytes_to_f64(bytes: &[u8]) -> Result<Vec<f64>, NativeBackendDiagnostic> {
    chunks_to_values(bytes, f64::from_le_bytes, "Float64")
}

#[cfg(feature = "melior")]
fn chunks_to_values<const N: usize, T>(
    bytes: &[u8],
    convert: impl Fn([u8; N]) -> T,
    kind: &str,
) -> Result<Vec<T>, NativeBackendDiagnostic> {
    if bytes.len() % N != 0 {
        return Err(NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::InvalidBackendArtifact,
            "$.jit.input_value_buffers",
            format!(
                "{kind} input buffer length {} is not {N}-byte aligned",
                bytes.len()
            ),
        ));
    }
    Ok(bytes
        .chunks_exact(N)
        .map(|chunk| {
            let mut array = [0u8; N];
            array.copy_from_slice(chunk);
            convert(array)
        })
        .collect())
}

fn report_with_diagnostic(
    source: &NativeBackendReport,
    status: NativeBackendStatus,
    diagnostic: NativeBackendDiagnostic,
) -> NativeBackendReport {
    let mut diagnostics = source.diagnostics.clone();
    diagnostics.push(diagnostic);
    NativeBackendReport {
        status,
        diagnostics,
        runtime_plan: source.runtime_plan.clone(),
        runtime_cache_key: source.runtime_cache_key.clone(),
        backend_identity: source.backend_identity.clone(),
        artifact: None,
    }
}

pub fn execute_copy_i32_jit(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
    input: &[i32],
) -> Result<Vec<i32>, MeliorBackendReport> {
    let artifact = build_melior_module(program, report)?;
    let reference =
        execute_supported_copy_i32(program, report, input).map_err(map_lowering_report)?;

    let validation = validate_translation_to_llvm_ir(
        &artifact,
        MlirValidationOptions {
            require_compatible_toolchain: true,
        },
    );
    if !validation.is_ok() || !validation.supported {
        return Err(jit_unavailable(
            &artifact,
            Some(validation),
            "compatible MLIR/LLVM ExecutionEngine toolchain is unavailable",
        ));
    }

    execute_copy_i32_with_backend(&artifact, &reference)
}

pub fn compare_native_output(reference: &[i32], native: &[i32]) -> Result<(), MeliorBackendReport> {
    if reference == native {
        return Ok(());
    }

    Err(MeliorBackendReport::diagnostic(
        MeliorBackendDiagnosticCode::NativeOutputMismatch,
        "$.jit.output",
        format!(
            "native output mismatch: expected {:?}, got {:?}",
            reference, native
        ),
    ))
}

pub fn jit_symbol_missing(symbol: &str) -> MeliorBackendReport {
    MeliorBackendReport::diagnostic(
        MeliorBackendDiagnosticCode::JitSymbolMissing,
        "$.jit.symbol",
        format!("JIT entry symbol '{symbol}' was not found"),
    )
}

fn execute_copy_i32_with_backend(
    artifact: &MeliorModuleArtifact,
    reference: &[i32],
) -> Result<Vec<i32>, MeliorBackendReport> {
    let _ = reference;
    Err(jit_unavailable(
        artifact,
        None,
        "melior ExecutionEngine invocation is not available in this build",
    ))
}

fn jit_unavailable(
    artifact: &MeliorModuleArtifact,
    validation: Option<MeliorBackendReport>,
    message: impl Into<String>,
) -> MeliorBackendReport {
    let mut report = MeliorBackendReport {
        entry_symbol: Some(ENTRY_SYMBOL.to_string()),
        row_count: Some(artifact.row_count),
        artifact_summary: Some(artifact.artifact_summary.clone()),
        ..MeliorBackendReport::default()
    };
    if let Some(validation) = validation {
        report.toolchain = validation.toolchain;
        for diagnostic in validation.diagnostics {
            report.diagnostics.push(diagnostic);
        }
    }
    report.push(
        MeliorBackendDiagnosticCode::JitUnavailable,
        "$.jit",
        message,
    );
    report
}

fn map_lowering_report(report: LoweringSupportReport) -> MeliorBackendReport {
    let mut backend = MeliorBackendReport::default();
    for diagnostic in report.diagnostics() {
        backend.push(
            map_lowering_code(diagnostic.code),
            diagnostic.path.clone(),
            diagnostic.message.clone(),
        );
    }
    backend
}

fn map_lowering_code(code: LoweringDiagnosticCode) -> MeliorBackendDiagnosticCode {
    match code {
        LoweringDiagnosticCode::VerifierRejected => MeliorBackendDiagnosticCode::VerifierRejected,
        LoweringDiagnosticCode::MissingVerifierFacts => {
            MeliorBackendDiagnosticCode::MissingVerifierFacts
        }
        _ => MeliorBackendDiagnosticCode::UnsupportedLoweringShape,
    }
}

#[cfg(test)]
mod tests {
    use arrow_schema::DataType;
    use loom_core::full_verifier::{verify_l2_core, FullVerificationReport};
    use loom_core::l2_core::{
        Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
        ResourceBudget, ScalarExpr,
    };

    use super::*;

    fn sample_program() -> L2CoreProgram {
        L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["l2core.copy.v0".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: "input0".to_string(),
                    offset: 0,
                    length: 16,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "out0".to_string(),
                    arrow_type: DataType::Int32,
                    nullable: true,
                    max_events: 4,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(4),
            body: vec![L2CoreStmt::ForRange {
                index: "i".to_string(),
                start: ScalarExpr::u64(0),
                end: ScalarExpr::u64(4),
                body: vec![
                    L2CoreStmt::ReadInput {
                        capability: "input0".to_string(),
                        offset: ScalarExpr::var("i"),
                        width: ScalarExpr::u64(4),
                        bind: "value".to_string(),
                    },
                    L2CoreStmt::AppendValue {
                        builder: "out0".to_string(),
                        value: ScalarExpr::var("value"),
                    },
                ],
            }],
        }
    }

    #[test]
    fn jit_reports_unavailable_after_supported_preconditions() {
        let program = sample_program();
        let report = verify_l2_core(&program);
        let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30, 40, 50])
            .expect_err("local JIT should be unavailable without compatible toolchain");
        assert!(err
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == MeliorBackendDiagnosticCode::JitUnavailable }));
        assert_eq!(err.entry_symbol.as_deref(), Some("loom_l2core_copy_i32"));
    }

    #[test]
    fn jit_rejects_missing_facts_before_toolchain() {
        let program = sample_program();
        let err = execute_copy_i32_jit(
            &program,
            &FullVerificationReport::default(),
            &[10, 20, 30, 40],
        )
        .expect_err("missing facts must reject before JIT");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::MissingVerifierFacts
        );
    }

    #[test]
    fn jit_rejects_unsupported_program_before_toolchain() {
        let mut program = sample_program();
        program.optional_features.push("debug.extra".to_string());
        let report = verify_l2_core(&program);
        let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30, 40])
            .expect_err("unsupported shape must reject before JIT");
        assert!(err.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == MeliorBackendDiagnosticCode::UnsupportedLoweringShape
        }));
    }

    #[test]
    fn jit_rejects_short_input_before_backend() {
        let program = sample_program();
        let report = verify_l2_core(&program);
        let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30])
            .expect_err("short input must reject before JIT");
        assert!(err.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == MeliorBackendDiagnosticCode::UnsupportedLoweringShape
        }));
    }

    #[test]
    fn native_output_mismatch_has_stable_code() {
        let err = compare_native_output(&[10, 20, 30, 40], &[10, 20, 30, 41])
            .expect_err("mismatch should reject");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::NativeOutputMismatch
        );
        assert_eq!(err.diagnostics[0].code.as_str(), "native-output-mismatch");
    }

    #[test]
    fn jit_symbol_missing_has_stable_code() {
        let err = jit_symbol_missing("missing_symbol");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::JitSymbolMissing
        );
        assert_eq!(err.diagnostics[0].code.as_str(), "jit-symbol-missing");
    }
}
