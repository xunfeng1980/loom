use loom_ffi::full_verifier::FullVerificationReport;
use loom_ffi::l2_core::L2CoreProgram;
use loom_ffi::native_lowering::{
    check_lowering_support, lower_to_textual_mlir, LoweringDiagnosticCode,
};

use crate::report::{MeliorBackendDiagnosticCode, MeliorBackendReport, ENTRY_SYMBOL};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeliorModuleArtifact {
    pub entry_symbol: String,
    pub mlir_text: String,
    pub facts_linkage: String,
    pub row_count: u64,
    pub artifact_summary: String,
}

pub fn build_melior_module(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
) -> Result<MeliorModuleArtifact, MeliorBackendReport> {
    if !report.is_ok() {
        return Err(MeliorBackendReport::diagnostic(
            MeliorBackendDiagnosticCode::VerifierRejected,
            "$.verification",
            "L2Core verifier rejected the program",
        ));
    }
    if report.facts().is_none() {
        return Err(MeliorBackendReport::diagnostic(
            MeliorBackendDiagnosticCode::MissingVerifierFacts,
            "$.verification.facts",
            "accepted backend construction requires verifier facts from the same report",
        ));
    }

    let support = check_lowering_support(program, report);
    if !support.is_supported() {
        let mut backend = MeliorBackendReport::default();
        for diagnostic in support.diagnostics() {
            backend.push(
                map_lowering_code(diagnostic.code),
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            );
        }
        return Err(backend);
    }

    let textual = lower_to_textual_mlir(program, report).map_err(|support| {
        let mut backend = MeliorBackendReport::default();
        for diagnostic in support.diagnostics() {
            backend.push(
                map_lowering_code(diagnostic.code),
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            );
        }
        backend
    })?;

    Ok(MeliorModuleArtifact {
        entry_symbol: textual.entry_symbol,
        mlir_text: textual.mlir_text,
        facts_linkage: textual.facts_linkage,
        row_count: textual.row_count,
        artifact_summary: format!(
            "backend=melior-programmatic;entry={ENTRY_SYMBOL};rows={}",
            textual.row_count
        ),
    })
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
    use loom_ffi::full_verifier::{verify_l2_core, FullVerificationReport};
    use loom_ffi::l2_core::{
        Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
        OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue, ScratchCapability,
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
                        offset: ScalarExpr::Add(
                            Box::new(ScalarExpr::var("i")),
                            Box::new(ScalarExpr::u64(0)),
                        ),
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
    fn builder_accepts_supported_copy() {
        let program = sample_program();
        let report = verify_l2_core(&program);
        let artifact = build_melior_module(&program, &report).expect("supported artifact");

        assert_eq!(artifact.entry_symbol, "loom_l2core_copy_i32");
        assert_eq!(artifact.row_count, 4);
        assert!(artifact
            .mlir_text
            .contains("func.func @loom_l2core_copy_i32"));
        assert!(artifact.mlir_text.contains("scf.for"));
        assert!(artifact.mlir_text.contains("memref.load"));
        assert!(artifact.mlir_text.contains("memref.store"));
        assert!(artifact.artifact_summary.contains("melior-programmatic"));
    }

    #[test]
    fn builder_rejects_verifier_rejected_program() {
        let mut program = sample_program();
        program.capabilities.clear();
        let report = verify_l2_core(&program);
        let err = build_melior_module(&program, &report).expect_err("should reject");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::VerifierRejected
        );
    }

    #[test]
    fn builder_rejects_missing_facts() {
        let program = sample_program();
        let err = build_melior_module(&program, &FullVerificationReport::default())
            .expect_err("should reject");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::MissingVerifierFacts
        );
    }

    #[test]
    fn builder_rejects_optional_features() {
        let mut program = sample_program();
        program.optional_features.push("debug.extra".to_string());
        assert_unsupported(program);
    }

    #[test]
    fn builder_rejects_non_int32_output() {
        let mut program = sample_program();
        if let Capability::OutputBuilder(builder) = &mut program.capabilities[1] {
            builder.arrow_type = L2DataType::Int64;
        }
        let report = verify_l2_core(&program);
        let err = build_melior_module(&program, &report).expect_err("should reject");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::VerifierRejected
        );
    }

    #[test]
    fn builder_rejects_cursor_loop_even_when_verified() {
        let mut program = sample_program();
        program.body = vec![L2CoreStmt::CursorLoop {
            cursor: "cursor".to_string(),
            limit: ScalarExpr::u64(4),
            progress: ScalarExpr::Add(
                Box::new(ScalarExpr::var("cursor")),
                Box::new(ScalarExpr::u64(1)),
            ),
            body: vec![],
        }];
        let report = verify_l2_core(&program);
        assert!(report.is_ok());
        let err = build_melior_module(&program, &report).expect_err("should reject");
        assert_eq!(
            err.diagnostics[0].code,
            MeliorBackendDiagnosticCode::UnsupportedLoweringShape
        );
    }

    #[test]
    fn builder_rejects_append_null() {
        let mut program = sample_program();
        program.body = vec![L2CoreStmt::ForRange {
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
                L2CoreStmt::AppendNull {
                    builder: "out0".to_string(),
                },
            ],
        }];
        assert_unsupported(program);
    }

    #[test]
    fn builder_rejects_scratch_capability() {
        let mut program = sample_program();
        program
            .capabilities
            .push(Capability::Scratch(ScratchCapability {
                id: "tmp".to_string(),
                max_bytes: 32,
            }));
        assert_unsupported(program);
    }

    #[test]
    fn builder_rejects_unsupported_expression_shape() {
        let mut program = sample_program();
        let L2CoreStmt::ForRange { body, .. } = &mut program.body[0] else {
            unreachable!();
        };
        let L2CoreStmt::ReadInput { offset, .. } = &mut body[0] else {
            unreachable!();
        };
        *offset = ScalarExpr::Mul(
            Box::new(ScalarExpr::var("i")),
            Box::new(ScalarExpr::Const(ScalarValue::UInt64(4))),
        );
        assert_unsupported(program);
    }

    fn assert_unsupported(program: L2CoreProgram) {
        let report = verify_l2_core(&program);
        let err = build_melior_module(&program, &report).expect_err("should reject");
        assert!(
            err.diagnostics.iter().any(|diagnostic| diagnostic.code
                == MeliorBackendDiagnosticCode::UnsupportedLoweringShape),
            "unexpected diagnostics: {:?}",
            err.diagnostics
        );
    }
}
