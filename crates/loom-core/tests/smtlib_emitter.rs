use loom_core::l2_core::constraints::{
    ConstraintSet, ConstraintTerm, IntegerType, LoomConstraint,
};
use loom_core::solver::{
    emit_required_qfbv_script, SmtLibScript, SmtLibScriptFamily, SolverRawResult, SolverTheory,
};

fn sample_constraints() -> ConstraintSet {
    let mut constraints = ConstraintSet::new();
    constraints.push(LoomConstraint::InRange {
        id: "$.body[0].read-in-range".to_string(),
        value: ConstraintTerm::Add(
            Box::new(ConstraintTerm::var("input.offset")),
            Box::new(ConstraintTerm::var("read.width")),
        ),
        lower: ConstraintTerm::int(0),
        upper_exclusive: ConstraintTerm::int(4097),
    });
    constraints.push(LoomConstraint::AddNoOverflow {
        id: "$.body[0].read-add-no-overflow".to_string(),
        left: ConstraintTerm::var("input.offset"),
        right: ConstraintTerm::var("read.width"),
        ty: IntegerType::UInt64,
    });
    constraints.push(LoomConstraint::Decreases {
        id: "$.body[1].cursor-decreases".to_string(),
        previous: ConstraintTerm::var("cursor.remaining_before"),
        next: ConstraintTerm::var("cursor.remaining_after"),
    });
    constraints
}

#[test]
fn required_script_is_qfbv_and_bitwuzla_ready() {
    let script = emit_required_qfbv_script("copy-i32.required", &sample_constraints());

    assert_eq!(script.family, SmtLibScriptFamily::Required);
    assert_eq!(script.logic, SolverTheory::QfBv);
    assert_eq!(script.expected_success, SolverRawResult::Unsat);
    assert!(script.text.contains("(set-logic QF_BV)"));
    assert!(script.text.contains("; loom-smt-primary-backend bitwuzla"));
    assert!(!script.text.contains("QF_LIA"));
}

#[test]
fn script_uses_stable_symbols_and_named_bad_states() {
    let script = emit_required_qfbv_script("copy-i32.required", &sample_constraints());

    assert!(script
        .text
        .contains("(declare-const loom_cursor_remaining_after (_ BitVec 64))"));
    assert!(script
        .text
        .contains("(declare-const loom_cursor_remaining_before (_ BitVec 64))"));
    assert!(script
        .text
        .contains("(declare-const loom_input_offset (_ BitVec 64))"));
    assert!(script
        .text
        .contains("(declare-const loom_read_width (_ BitVec 64))"));
    assert!(script.text.contains(":named loom_bad___body_0__read_add_no_overflow_0"));
    assert!(script.text.contains(":named loom_bad___body_0__read_in_range_1"));
    assert!(script.text.contains(":named loom_bad___body_1__cursor_decreases_2"));
}

#[test]
fn offset_width_bounds_include_unsigned_overflow_bad_state() {
    let script = emit_required_qfbv_script("copy-i32.required", &sample_constraints());

    assert!(script.text.contains("(bvadd loom_input_offset loom_read_width)"));
    assert!(script
        .text
        .contains("(bvult (bvadd loom_input_offset loom_read_width) loom_input_offset)"));
    assert!(script.text.contains("bvult"));
    assert!(script.text.contains("bvugt") || script.text.contains("(not (bvult"));
}

#[test]
fn emission_is_byte_stable() {
    let first = emit_required_qfbv_script("copy-i32.required", &sample_constraints());
    let second = emit_required_qfbv_script("copy-i32.required", &sample_constraints());

    assert_eq!(first.text, second.text);
    assert_eq!(first.deterministic_id, second.deterministic_id);
    assert!(!first.text.contains("2026-"));
    assert!(!first.text.contains("/Users/"));
}

#[test]
fn cross_check_scripts_are_distinct_from_required_scripts() {
    let cross_check = SmtLibScript::cross_check_qflia(
        "copy-i32.cross-check",
        "(set-logic QF_LIA)\n(check-sat)\n".to_string(),
        vec!["ob.bounds.0001".to_string()],
    );

    assert_eq!(cross_check.family, SmtLibScriptFamily::CrossCheck);
    assert_eq!(cross_check.logic, SolverTheory::QfLia);
    assert_ne!(
        cross_check.family,
        emit_required_qfbv_script("copy-i32.required", &sample_constraints()).family
    );
}
