use arrow_schema::DataType;
use loom_core::full_verifier::{verify_l2_core, FullVerificationCode};
use loom_core::l2_core::constraints::LoomConstraint;
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, ScalarValue,
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
