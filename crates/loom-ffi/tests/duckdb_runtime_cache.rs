use arrow::datatypes::DataType;
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use loom_ffi::duckdb_runtime::{
    duckdb_runtime_clear_native_preparation_cache_for_test,
    duckdb_runtime_corrupt_cached_canonical_input_for_test, plan_duckdb_runtime,
    prepare_duckdb_runtime, DuckDbNativeBuffer, DuckDbPreparedRoute, DuckDbProjection,
    DuckDbRouteDecision, DuckDbRuntimeDiagnostic, DuckDbRuntimePlanInput, DuckDbRuntimePlanReport,
    DuckDbRuntimePolicy, DuckDbTestNativeFacts,
};
use loom_native_melior::backend::NativeBackendCancellation;
use std::sync::{Mutex, MutexGuard};

static CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn raw_i32_lmc1(row_count: u64) -> Vec<u8> {
    let values = (0..row_count as i32)
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: values,
            elem_size: 4,
            count: row_count as usize,
        },
        row_count: row_count as usize,
    };
    let payload = encode_layout_payload(&desc);
    wrap_layout_payload(&payload).expect("valid LMC1 layout")
}

fn two_column_table_lmc1(row_count: usize) -> Vec<u8> {
    let i32_values = (0..row_count as i32)
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let i64_values = (0..row_count as i64)
        .flat_map(i64::to_le_bytes)
        .collect::<Vec<_>>();
    let table = TableDescription {
        row_count,
        columns: vec![
            TableColumn {
                name: "a".to_string(),
                layout: LayoutDescription {
                    data_type: DataType::Int32,
                    root: LayoutNode::Raw {
                        data: i32_values,
                        elem_size: 4,
                        count: row_count,
                    },
                    row_count,
                },
            },
            TableColumn {
                name: "b".to_string(),
                layout: LayoutDescription {
                    data_type: DataType::Int64,
                    root: LayoutNode::Raw {
                        data: i64_values,
                        elem_size: 8,
                        count: row_count,
                    },
                    row_count,
                },
            },
        ],
    };
    let payload = encode_table_payload(&table).expect("valid table payload");
    wrap_table_payload(&payload).expect("valid LMC1 table")
}

fn native_input_with_buffers(buffers: Vec<Vec<u8>>) -> DuckDbRuntimePlanInput {
    DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32],
                test_jit_value_buffers: Some(buffers),
            }),
        },
    }
}

fn table_native_input_with_projection(projection: DuckDbProjection) -> DuckDbRuntimePlanInput {
    DuckDbRuntimePlanInput {
        artifact_bytes: two_column_table_lmc1(4),
        projection,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32, DataType::Int64],
                test_jit_value_buffers: None,
            }),
        },
    }
}

fn native_plan() -> DuckDbRuntimePlanReport {
    plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32],
                test_jit_value_buffers: None,
            }),
        },
    })
    .expect("native plan")
}

fn prepare(plan: &DuckDbRuntimePlanReport) -> Vec<DuckDbRuntimeDiagnostic> {
    prepare_route(plan).diagnostics
}

fn prepare_route(plan: &DuckDbRuntimePlanReport) -> DuckDbPreparedRoute {
    let route = prepare_duckdb_runtime(plan, NativeBackendCancellation::default());
    assert_eq!(route.decision, DuckDbRouteDecision::NativeCandidate);
    assert!(
        !route.native_buffers.is_empty(),
        "eligible native route should expose buffers"
    );
    route
}

fn diagnostic_codes(diagnostics: &[DuckDbRuntimeDiagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn isolated_cache() -> MutexGuard<'static, ()> {
    let guard = CACHE_TEST_LOCK
        .lock()
        .expect("cache integration test mutex poisoned");
    duckdb_runtime_clear_native_preparation_cache_for_test();
    guard
}

fn buffer_fingerprints(buffers: &[DuckDbNativeBuffer]) -> Vec<(String, DataType, Vec<u8>)> {
    buffers
        .iter()
        .map(|buffer| {
            (
                buffer.builder_id.clone(),
                buffer.arrow_type.clone(),
                buffer.value_buffer.clone(),
            )
        })
        .collect()
}

#[test]
fn identical_native_candidate_prepares_miss_insert_then_hit() {
    let _guard = isolated_cache();
    let plan = native_plan();

    let first = prepare_route(&plan);
    let first_codes = diagnostic_codes(&first.diagnostics);
    assert!(first_codes.contains(&"cache-miss"));
    assert!(first_codes.contains(&"cache-inserted"));
    assert!(!first_codes.contains(&"cache-hit"));

    let second = prepare_route(&plan);
    let second_codes = diagnostic_codes(&second.diagnostics);
    assert!(second_codes.contains(&"cache-hit"));
    assert!(!second_codes.contains(&"cache-inserted"));
    assert_eq!(
        buffer_fingerprints(&second.native_buffers),
        buffer_fingerprints(&first.native_buffers),
        "cache hits must replay the same validated native buffer metadata and bytes"
    );
}

#[test]
fn projection_and_policy_drift_miss_instead_of_reusing_prior_entry() {
    let _guard = isolated_cache();

    let all_columns =
        plan_duckdb_runtime(table_native_input_with_projection(DuckDbProjection::All))
            .expect("all columns native plan");
    let all_diagnostics = prepare(&all_columns);
    let all_codes = diagnostic_codes(&all_diagnostics);
    assert!(all_codes.contains(&"cache-miss"));
    assert!(all_codes.contains(&"cache-inserted"));

    let projected = plan_duckdb_runtime(table_native_input_with_projection(
        DuckDbProjection::Columns(vec![1, 0]),
    ))
    .expect("projected native plan");
    let projected_diagnostics = prepare(&projected);
    let projected_codes = diagnostic_codes(&projected_diagnostics);
    assert!(projected_codes.contains(&"cache-miss"));
    assert!(!projected_codes.contains(&"cache-hit"));

    let mut policy_drift = DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32],
                test_jit_value_buffers: None,
            }),
        },
    };
    policy_drift.policy.allow_interpreter_fallback = true;
    let policy_drift = plan_duckdb_runtime(policy_drift).expect("policy drift native plan");
    let policy_diagnostics = prepare(&policy_drift);
    let policy_codes = diagnostic_codes(&policy_diagnostics);
    assert!(policy_codes.contains(&"cache-miss"));
    assert!(!policy_codes.contains(&"cache-hit"));
}

#[test]
fn canonical_input_mismatch_for_same_stable_id_reports_key_mismatch() {
    let _guard = isolated_cache();
    let plan = native_plan();

    let inserted_diagnostics = prepare(&plan);
    let inserted_codes = diagnostic_codes(&inserted_diagnostics);
    assert!(inserted_codes.contains(&"cache-inserted"));

    assert!(duckdb_runtime_corrupt_cached_canonical_input_for_test(
        &plan.cache_key.stable_id,
        "corrupted canonical cache input"
    ));

    let mismatch_diagnostics = prepare(&plan);
    let mismatch_codes = diagnostic_codes(&mismatch_diagnostics);
    assert!(mismatch_codes.contains(&"cache-key-mismatch"));
    assert!(!mismatch_codes.contains(&"cache-hit"));
}

#[test]
fn unsafe_routes_are_non_cacheable_and_do_not_seed_hits() {
    let _guard = isolated_cache();
    let plan = native_plan();

    let cancelled = prepare_duckdb_runtime(
        &plan,
        NativeBackendCancellation::cancelled("duckdb interrupt"),
    );
    assert_eq!(cancelled.decision, DuckDbRouteDecision::Cancelled);
    assert!(diagnostic_codes(&cancelled.diagnostics).contains(&"cache-non-cacheable"));

    let mismatch = plan_duckdb_runtime(native_input_with_buffers(vec![vec![0xff; 16]]))
        .expect("mismatch plan");
    let mismatch = prepare_duckdb_runtime(&mismatch, NativeBackendCancellation::default());
    assert_eq!(mismatch.decision, DuckDbRouteDecision::FailClosed);
    let mismatch_codes = diagnostic_codes(&mismatch.diagnostics);
    assert!(mismatch_codes.contains(&"native-output-mismatch"));
    assert!(mismatch_codes.contains(&"cache-non-cacheable"));

    let mut fallback_input = DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32],
                test_jit_value_buffers: None,
            }),
        },
    };
    fallback_input.policy.test_native_facts = None;
    fallback_input.policy.allow_interpreter_fallback = true;
    let fallback = plan_duckdb_runtime(fallback_input).expect("fallback plan");
    assert_eq!(fallback.decision, DuckDbRouteDecision::InterpreterFallback);
    let fallback = prepare_duckdb_runtime(&fallback, NativeBackendCancellation::default());
    assert_eq!(fallback.decision, DuckDbRouteDecision::InterpreterFallback);
    assert!(diagnostic_codes(&fallback.diagnostics).contains(&"cache-non-cacheable"));

    let eligible = prepare(&plan);
    assert!(diagnostic_codes(&eligible).contains(&"cache-miss"));
    assert!(!diagnostic_codes(&eligible).contains(&"cache-hit"));
}

#[test]
fn cache_hit_reuses_preparation_evidence_but_still_validates_output() {
    let _guard = isolated_cache();
    let plan = native_plan();
    assert!(diagnostic_codes(&prepare(&plan)).contains(&"cache-inserted"));

    let mut mismatched_after_hit = plan.clone();
    mismatched_after_hit.test_jit_value_buffers = Some(vec![vec![0xff; 16]]);
    let route = prepare_duckdb_runtime(&mismatched_after_hit, NativeBackendCancellation::default());

    assert_eq!(route.decision, DuckDbRouteDecision::FailClosed);
    assert!(route.native_buffers.is_empty());
    let codes = diagnostic_codes(&route.diagnostics);
    assert!(codes.contains(&"cache-hit"));
    assert!(codes.contains(&"native-output-mismatch"));
    assert!(codes.contains(&"cache-non-cacheable"));
}

#[test]
fn native_output_mismatch_never_poisons_later_valid_cache_replay() {
    let _guard = isolated_cache();

    let mismatch = plan_duckdb_runtime(native_input_with_buffers(vec![vec![0xff; 16]]))
        .expect("mismatch plan");
    let mismatch = prepare_duckdb_runtime(&mismatch, NativeBackendCancellation::default());
    assert_eq!(mismatch.decision, DuckDbRouteDecision::FailClosed);
    assert!(mismatch.native_buffers.is_empty());
    let mismatch_codes = diagnostic_codes(&mismatch.diagnostics);
    assert!(mismatch_codes.contains(&"native-output-mismatch"));
    assert!(mismatch_codes.contains(&"cache-non-cacheable"));
    assert!(!mismatch_codes.contains(&"cache-inserted"));

    let plan = native_plan();
    let first_valid = prepare_route(&plan);
    let first_codes = diagnostic_codes(&first_valid.diagnostics);
    assert!(first_codes.contains(&"cache-miss"));
    assert!(first_codes.contains(&"cache-inserted"));
    assert!(!first_codes.contains(&"cache-hit"));

    let replay = prepare_route(&plan);
    let replay_codes = diagnostic_codes(&replay.diagnostics);
    assert!(replay_codes.contains(&"cache-hit"));
    assert_eq!(
        buffer_fingerprints(&replay.native_buffers),
        buffer_fingerprints(&first_valid.native_buffers)
    );
}

#[test]
fn repeated_post_error_scans_are_deterministic_and_emit_no_partial_output() {
    let _guard = isolated_cache();
    let plan = native_plan();

    for _ in 0..2 {
        let cancelled = prepare_duckdb_runtime(
            &plan,
            NativeBackendCancellation::cancelled("duckdb interrupt"),
        );
        assert_eq!(cancelled.decision, DuckDbRouteDecision::Cancelled);
        assert!(cancelled.native_buffers.is_empty());
        let codes = diagnostic_codes(&cancelled.diagnostics);
        assert!(codes.contains(&"cache-non-cacheable"));
        assert!(codes.contains(&"cancelled"));
        assert!(!codes.contains(&"cache-hit"));
        assert!(!codes.contains(&"cache-inserted"));
    }

    let valid_after_cancel = prepare_route(&plan);
    let valid_codes = diagnostic_codes(&valid_after_cancel.diagnostics);
    assert!(valid_codes.contains(&"cache-miss"));
    assert!(valid_codes.contains(&"cache-inserted"));

    let mismatch_plan = plan_duckdb_runtime(native_input_with_buffers(vec![vec![0xff; 16]]))
        .expect("mismatch plan");
    for _ in 0..2 {
        let mismatch = prepare_duckdb_runtime(&mismatch_plan, NativeBackendCancellation::default());
        assert_eq!(mismatch.decision, DuckDbRouteDecision::FailClosed);
        assert!(mismatch.native_buffers.is_empty());
        let codes = diagnostic_codes(&mismatch.diagnostics);
        assert!(codes.contains(&"cache-hit"));
        assert!(codes.contains(&"native-output-mismatch"));
        assert!(codes.contains(&"cache-non-cacheable"));
        assert!(!codes.contains(&"cache-inserted"));
    }

    let replay = prepare_route(&plan);
    let replay_codes = diagnostic_codes(&replay.diagnostics);
    assert!(replay_codes.contains(&"cache-hit"));
    assert_eq!(
        buffer_fingerprints(&replay.native_buffers),
        buffer_fingerprints(&valid_after_cancel.native_buffers)
    );
}
