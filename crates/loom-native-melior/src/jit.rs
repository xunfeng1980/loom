use loom_core::full_verifier::FullVerificationReport;
use loom_core::l2_core::L2CoreProgram;
use loom_core::native_lowering::{
    execute_supported_copy_i32, LoweringDiagnosticCode, LoweringSupportReport,
};

use crate::builder::{build_melior_module, MeliorModuleArtifact};
use crate::pipeline::{validate_translation_to_llvm_ir, MlirValidationOptions};
use crate::report::{MeliorBackendDiagnosticCode, MeliorBackendReport, ENTRY_SYMBOL};

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
