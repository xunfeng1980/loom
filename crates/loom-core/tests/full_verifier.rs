use loom_core::full_verifier::{verify_l2_core, FullVerificationCode};
use loom_core::l2_core::constraints::LoomConstraint;
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
};
use std::fs;

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

fn diagnostic_codes(program: &L2CoreProgram) -> Vec<FullVerificationCode> {
    verify_l2_core(program)
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

struct CorrespondenceCase {
    id: &'static str,
    program: L2CoreProgram,
    expected: Option<FullVerificationCode>,
}

fn correspondence_classification(program: &L2CoreProgram) -> Option<FullVerificationCode> {
    verify_l2_core(program)
        .first_error()
        .map(|diagnostic| diagnostic.code)
}

fn correspondence_classification_str(code: Option<FullVerificationCode>) -> &'static str {
    match code {
        Some(code) => code.as_str(),
        None => "accepted",
    }
}

fn correspondence_line(case: &CorrespondenceCase) -> String {
    format!(
        "correspondence:{}:{}",
        case.id,
        correspondence_classification_str(correspondence_classification(&case.program))
    )
}

fn correspondence_cases() -> Vec<CorrespondenceCase> {
    let mut missing_input = sample_program();
    missing_input
        .capabilities
        .retain(|capability| !matches!(capability, Capability::InputSlice(_)));

    let mut missing_output = sample_program();
    missing_output.body = vec![L2CoreStmt::AppendNull {
        builder: "missing".to_string(),
    }];

    let mut invalid_loop = sample_program();
    invalid_loop.body = vec![L2CoreStmt::ForRange {
        index: "i".to_string(),
        start: ScalarExpr::u64(4),
        end: ScalarExpr::u64(0),
        body: vec![],
    }];

    let mut non_monotone = sample_program();
    non_monotone.body = vec![L2CoreStmt::CursorLoop {
        cursor: "cursor".to_string(),
        limit: ScalarExpr::u64(4),
        progress: ScalarExpr::var("cursor"),
        body: vec![],
    }];

    let mut resource_budget = sample_program();
    resource_budget.body = vec![L2CoreStmt::ForRange {
        index: "i".to_string(),
        start: ScalarExpr::u64(0),
        end: ScalarExpr::u64(5),
        body: vec![],
    }];

    let mut unknown_variable = sample_program();
    unknown_variable.body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: ScalarExpr::var("missing"),
    }];

    let mut output_type_mismatch = sample_program();
    output_type_mismatch.body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: ScalarExpr::Const(ScalarValue::Bool(true)),
    }];

    let mut output_nullability_mismatch = sample_program();
    output_nullability_mismatch.capabilities = vec![
        Capability::InputSlice(InputSliceCapability {
            id: "input0".to_string(),
            offset: 0,
            length: 16,
        }),
        Capability::OutputBuilder(OutputBuilderCapability {
            id: "out0".to_string(),
            arrow_type: L2DataType::Int32,
            nullable: false,
            max_events: 4,
        }),
    ];
    output_nullability_mismatch.body = vec![L2CoreStmt::AppendNull {
        builder: "out0".to_string(),
    }];

    let mut fuzz_000 = sample_program();
    fuzz_000.body = vec![
        L2CoreStmt::LetScalar {
            name: "x".to_string(),
            expr: ScalarExpr::Const(ScalarValue::Int32(7)),
        },
        L2CoreStmt::LetScalar {
            name: "y".to_string(),
            expr: ScalarExpr::Add(
                Box::new(ScalarExpr::var("x")),
                Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            ),
        },
        L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: ScalarExpr::var("y"),
        },
    ];

    let mut fuzz_001 = sample_program();
    fuzz_001.capabilities = vec![
        Capability::InputSlice(InputSliceCapability {
            id: "input0".to_string(),
            offset: 0,
            length: 16,
        }),
        Capability::OutputBuilder(OutputBuilderCapability {
            id: "out0".to_string(),
            arrow_type: L2DataType::Boolean,
            nullable: false,
            max_events: 4,
        }),
    ];
    fuzz_001.body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: ScalarExpr::Eq(
            Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
        ),
    }];

    let mut fuzz_002 = sample_program();
    fuzz_002.body = vec![
        L2CoreStmt::ReadInput {
            capability: "input0".to_string(),
            offset: ScalarExpr::u64(0),
            width: ScalarExpr::u64(3),
            bind: "value".to_string(),
        },
        L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: ScalarExpr::var("value"),
        },
    ];

    let mut read_out_of_bounds = sample_program();
    read_out_of_bounds.body = vec![L2CoreStmt::ReadInput {
        capability: "input0".to_string(),
        offset: ScalarExpr::u64(16),
        width: ScalarExpr::u64(4),
        bind: "value".to_string(),
    }];

    let mut fuzz_003 = sample_program();
    fuzz_003.capabilities = vec![Capability::OutputBuilder(OutputBuilderCapability {
        id: "score32".to_string(),
        arrow_type: L2DataType::Float32,
        nullable: false,
        max_events: 4,
    })];
    fuzz_003.body = vec![L2CoreStmt::AppendValue {
        builder: "score32".to_string(),
        value: ScalarExpr::Const(ScalarValue::Float32Bits(1.5f32.to_bits())),
    }];

    let mut fuzz_004 = sample_program();
    fuzz_004.capabilities = vec![Capability::OutputBuilder(OutputBuilderCapability {
        id: "score64".to_string(),
        arrow_type: L2DataType::Float64,
        nullable: true,
        max_events: 4,
    })];
    fuzz_004.body = vec![
        L2CoreStmt::AppendValue {
            builder: "score64".to_string(),
            value: ScalarExpr::Const(ScalarValue::Float64Bits((-2.25f64).to_bits())),
        },
        L2CoreStmt::AppendNull {
            builder: "score64".to_string(),
        },
    ];

    let mut explicit_fail_closed = sample_program();
    explicit_fail_closed.body = vec![L2CoreStmt::FailClosed {
        code: "test-fail-closed".to_string(),
    }];

    vec![
        CorrespondenceCase {
            id: "matrix-accepted-copy",
            program: sample_program(),
            expected: None,
        },
        CorrespondenceCase {
            id: "matrix-missing-input-capability",
            program: missing_input,
            expected: Some(FullVerificationCode::MissingInputCapability),
        },
        CorrespondenceCase {
            id: "matrix-missing-output-builder",
            program: missing_output,
            expected: Some(FullVerificationCode::MissingOutputBuilder),
        },
        CorrespondenceCase {
            id: "matrix-invalid-loop-bounds",
            program: invalid_loop,
            expected: Some(FullVerificationCode::InvalidLoopBounds),
        },
        CorrespondenceCase {
            id: "matrix-non-monotone-cursor-loop",
            program: non_monotone,
            expected: Some(FullVerificationCode::NonMonotoneCursorLoop),
        },
        CorrespondenceCase {
            id: "matrix-resource-budget-exceeded",
            program: resource_budget,
            expected: Some(FullVerificationCode::ResourceBudgetExceeded),
        },
        CorrespondenceCase {
            id: "matrix-unknown-variable",
            program: unknown_variable,
            expected: Some(FullVerificationCode::UnknownVariable),
        },
        CorrespondenceCase {
            id: "matrix-output-type-mismatch",
            program: output_type_mismatch,
            expected: Some(FullVerificationCode::OutputTypeMismatch),
        },
        CorrespondenceCase {
            id: "matrix-output-nullability-mismatch",
            program: output_nullability_mismatch,
            expected: Some(FullVerificationCode::OutputNullabilityMismatch),
        },
        CorrespondenceCase {
            id: "fuzz-000-let-add-int32",
            program: fuzz_000,
            expected: None,
        },
        CorrespondenceCase {
            id: "fuzz-001-eq-bool",
            program: fuzz_001,
            expected: None,
        },
        CorrespondenceCase {
            id: "fuzz-002-read-width-bytes-mismatch",
            program: fuzz_002,
            expected: Some(FullVerificationCode::OutputTypeMismatch),
        },
        CorrespondenceCase {
            id: "matrix-read-out-of-bounds",
            program: read_out_of_bounds,
            expected: Some(FullVerificationCode::MissingInputCapability),
        },
        CorrespondenceCase {
            id: "fuzz-003-float32-builder",
            program: fuzz_003,
            expected: None,
        },
        CorrespondenceCase {
            id: "fuzz-004-float64-nullable-builder",
            program: fuzz_004,
            expected: None,
        },
        CorrespondenceCase {
            id: "matrix-explicit-fail-closed",
            program: explicit_fail_closed,
            expected: Some(FullVerificationCode::ExplicitFailClosed),
        },
    ]
}

#[test]
fn verifier_06_10_accepts_bounded_copy_and_emits_facts() {
    let program = sample_program();
    let report = verify_l2_core(&program);

    assert!(
        report.is_ok(),
        "unexpected diagnostics: {:?}",
        report.diagnostics()
    );
    let facts = report.facts().expect("accepted program should emit facts");
    assert_eq!(facts.row_count_bound, Some(4));
    assert_eq!(facts.input_ranges[0].capability_id, "input0");
    assert_eq!(facts.output_schema[0].builder_id, "out0");
    assert!(facts
        .proof_obligation_ids
        .iter()
        .any(|id| id == "VERIFIER-06"));
    assert!(facts
        .proof_obligation_ids
        .iter()
        .any(|id| id == "VERIFIER-10"));
}

#[test]
fn verifier_04_missing_input_capability_rejects_program() {
    let mut program = sample_program();
    program
        .capabilities
        .retain(|capability| !matches!(capability, Capability::InputSlice(_)));

    let codes = diagnostic_codes(&program);
    assert!(codes.contains(&FullVerificationCode::MissingInputCapability));
}

#[test]
fn verifier_08_output_type_mismatch_has_stable_diagnostic() {
    let mut program = sample_program();
    program.body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: ScalarExpr::Const(ScalarValue::Bool(true)),
    }];

    let report = verify_l2_core(&program);
    let first = report.first_error().expect("type mismatch should reject");
    assert_eq!(first.code, FullVerificationCode::OutputTypeMismatch);
    assert_eq!(first.code.as_str(), "output-type-mismatch");
    assert!(first.path.contains("$.body[0].value"));
}

#[test]
fn verifier_06_non_monotone_cursor_loop_rejects_program() {
    let mut program = sample_program();
    program.body = vec![L2CoreStmt::CursorLoop {
        cursor: "cursor".to_string(),
        limit: ScalarExpr::u64(4),
        progress: ScalarExpr::var("cursor"),
        body: vec![],
    }];

    let codes = diagnostic_codes(&program);
    assert!(codes.contains(&FullVerificationCode::NonMonotoneCursorLoop));
}

#[test]
fn verifier_07_emits_overflow_range_and_progress_constraints() {
    let mut program = sample_program();
    program.body.push(L2CoreStmt::CursorLoop {
        cursor: "cursor".to_string(),
        limit: ScalarExpr::u64(4),
        progress: ScalarExpr::Add(
            Box::new(ScalarExpr::var("cursor")),
            Box::new(ScalarExpr::u64(1)),
        ),
        body: vec![],
    });

    let report = verify_l2_core(&program);
    assert!(
        report.is_ok(),
        "unexpected diagnostics: {:?}",
        report.diagnostics()
    );
    let comments = report.constraint_comments();
    assert!(comments.contains("AddNoOverflow"));
    assert!(comments.contains("InRange"));
    assert!(comments.contains("Decreases"));

    let facts = report.facts().expect("accepted program should emit facts");
    assert!(facts
        .constraint_ids
        .iter()
        .any(|id| id.contains("read-add-no-overflow")));
    assert!(report.proof_obligations().iter().any(|obligation| {
        obligation.id == "VERIFIER-07"
            && obligation
                .constraint_ids
                .iter()
                .any(|id| id.contains("cursor-decreases"))
    }));
}

#[test]
fn verifier_08_facts_are_absent_for_rejected_programs() {
    let mut program = sample_program();
    program.body = vec![L2CoreStmt::AppendNull {
        builder: "missing".to_string(),
    }];

    let report = verify_l2_core(&program);
    assert!(!report.is_ok());
    assert!(report.facts().is_none());
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == FullVerificationCode::MissingOutputBuilder));
}

#[test]
fn verifier_07_constraint_ir_retains_stable_variants() {
    let report = verify_l2_core(&sample_program());
    let facts = report.facts().expect("accepted program should emit facts");
    assert!(facts
        .constraint_ids
        .iter()
        .any(|id| id.contains("read-in-range")));

    let variant_name = match (LoomConstraint::InRange {
        id: "sample".to_string(),
        value: loom_core::l2_core::constraints::ConstraintTerm::var("x"),
        lower: loom_core::l2_core::constraints::ConstraintTerm::int(0),
        upper_exclusive: loom_core::l2_core::constraints::ConstraintTerm::int(1),
    }) {
        LoomConstraint::InRange { .. } => "InRange",
        _ => unreachable!(),
    };
    assert_eq!(variant_name, "InRange");
}

#[test]
fn lean_rust_correspondence_matrix_matches_expected() {
    let cases = correspondence_cases();

    for case in &cases {
        assert_eq!(
            correspondence_classification(&case.program),
            case.expected,
            "classification mismatch for {}",
            case.id
        );
    }

    let lines = cases
        .iter()
        .map(correspondence_line)
        .collect::<Vec<_>>()
        .join("\n");

    if let Ok(path) = std::env::var("LOOM_WRITE_CORRESPONDENCE_REPORT") {
        fs::write(path, format!("{lines}\n")).expect("write correspondence report");
    }
}
