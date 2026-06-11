//! Tests for the K spec-oracle skip semantics introduced in Phase 48.
//!
//! These tests exercise the typed outcome enum and the unsupported-construct
//! predicate without requiring a working krun installation.

use loom_core::kloom_harness::{kloom_trace_for_program, KOracleOutcome};
use loom_core::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, L2DataType, OutputBuilderCapability, ResourceBudget,
    ScalarExpr, ScalarValue,
};

use std::sync::Mutex;

/// Process-wide lock for tests that mutate process environment variables
/// (PATH, LOOM_ALLOW_K_ORACLE_SKIP) or invoke krun.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn make_program(body: Vec<L2CoreStmt>) -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec![],
        optional_features: vec![],
        capabilities: vec![Capability::OutputBuilder(OutputBuilderCapability {
            id: "out".to_string(),
            arrow_type: L2DataType::Int32,
            nullable: false,
            max_events: 8,
        })],
        resource_budget: ResourceBudget {
            max_steps: 10,
            max_input_bytes_read: 0,
            max_scratch_bytes: 0,
            max_builder_events: 8,
            max_rows: 8,
            max_constraint_count: 0,
        },
        body,
    }
}

#[test]
fn min_expr_is_supported() {
    let _guard = ENV_LOCK.lock().unwrap();
    let program = make_program(vec![L2CoreStmt::AppendValue {
        builder: "out".to_string(),
        value: ScalarExpr::Min(
            Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            Box::new(ScalarExpr::Const(ScalarValue::Int32(2))),
        ),
    }]);
    let outcome = kloom_trace_for_program(&program).unwrap();
    assert!(
        !matches!(outcome, KOracleOutcome::UnsupportedProgram { .. }),
        "Min should be supported by kloom v4+, got {outcome:?}"
    );
}

#[test]
fn max_nested_in_add_is_supported() {
    let _guard = ENV_LOCK.lock().unwrap();
    let program = make_program(vec![L2CoreStmt::AppendValue {
        builder: "out".to_string(),
        value: ScalarExpr::Add(
            Box::new(ScalarExpr::Max(
                Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
                Box::new(ScalarExpr::Const(ScalarValue::Int32(2))),
            )),
            Box::new(ScalarExpr::Const(ScalarValue::Int32(3))),
        ),
    }]);
    let outcome = kloom_trace_for_program(&program).unwrap();
    assert!(
        !matches!(outcome, KOracleOutcome::UnsupportedProgram { .. }),
        "Max should be supported by kloom v4+, got {outcome:?}"
    );
}

#[test]
fn unsupported_bytes_constant() {
    let _guard = ENV_LOCK.lock().unwrap();
    let program = make_program(vec![L2CoreStmt::AppendValue {
        builder: "out".to_string(),
        value: ScalarExpr::Const(ScalarValue::Bytes(vec![0xAB, 0xCD])),
    }]);
    let outcome = kloom_trace_for_program(&program).unwrap();
    assert!(
        matches!(outcome, KOracleOutcome::UnsupportedProgram { ref reason } if reason.contains("Bytes")),
        "expected UnsupportedProgram for Bytes, got {outcome:?}"
    );
}

#[test]
fn min_inside_forrange_body_is_supported() {
    let _guard = ENV_LOCK.lock().unwrap();
    let program = make_program(vec![L2CoreStmt::ForRange {
        index: "i".to_string(),
        start: ScalarExpr::Const(ScalarValue::Int32(0)),
        end: ScalarExpr::Const(ScalarValue::Int32(2)),
        body: vec![L2CoreStmt::AppendValue {
            builder: "out".to_string(),
            value: ScalarExpr::Min(
                Box::new(ScalarExpr::Var("i".to_string())),
                Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            ),
        }],
    }]);
    let outcome = kloom_trace_for_program(&program).unwrap();
    assert!(
        !matches!(outcome, KOracleOutcome::UnsupportedProgram { .. }),
        "Min inside ForRange body should be supported, got {outcome:?}"
    );
}

#[test]
fn max_inside_cursorloop_body_is_supported() {
    let _guard = ENV_LOCK.lock().unwrap();
    let program = make_program(vec![L2CoreStmt::CursorLoop {
        cursor: "c".to_string(),
        limit: ScalarExpr::Const(ScalarValue::UInt64(2)),
        progress: ScalarExpr::Add(
            Box::new(ScalarExpr::Var("c".to_string())),
            Box::new(ScalarExpr::Const(ScalarValue::UInt64(1))),
        ),
        body: vec![L2CoreStmt::AppendValue {
            builder: "out".to_string(),
            value: ScalarExpr::Max(
                Box::new(ScalarExpr::Var("c".to_string())),
                Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            ),
        }],
    }]);
    let outcome = kloom_trace_for_program(&program).unwrap();
    assert!(
        !matches!(outcome, KOracleOutcome::UnsupportedProgram { .. }),
        "Max inside CursorLoop body should be supported, got {outcome:?}"
    );
}

#[test]
fn pure_append_int32_not_unsupported() {
    let _guard = ENV_LOCK.lock().unwrap();
    let program = make_program(vec![L2CoreStmt::AppendValue {
        builder: "out".to_string(),
        value: ScalarExpr::Const(ScalarValue::Int32(42)),
    }]);
    let outcome = kloom_trace_for_program(&program).unwrap();
    assert!(
        !matches!(outcome, KOracleOutcome::UnsupportedProgram { .. }),
        "pure-append Int32 should not be classified unsupported, got {outcome:?}"
    );
}

#[test]
fn krun_absent_with_skip_allowed() {
    let _guard = ENV_LOCK.lock().unwrap();

    let original_path = std::env::var_os("PATH");
    let original_skip = std::env::var_os("LOOM_ALLOW_K_ORACLE_SKIP");

    std::env::set_var("LOOM_ALLOW_K_ORACLE_SKIP", "1");
    std::env::remove_var("PATH");

    let program = make_program(vec![L2CoreStmt::AppendValue {
        builder: "out".to_string(),
        value: ScalarExpr::Const(ScalarValue::Int32(42)),
    }]);

    let outcome = kloom_trace_for_program(&program);

    // Restore env.
    match original_path {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
    match original_skip {
        Some(s) => std::env::set_var("LOOM_ALLOW_K_ORACLE_SKIP", s),
        None => std::env::remove_var("LOOM_ALLOW_K_ORACLE_SKIP"),
    }

    assert!(
        matches!(
            outcome,
            Ok(KOracleOutcome::SkippedRefereeAbsent { ref reason })
            if reason.contains("krun not found") || reason.contains("definition directory not found")
        ),
        "expected SkippedRefereeAbsent when krun absent with skip allowed, got {outcome:?}"
    );
}

#[test]
fn krun_absent_without_skip_is_hard_error() {
    let _guard = ENV_LOCK.lock().unwrap();

    let original_path = std::env::var_os("PATH");
    let original_skip = std::env::var_os("LOOM_ALLOW_K_ORACLE_SKIP");

    std::env::remove_var("LOOM_ALLOW_K_ORACLE_SKIP");
    std::env::remove_var("PATH");

    let program = make_program(vec![L2CoreStmt::AppendValue {
        builder: "out".to_string(),
        value: ScalarExpr::Const(ScalarValue::Int32(42)),
    }]);

    let outcome = kloom_trace_for_program(&program);

    match original_path {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
    match original_skip {
        Some(s) => std::env::set_var("LOOM_ALLOW_K_ORACLE_SKIP", s),
        None => std::env::remove_var("LOOM_ALLOW_K_ORACLE_SKIP"),
    }

    assert!(
        outcome.is_err(),
        "expected hard error when krun absent without skip env, got {outcome:?}"
    );
}
