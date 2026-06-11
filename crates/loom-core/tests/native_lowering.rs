use loom_core::full_verifier::{verify_l2_core, FullVerificationReport};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue, ScratchCapability,
};
use loom_core::native_lowering::{
    check_lowering_support, execute_supported_copy_i32, lower_to_textual_mlir, LoweringBackend,
    LoweringDiagnosticCode,
};

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

fn lowering_codes(program: &L2CoreProgram) -> Vec<LoweringDiagnosticCode> {
    let verifier_report = verify_l2_core(program);
    check_lowering_support(program, &verifier_report)
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn native_lowering_accepts_verified_bounded_int32_copy() {
    let program = sample_program();
    let verifier_report = verify_l2_core(&program);
    let support = check_lowering_support(&program, &verifier_report);

    assert!(
        support.is_supported(),
        "unexpected lowering diagnostics: {:?}",
        support.diagnostics()
    );
    let copy = support
        .supported_copy()
        .expect("supported program should expose copy slice");
    assert_eq!(copy.input_id, "input0");
    assert_eq!(copy.output_builder_id, "out0");
    assert_eq!(copy.row_count, 4);
    assert_eq!(copy.loop_index, "i");
    assert_eq!(copy.read_bind, "value");
}

#[test]
fn native_lowering_emits_deterministic_textual_mlir_for_supported_copy() {
    let program = sample_program();
    let verifier_report = verify_l2_core(&program);
    let artifact = lower_to_textual_mlir(&program, &verifier_report)
        .expect("supported program should emit textual MLIR");

    assert_eq!(artifact.backend, LoweringBackend::TextualMlir);
    assert_eq!(artifact.backend.as_str(), "textual-mlir");
    assert_eq!(artifact.entry_symbol, "loom_l2core_copy_i32");
    assert_eq!(artifact.row_count, 4);
    assert!(artifact.facts_linkage.contains("artifact_version=1"));
    assert!(artifact.facts_linkage.contains("features=l2core.copy.v0"));
    assert!(artifact.facts_linkage.contains("VERIFIER-10"));

    let expected = "module {\n  func.func @loom_l2core_copy_i32(%input: memref<?xi32>, %output: memref<?xi32>, %rows: index) {\n    %c0 = arith.constant 0 : index\n    %c1 = arith.constant 1 : index\n    scf.for %i = %c0 to %rows step %c1 {\n      %v = memref.load %input[%i] : memref<?xi32>\n      memref.store %v, %output[%i] : memref<?xi32>\n    }\n    return\n  }\n}\n";
    assert_eq!(artifact.mlir_text, expected);
    for marker in [
        "func.func @loom_l2core_copy_i32",
        "arith.constant",
        "scf.for",
        "memref.load",
        "memref.store",
        "return",
    ] {
        assert!(
            artifact.mlir_text.contains(marker),
            "missing MLIR marker {marker}"
        );
    }
}

#[test]
fn native_lowering_does_not_emit_mlir_for_unsupported_programs() {
    let mut program = sample_program();
    program.optional_features.push("debug.extra".to_string());
    let verifier_report = verify_l2_core(&program);

    let err = lower_to_textual_mlir(&program, &verifier_report)
        .expect_err("unsupported optional feature should reject before emission");
    assert!(!err.diagnostics().is_empty());
    assert_eq!(
        err.first_error().expect("diagnostic").code,
        LoweringDiagnosticCode::UnsupportedFeature
    );
}

#[test]
fn native_lowering_reference_copy_matches_supported_shape() {
    let program = sample_program();
    let verifier_report = verify_l2_core(&program);
    let artifact = lower_to_textual_mlir(&program, &verifier_report)
        .expect("supported program should emit textual MLIR");
    let output = execute_supported_copy_i32(&program, &verifier_report, &[10, 20, 30, 40, 50])
        .expect("supported copy should execute against typed primitive input");

    assert_eq!(artifact.row_count as usize, output.len());
    assert_eq!(output, vec![10, 20, 30, 40]);
}

#[test]
fn native_lowering_reference_copy_rejects_short_input() {
    let program = sample_program();
    let verifier_report = verify_l2_core(&program);
    let err = execute_supported_copy_i32(&program, &verifier_report, &[10, 20])
        .expect_err("short input should reject before copying");

    let first = err.first_error().expect("diagnostic");
    assert_eq!(
        first.code,
        LoweringDiagnosticCode::UnsupportedCapabilityShape
    );
    assert!(first.message.contains("row_count_bound"));
}

#[test]
fn native_lowering_rejects_verifier_rejected_program() {
    let mut program = sample_program();
    program.capabilities.clear();
    let verifier_report = verify_l2_core(&program);
    assert!(!verifier_report.is_ok());

    let support = check_lowering_support(&program, &verifier_report);
    let first = support.first_error().expect("lowering should reject");
    assert_eq!(first.code, LoweringDiagnosticCode::VerifierRejected);
    assert_eq!(first.code.as_str(), "verifier-rejected");
}

#[test]
fn native_lowering_rejects_missing_verifier_facts() {
    let program = sample_program();
    let support = check_lowering_support(&program, &FullVerificationReport::default());

    let first = support.first_error().expect("lowering should reject");
    assert_eq!(first.code, LoweringDiagnosticCode::MissingVerifierFacts);
    assert_eq!(first.code.as_str(), "missing-verifier-facts");
}

#[test]
fn native_lowering_rejects_cursor_loop_even_when_verified() {
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
    let verifier_report = verify_l2_core(&program);
    assert!(
        verifier_report.is_ok(),
        "cursor loop should be verifier-accepted for this support-gate test"
    );

    let codes = lowering_codes(&program);
    assert!(codes.contains(&LoweringDiagnosticCode::UnsupportedLoopShape));
}

#[test]
fn native_lowering_rejects_append_null() {
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
    let verifier_report = verify_l2_core(&program);
    assert!(verifier_report.is_ok());

    let codes = lowering_codes(&program);
    assert!(codes.contains(&LoweringDiagnosticCode::UnsupportedNullability));
}

#[test]
fn native_lowering_rejects_non_int32_output_type() {
    let mut program = sample_program();
    program.capabilities = vec![
        Capability::InputSlice(InputSliceCapability {
            id: "input0".to_string(),
            offset: 0,
            length: 32,
        }),
        Capability::OutputBuilder(OutputBuilderCapability {
            id: "out0".to_string(),
            arrow_type: L2DataType::Int64,
            nullable: true,
            max_events: 4,
        }),
    ];
    if let L2CoreStmt::ForRange { body, .. } = &mut program.body[0] {
        body[1] = L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: ScalarExpr::Const(ScalarValue::Int64(7)),
        };
    }

    let verifier_report = verify_l2_core(&program);
    assert!(verifier_report.is_ok());
    let codes = lowering_codes(&program);
    assert!(codes.contains(&LoweringDiagnosticCode::UnsupportedType));
}

#[test]
fn native_lowering_rejects_extra_scratch_capability() {
    let mut program = sample_program();
    program
        .capabilities
        .push(Capability::Scratch(ScratchCapability {
            id: "scratch0".to_string(),
            max_bytes: 16,
        }));

    let verifier_report = verify_l2_core(&program);
    assert!(verifier_report.is_ok());
    let codes = lowering_codes(&program);
    assert!(codes.contains(&LoweringDiagnosticCode::UnsupportedCapabilityShape));
}

#[test]
fn native_lowering_rejects_unsupported_expression_shape() {
    let mut program = sample_program();
    if let L2CoreStmt::ForRange { body, .. } = &mut program.body[0] {
        body[0] = L2CoreStmt::ReadInput {
            capability: "input0".to_string(),
            offset: ScalarExpr::Add(Box::new(ScalarExpr::var("i")), Box::new(ScalarExpr::u64(1))),
            width: ScalarExpr::u64(4),
            bind: "value".to_string(),
        };
    }

    let verifier_report = verify_l2_core(&program);
    assert!(verifier_report.is_ok());
    let codes = lowering_codes(&program);
    assert!(codes.contains(&LoweringDiagnosticCode::UnsupportedExpressionShape));
}
