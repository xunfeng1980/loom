use arrow_schema::DataType;
use loom_core::l2_core::constraints::{ConstraintSet, ConstraintTerm, IntegerType, LoomConstraint};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, VerifiedArtifactFacts,
};

fn sample_bounded_copy_program() -> L2CoreProgram {
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
fn verifier_03_04_05_l2core_bounded_copy_program_is_representable() {
    let program = sample_bounded_copy_program();

    assert_eq!(program.artifact_version, 1);
    assert_eq!(program.capabilities.len(), 2);
    assert_eq!(program.resource_budget.max_rows, 4);

    match &program.body[0] {
        L2CoreStmt::ForRange { index, body, .. } => {
            assert_eq!(index, "i");
            assert!(matches!(body[0], L2CoreStmt::ReadInput { .. }));
            assert!(matches!(body[1], L2CoreStmt::AppendValue { .. }));
        }
        other => panic!("expected bounded ForRange, got {other:?}"),
    }
}

#[test]
fn verifier_10_verified_artifact_facts_record_lowering_preconditions() {
    let program = sample_bounded_copy_program();
    let facts = VerifiedArtifactFacts::for_program(
        &program,
        vec![
            "c-add-no-overflow".to_string(),
            "c-input-range".to_string(),
            "c-loop-decreases".to_string(),
        ],
        vec!["VERIFIER-10".to_string()],
    );

    assert_eq!(facts.artifact_version, 1);
    assert_eq!(facts.row_count_bound, Some(4));
    assert_eq!(facts.input_ranges.len(), 1);
    assert_eq!(facts.input_ranges[0].capability_id, "input0");
    assert_eq!(facts.output_schema.len(), 1);
    assert_eq!(facts.output_schema[0].arrow_type, DataType::Int32);
    assert_eq!(facts.loop_bounds[0].loop_id, "i");
    assert!(facts.constraint_ids.iter().any(|id| id == "c-input-range"));
    assert!(facts
        .proof_obligation_ids
        .iter()
        .any(|id| id == "VERIFIER-10"));
}

#[test]
fn verifier_07_constraint_comments_are_stable_and_smt_ready() {
    let mut constraints = ConstraintSet::new();
    assert!(constraints.is_empty());

    constraints.push(LoomConstraint::AddNoOverflow {
        id: "c-add-no-overflow".to_string(),
        left: ConstraintTerm::var("offset"),
        right: ConstraintTerm::var("width"),
        ty: IntegerType::UInt64,
    });
    constraints.push(LoomConstraint::InRange {
        id: "c-input-range".to_string(),
        value: ConstraintTerm::var("offset"),
        lower: ConstraintTerm::int(0),
        upper_exclusive: ConstraintTerm::var("input0.length"),
    });
    constraints.push(LoomConstraint::Decreases {
        id: "c-loop-decreases".to_string(),
        previous: ConstraintTerm::var("remaining_before"),
        next: ConstraintTerm::var("remaining_after"),
    });

    assert_eq!(constraints.iter().count(), 3);
    let text = constraints.to_smtlib_comments();

    assert!(text.contains("; loom-constraint c-add-no-overflow AddNoOverflow"));
    assert!(text.contains("; loom-constraint c-input-range InRange"));
    assert!(text.contains("; loom-constraint c-loop-decreases Decreases"));
    assert!(text.find("AddNoOverflow").unwrap() < text.find("InRange").unwrap());
    assert!(text.find("InRange").unwrap() < text.find("Decreases").unwrap());
}
