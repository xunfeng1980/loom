use loom_core::full_verifier::{verify_l2_core, FullVerificationReport};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr,
};
use loom_native_melior::jit::{compare_native_output, execute_copy_i32_jit, jit_symbol_missing};
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
fn external_jit_supported_program_reaches_jit_availability_gate() {
    let program = sample_program();
    let report = verify_l2_core(&program);
    let result = execute_copy_i32_jit(&program, &report, &[10, 20, 30, 40, 50]);

    match result {
        Ok(output) => assert_eq!(output, vec![10, 20, 30, 40]),
        Err(err) => assert!(err.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == MeliorBackendDiagnosticCode::JitUnavailable
                || diagnostic.code == MeliorBackendDiagnosticCode::ToolchainVersionMismatch
                || diagnostic.code == MeliorBackendDiagnosticCode::ToolchainMissing
        })),
    }
}

#[test]
fn external_jit_rejects_missing_facts() {
    let program = sample_program();
    let err = execute_copy_i32_jit(
        &program,
        &FullVerificationReport::default(),
        &[10, 20, 30, 40],
    )
    .expect_err("missing facts must reject");
    assert_eq!(
        err.diagnostics[0].code,
        MeliorBackendDiagnosticCode::MissingVerifierFacts
    );
}

#[test]
fn external_jit_rejects_unsupported_before_backend() {
    let mut program = sample_program();
    program.optional_features.push("debug.extra".to_string());
    let report = verify_l2_core(&program);
    let err = execute_copy_i32_jit(&program, &report, &[10, 20, 30, 40])
        .expect_err("unsupported shape must reject");
    assert!(err.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == MeliorBackendDiagnosticCode::UnsupportedLoweringShape
    }));
}

#[test]
fn external_jit_equivalence_helpers_have_stable_diagnostics() {
    let mismatch = compare_native_output(&[10, 20, 30, 40], &[10, 20, 30, 41])
        .expect_err("native-output-mismatch");
    assert_eq!(
        mismatch.diagnostics[0].code,
        MeliorBackendDiagnosticCode::NativeOutputMismatch
    );

    let missing_symbol = jit_symbol_missing("loom_l2core_copy_i32_typo");
    assert_eq!(
        missing_symbol.diagnostics[0].code,
        MeliorBackendDiagnosticCode::JitSymbolMissing
    );
}
