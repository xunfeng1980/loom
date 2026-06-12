use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use loom_interp::arrow_buffer_lowering::{
    lower_arrow_raw_copy_to_standard_mlir, plan_arrow_buffers_from_decode_dialect,
};

use super::backend::{
    validate_backend_request, NativeBackendDiagnostic, NativeBackendDiagnosticCode,
    NativeBackendReport, NativeBackendRequest, NativeBackendRequestInput, NativeBackendStatus,
};
use super::builder::MeliorModuleArtifact;
use super::report::{
    MeliorBackendDiagnosticCode, MeliorBackendReport, MlirToolKind, MlirToolStatus,
};
use super::toolchain::{probe_toolchain, require_compatible_toolchain};

pub const PRODUCTION_MLIR_VALIDATION_PIPELINE_ID: &str = "phase23-production-mlir-validation-v0";
pub const PRODUCTION_LLVM_LOWERING_PIPELINE_ID: &str = "phase23-llvm-lowering-v0";
pub const LLVM_LOWERING_PIPELINE: &str = "builtin.module(convert-scf-to-cf,convert-cf-to-llvm,expand-strided-metadata,finalize-memref-to-llvm,convert-func-to-llvm,convert-arith-to-llvm,reconcile-unrealized-casts)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MlirValidationOptions {
    pub require_compatible_toolchain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionMlirArtifact {
    pub entry_symbol: String,
    pub mlir_text: String,
    pub row_count: u64,
    pub column_count: usize,
    pub artifact_summary: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProductionBackendPipelineOptions {
    pub require_compatible_toolchain: bool,
    pub validate_llvm_translation: bool,
}

pub fn validate_and_prepare_production_backend(
    input: NativeBackendRequestInput,
    options: ProductionBackendPipelineOptions,
) -> NativeBackendReport {
    match validate_backend_request(input) {
        Ok(request) => prepare_production_backend_pipeline(&request, options),
        Err(report) => report,
    }
}

pub fn prepare_production_backend_pipeline(
    request: &NativeBackendRequest,
    options: ProductionBackendPipelineOptions,
) -> NativeBackendReport {
    if request.cancellation.cancelled {
        return NativeBackendReport::failed_from_request(
            NativeBackendStatus::Cancelled,
            request,
            request.backend_identity.clone(),
            vec![NativeBackendDiagnostic::new(
                NativeBackendDiagnosticCode::Cancelled,
                "$.cancellation",
                request
                    .cancellation
                    .reason
                    .clone()
                    .unwrap_or_else(|| "native backend request was cancelled".to_string()),
            )],
        );
    }

    let pipeline_id = if options.validate_llvm_translation {
        PRODUCTION_LLVM_LOWERING_PIPELINE_ID
    } else {
        PRODUCTION_MLIR_VALIDATION_PIPELINE_ID
    };
    let mut identity = request
        .backend_identity
        .clone()
        .with_pipeline(pipeline_id, Some(LLVM_LOWERING_PIPELINE));
    identity.capabilities.ods_manifest = true;
    identity.capabilities.llvm_lowering = options.validate_llvm_translation;

    let artifact = match production_mlir_artifact_from_request(request) {
        Ok(artifact) => artifact,
        Err(diagnostics) => {
            return NativeBackendReport::failed_from_request(
                NativeBackendStatus::FailClosed,
                request,
                identity,
                diagnostics,
            )
        }
    };

    let melior_report = if options.validate_llvm_translation {
        validate_production_translation_to_llvm_ir(
            &artifact,
            MlirValidationOptions {
                require_compatible_toolchain: options.require_compatible_toolchain,
            },
        )
    } else {
        validate_production_standard_mlir(
            &artifact,
            MlirValidationOptions {
                require_compatible_toolchain: options.require_compatible_toolchain,
            },
        )
    };

    if let Some(toolchain) = melior_report.toolchain.as_ref() {
        identity = identity.with_toolchain(toolchain);
    }

    if melior_report.is_ok() && melior_report.supported {
        return NativeBackendReport::accepted_pipeline(
            request,
            identity,
            artifact.entry_symbol,
            artifact.row_count,
            artifact.column_count,
            artifact.artifact_summary,
        );
    }

    let status = if melior_report.is_ok() && !melior_report.supported {
        NativeBackendStatus::SkippedToolchain
    } else {
        NativeBackendStatus::FailClosed
    };
    let diagnostics = if melior_report.is_ok() && !melior_report.supported {
        vec![NativeBackendDiagnostic::new(
            NativeBackendDiagnosticCode::ToolchainSkipped,
            "$.toolchain",
            "compatible MLIR/LLVM toolchain was not available and strict validation was not required",
        )]
    } else {
        melior_report
            .diagnostics
            .iter()
            .map(|diagnostic| {
                NativeBackendDiagnostic::new(
                    map_melior_diagnostic_code(diagnostic.code),
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect()
    };
    NativeBackendReport::failed_from_request(status, request, identity, diagnostics)
}

fn production_mlir_artifact_from_request(
    request: &NativeBackendRequest,
) -> Result<ProductionMlirArtifact, Vec<NativeBackendDiagnostic>> {
    let buffers = plan_arrow_buffers_from_decode_dialect(&request.lowering_facts);
    let Some(table) = buffers.table() else {
        let diagnostics = buffers
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::BackendFailed,
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect();
        return Err(diagnostics);
    };
    let mlir_text = lower_arrow_raw_copy_to_standard_mlir(table).map_err(|report| {
        report
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                NativeBackendDiagnostic::new(
                    NativeBackendDiagnosticCode::BackendFailed,
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect::<Vec<_>>()
    })?;
    Ok(ProductionMlirArtifact {
        entry_symbol: "loom_decode_build_buffers".to_string(),
        mlir_text,
        row_count: table.row_count,
        column_count: table.columns.len(),
        artifact_summary: format!(
            "phase=23;backend=production-pipeline;columns={};cache={}",
            table.columns.len(),
            request.runtime_cache_key.stable_id
        ),
    })
}

pub fn validate_with_mlir_opt(
    artifact: &MeliorModuleArtifact,
    options: MlirValidationOptions,
) -> MeliorBackendReport {
    let mut report = artifact_report(artifact);
    if let Err(diagnostic) = validate_mlir_shape(artifact) {
        report.push(
            diagnostic,
            "$.mlir_text",
            "MLIR artifact shape is malformed",
        );
        return report;
    }

    let toolchain = if options.require_compatible_toolchain {
        match require_compatible_toolchain() {
            Ok(facts) => facts,
            Err(mut err) => {
                err.entry_symbol = Some(artifact.entry_symbol.clone());
                err.row_count = Some(artifact.row_count);
                err.artifact_summary = Some(artifact.artifact_summary.clone());
                return err;
            }
        }
    } else {
        let facts = probe_toolchain();
        if !facts.compatible {
            report.toolchain = Some(facts);
            return report;
        }
        facts
    };

    let Some(mlir_opt) = tool_path(&toolchain, MlirToolKind::MlirOpt) else {
        report.toolchain = Some(toolchain);
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain.mlir-opt",
            "mlir-opt is required for Phase 16 MLIR validation",
        );
        return report;
    };

    let path = temp_mlir_path("loom-melior-validate");
    if let Err(err) = fs::write(&path, &artifact.mlir_text) {
        report.toolchain = Some(toolchain);
        report.push(
            MeliorBackendDiagnosticCode::PassPipelineFailed,
            "$.tempfile",
            format!("failed to write temporary MLIR file: {err}"),
        );
        return report;
    }

    let output = Command::new(&mlir_opt).arg(&path).output();
    let _ = fs::remove_file(&path);

    report.toolchain = Some(toolchain);
    match output {
        Ok(output) if output.status.success() => {
            report.supported = true;
            report
        }
        Ok(output) => {
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            );
            report
        }
        Err(err) => {
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt",
                format!("failed to run mlir-opt: {err}"),
            );
            report
        }
    }
}

pub fn validate_translation_to_llvm_ir(
    artifact: &MeliorModuleArtifact,
    options: MlirValidationOptions,
) -> MeliorBackendReport {
    let mut report = validate_with_mlir_opt(artifact, options);
    if !report.is_ok() || !report.supported {
        return report;
    }

    let Some(toolchain) = report.toolchain.clone() else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain",
            "compatible toolchain facts are required before MLIR translation",
        );
        return report;
    };
    let Some(mlir_translate) = tool_path(&toolchain, MlirToolKind::MlirTranslate) else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain.mlir-translate",
            "mlir-translate is required for Phase 16 LLVM IR validation",
        );
        return report;
    };
    let Some(mlir_opt) = tool_path(&toolchain, MlirToolKind::MlirOpt) else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain.mlir-opt",
            "mlir-opt is required before Phase 16 LLVM IR translation",
        );
        return report;
    };

    let path = temp_mlir_path("loom-melior-translate");
    if let Err(err) = fs::write(&path, &artifact.mlir_text) {
        report.push(
            MeliorBackendDiagnosticCode::PassPipelineFailed,
            "$.tempfile",
            format!("failed to write temporary MLIR file: {err}"),
        );
        return report;
    }

    let lowered_path = temp_mlir_path("loom-melior-lowered");
    let lowering_output = Command::new(&mlir_opt)
        .arg(&path)
        .arg(format!("--pass-pipeline={LLVM_LOWERING_PIPELINE}"))
        .output();
    if let Ok(output) = &lowering_output {
        if output.status.success() {
            let _ = fs::write(&lowered_path, &output.stdout);
        }
    }
    let _ = fs::remove_file(&path);

    match lowering_output {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let _ = fs::remove_file(&lowered_path);
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt.llvm-lowering",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            );
            return report;
        }
        Err(err) => {
            let _ = fs::remove_file(&lowered_path);
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt.llvm-lowering",
                format!("failed to run mlir-opt lowering pipeline: {err}"),
            );
            return report;
        }
    }

    let output = Command::new(&mlir_translate)
        .arg("--mlir-to-llvmir")
        .arg(&lowered_path)
        .output();
    let _ = fs::remove_file(&lowered_path);

    match output {
        Ok(output) if output.status.success() => report,
        Ok(output) => {
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-translate",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            );
            report
        }
        Err(err) => {
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-translate",
                format!("failed to run mlir-translate: {err}"),
            );
            report
        }
    }
}

pub fn validate_production_standard_mlir(
    artifact: &ProductionMlirArtifact,
    options: MlirValidationOptions,
) -> MeliorBackendReport {
    let mut report = MeliorBackendReport {
        entry_symbol: Some(artifact.entry_symbol.clone()),
        row_count: Some(artifact.row_count),
        artifact_summary: Some(artifact.artifact_summary.clone()),
        ..MeliorBackendReport::default()
    };
    if !validate_production_mlir_shape(artifact) {
        report.push(
            MeliorBackendDiagnosticCode::MlirVerificationFailed,
            "$.mlir_text",
            "Phase 20 production MLIR artifact shape is malformed",
        );
        return report;
    }

    let toolchain = if options.require_compatible_toolchain {
        match require_compatible_toolchain() {
            Ok(facts) => facts,
            Err(mut err) => {
                err.entry_symbol = Some(artifact.entry_symbol.clone());
                err.row_count = Some(artifact.row_count);
                err.artifact_summary = Some(artifact.artifact_summary.clone());
                return err;
            }
        }
    } else {
        let facts = probe_toolchain();
        if !facts.compatible {
            report.toolchain = Some(facts);
            return report;
        }
        facts
    };

    let Some(mlir_opt) = tool_path(&toolchain, MlirToolKind::MlirOpt) else {
        report.toolchain = Some(toolchain);
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain.mlir-opt",
            "mlir-opt is required for Phase 20 production MLIR validation",
        );
        return report;
    };

    let path = temp_mlir_path("loom-production-validate");
    if let Err(err) = fs::write(&path, &artifact.mlir_text) {
        report.toolchain = Some(toolchain);
        report.push(
            MeliorBackendDiagnosticCode::PassPipelineFailed,
            "$.tempfile",
            format!("failed to write temporary MLIR file: {err}"),
        );
        return report;
    }

    let output = Command::new(&mlir_opt).arg(&path).output();
    let _ = fs::remove_file(&path);

    report.toolchain = Some(toolchain);
    match output {
        Ok(output) if output.status.success() => {
            report.supported = true;
            report
        }
        Ok(output) => {
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt.production",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            );
            report
        }
        Err(err) => {
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt.production",
                format!("failed to run mlir-opt: {err}"),
            );
            report
        }
    }
}

pub fn validate_production_translation_to_llvm_ir(
    artifact: &ProductionMlirArtifact,
    options: MlirValidationOptions,
) -> MeliorBackendReport {
    let mut report = validate_production_standard_mlir(artifact, options);
    if !report.is_ok() || !report.supported {
        return report;
    }

    let Some(toolchain) = report.toolchain.clone() else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain",
            "compatible toolchain facts are required before production LLVM translation",
        );
        return report;
    };
    let Some(mlir_translate) = tool_path(&toolchain, MlirToolKind::MlirTranslate) else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain.mlir-translate",
            "mlir-translate is required for production LLVM IR validation",
        );
        return report;
    };
    let Some(mlir_opt) = tool_path(&toolchain, MlirToolKind::MlirOpt) else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain.mlir-opt",
            "mlir-opt is required before production LLVM IR translation",
        );
        return report;
    };

    let path = temp_mlir_path("loom-production-translate");
    if let Err(err) = fs::write(&path, &artifact.mlir_text) {
        report.push(
            MeliorBackendDiagnosticCode::PassPipelineFailed,
            "$.tempfile",
            format!("failed to write temporary MLIR file: {err}"),
        );
        return report;
    }

    let lowered_path = temp_mlir_path("loom-production-lowered");
    let lowering_output = Command::new(&mlir_opt)
        .arg(&path)
        .arg(format!("--pass-pipeline={LLVM_LOWERING_PIPELINE}"))
        .output();
    if let Ok(output) = &lowering_output {
        if output.status.success() {
            let _ = fs::write(&lowered_path, &output.stdout);
        }
    }
    let _ = fs::remove_file(&path);

    match lowering_output {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let _ = fs::remove_file(&lowered_path);
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt.production-llvm-lowering",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            );
            return report;
        }
        Err(err) => {
            let _ = fs::remove_file(&lowered_path);
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-opt.production-llvm-lowering",
                format!("failed to run production LLVM lowering pipeline: {err}"),
            );
            return report;
        }
    }

    let output = Command::new(&mlir_translate)
        .arg("--mlir-to-llvmir")
        .arg(&lowered_path)
        .output();
    let _ = fs::remove_file(&lowered_path);

    match output {
        Ok(output) if output.status.success() => report,
        Ok(output) => {
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-translate.production",
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            );
            report
        }
        Err(err) => {
            report.supported = false;
            report.push(
                MeliorBackendDiagnosticCode::PassPipelineFailed,
                "$.mlir-translate.production",
                format!("failed to run production mlir-translate: {err}"),
            );
            report
        }
    }
}

fn map_melior_diagnostic_code(code: MeliorBackendDiagnosticCode) -> NativeBackendDiagnosticCode {
    match code {
        MeliorBackendDiagnosticCode::ToolchainMissing
        | MeliorBackendDiagnosticCode::ToolchainVersionMismatch => {
            NativeBackendDiagnosticCode::ToolchainFailed
        }
        _ => NativeBackendDiagnosticCode::BackendFailed,
    }
}

fn artifact_report(artifact: &MeliorModuleArtifact) -> MeliorBackendReport {
    MeliorBackendReport {
        entry_symbol: Some(artifact.entry_symbol.clone()),
        row_count: Some(artifact.row_count),
        artifact_summary: Some(artifact.artifact_summary.clone()),
        ..MeliorBackendReport::default()
    }
}

fn validate_mlir_shape(artifact: &MeliorModuleArtifact) -> Result<(), MeliorBackendDiagnosticCode> {
    let text = artifact.mlir_text.as_str();
    let required = [
        "module {",
        "func.func @loom_l2core_copy_i32",
        "scf.for",
        "memref.load",
        "memref.store",
        "return",
    ];
    if artifact.entry_symbol != crate::report::ENTRY_SYMBOL {
        return Err(MeliorBackendDiagnosticCode::MlirVerificationFailed);
    }
    if required.iter().all(|needle| text.contains(needle)) {
        Ok(())
    } else {
        Err(MeliorBackendDiagnosticCode::MlirVerificationFailed)
    }
}

fn validate_production_mlir_shape(artifact: &ProductionMlirArtifact) -> bool {
    artifact.entry_symbol == "loom_decode_build_buffers"
        && artifact.column_count > 0
        && artifact.mlir_text.contains("module {")
        && artifact
            .mlir_text
            .contains("func.func @loom_decode_build_buffers")
        && artifact.mlir_text.contains("scf.for")
        && artifact.mlir_text.contains("memref.store")
        && artifact.mlir_text.contains("return")
}

fn tool_path(facts: &crate::report::MlirToolchainFacts, kind: MlirToolKind) -> Option<String> {
    facts.tool(kind).and_then(|fact| match &fact.status {
        MlirToolStatus::Found { path } => Some(path.clone()),
        MlirToolStatus::Missing => None,
    })
}

fn temp_mlir_path(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("{prefix}-{nanos}.mlir"))
}

#[cfg(test)]
mod tests {
    use crate::full_verifier::verify_l2_core;
    use crate::l2_core::{
        Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
        OutputBuilderCapability, ResourceBudget, ScalarExpr,
    };

    use crate::jit::builder::build_melior_module;

    use super::*;

    fn sample_artifact() -> MeliorModuleArtifact {
        let program = L2CoreProgram {
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
                    arrow_type: L2DataType::Int32,
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
        };
        let report = verify_l2_core(&program);
        build_melior_module(&program, &report).expect("supported artifact")
    }

    #[test]
    fn validate_with_mlir_opt_is_skip_aware_without_strict_toolchain() {
        let artifact = sample_artifact();
        let report = validate_with_mlir_opt(&artifact, MlirValidationOptions::default());
        assert!(report.is_ok());
        assert_eq!(report.entry_symbol.as_deref(), Some("loom_l2core_copy_i32"));
    }

    #[test]
    fn validate_with_mlir_opt_rejects_malformed_mlir_text() {
        let mut artifact = sample_artifact();
        artifact.mlir_text = "not valid mlir".to_string();
        let report = validate_with_mlir_opt(&artifact, MlirValidationOptions::default());
        assert!(!report.is_ok());
        assert_eq!(
            report.diagnostics[0].code,
            MeliorBackendDiagnosticCode::MlirVerificationFailed
        );
    }

    #[test]
    fn strict_validation_reports_toolchain_failure_when_incompatible() {
        let artifact = sample_artifact();
        let report = validate_with_mlir_opt(
            &artifact,
            MlirValidationOptions {
                require_compatible_toolchain: true,
            },
        );
        if !report.is_ok() {
            assert!(matches!(
                report.diagnostics[0].code,
                MeliorBackendDiagnosticCode::ToolchainMissing
                    | MeliorBackendDiagnosticCode::ToolchainVersionMismatch
            ));
            assert!(
                report.diagnostics[0].code.as_str() == "toolchain-version-mismatch"
                    || report.diagnostics[0].code.as_str() == "toolchain-missing"
            );
        }
    }

    #[test]
    fn translation_validation_is_skip_aware_without_strict_toolchain() {
        let artifact = sample_artifact();
        let report = validate_translation_to_llvm_ir(&artifact, MlirValidationOptions::default());
        assert!(report.is_ok());
    }

    #[test]
    fn diagnostic_code_markers_are_stable_for_pipeline_failures() {
        assert_eq!(
            MeliorBackendDiagnosticCode::PassPipelineFailed.as_str(),
            "pass-pipeline-failed"
        );
    }
}
