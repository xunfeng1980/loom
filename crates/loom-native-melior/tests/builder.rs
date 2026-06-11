use loom_core::full_verifier::{verify_l2_core, FullVerificationReport};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue, ScratchCapability,
};
use loom_native_melior::builder::build_melior_module;
use loom_native_melior::report::MeliorBackendDiagnosticCode;

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
fn external_builder_accepts_supported_program() {
    let program = sample_program();
    let report = verify_l2_core(&program);
    let artifact = build_melior_module(&program, &report).expect("supported artifact");

    assert_eq!(artifact.entry_symbol, "loom_l2core_copy_i32");
    assert_eq!(artifact.row_count, 4);
    assert!(artifact.mlir_text.contains("memref.store"));
}

#[test]
fn external_builder_rejects_standalone_missing_facts() {
    let program = sample_program();
    let err = build_melior_module(&program, &FullVerificationReport::default())
        .expect_err("missing facts must reject");
    assert_eq!(
        err.diagnostics[0].code,
        MeliorBackendDiagnosticCode::MissingVerifierFacts
    );
}

#[test]
fn external_builder_rejects_verifier_rejected_program() {
    let mut program = sample_program();
    program.capabilities.clear();
    let report = verify_l2_core(&program);
    assert_rejected_with(
        program,
        report,
        MeliorBackendDiagnosticCode::VerifierRejected,
    );
}

#[test]
fn external_builder_rejects_optional_features() {
    let mut program = sample_program();
    program.optional_features.push("debug.extra".to_string());
    assert_unsupported(program);
}

#[test]
fn external_builder_rejects_cursor_loop_even_when_verified() {
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
    assert_rejected_with(
        program,
        report,
        MeliorBackendDiagnosticCode::UnsupportedLoweringShape,
    );
}

#[test]
fn external_builder_rejects_append_null() {
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
fn external_builder_rejects_scratch_capability() {
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
fn external_builder_rejects_unsupported_expression_shape() {
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
    assert_rejected_with(
        program,
        report,
        MeliorBackendDiagnosticCode::UnsupportedLoweringShape,
    );
}

fn assert_rejected_with(
    program: L2CoreProgram,
    report: FullVerificationReport,
    code: MeliorBackendDiagnosticCode,
) {
    let err = build_melior_module(&program, &report).expect_err("should reject");
    assert!(
        err.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "unexpected diagnostics: {:?}",
        err.diagnostics
    );
}
