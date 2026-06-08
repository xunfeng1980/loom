use arrow::datatypes::DataType;
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_ffi::duckdb_runtime::{
    plan_duckdb_runtime, DuckDbProjection, DuckDbRouteDecision, DuckDbRuntimePlanInput,
    DuckDbRuntimePolicy, DuckDbTestNativeFacts,
};

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

fn native_input() -> DuckDbRuntimePlanInput {
    DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
            test_native_facts: Some(DuckDbTestNativeFacts {
                row_count: 4,
                columns: vec![DataType::Int32],
            }),
        },
    }
}

mod runtime_planning {
    use super::*;
    use loom_core::runtime_abi::{
        ConcurrencyPolicy, PredicateEnvelope, ProjectionSet, SplitDescriptor,
    };

    #[test]
    fn all_columns_native_candidate_uses_no_predicate_full_scan_single_worker() {
        let report = plan_duckdb_runtime(native_input()).expect("runtime plan");

        assert_eq!(report.decision, DuckDbRouteDecision::NativeCandidate);
        assert_eq!(report.decision.as_str(), "native-candidate");
        assert_eq!(report.runtime_plan.projection, ProjectionSet::All);
        assert_eq!(report.runtime_plan.predicate, PredicateEnvelope::None);
        assert_eq!(
            report.runtime_plan.split,
            SplitDescriptor::FullScan { row_count: 4 }
        );
        assert_eq!(report.policy.concurrency, ConcurrencyPolicy::SingleWorker);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "test-native-facts"));
    }

    #[test]
    fn projected_source_columns_preserve_output_order_and_reject_bad_mappings() {
        let mut input = native_input();
        input.projection = DuckDbProjection::Columns(vec![1, 0]);
        input.policy.test_native_facts.as_mut().unwrap().columns =
            vec![DataType::Int32, DataType::Int64];
        let report = plan_duckdb_runtime(input).expect("projected runtime plan");

        assert_eq!(report.output_to_source, vec![1, 0]);
        match report.runtime_plan.projection {
            ProjectionSet::Columns(columns) => {
                assert_eq!(columns[0].source_index, 1);
                assert_eq!(columns[0].output_index, 0);
                assert_eq!(columns[1].source_index, 0);
                assert_eq!(columns[1].output_index, 1);
            }
            other => panic!("expected projected columns, got {other:?}"),
        }

        let mut duplicate = native_input();
        duplicate.projection = DuckDbProjection::Columns(vec![0, 0]);
        let err = plan_duckdb_runtime(duplicate).expect_err("duplicate source rejected");
        assert!(err
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported-projection"));

        let mut out_of_range = native_input();
        out_of_range.projection = DuckDbProjection::Columns(vec![3]);
        let err = plan_duckdb_runtime(out_of_range).expect_err("out of range source rejected");
        assert!(err
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported-projection"));
    }

    #[test]
    fn unsupported_native_lowering_uses_policy_controlled_fallback() {
        let unsupported = DuckDbRuntimePlanInput {
            artifact_bytes: raw_i32_lmc1(4),
            projection: DuckDbProjection::All,
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: true,
                test_native_facts: None,
            },
        };
        let report = plan_duckdb_runtime(unsupported).expect("fallback runtime plan");

        assert_eq!(report.decision, DuckDbRouteDecision::InterpreterFallback);
        assert_eq!(report.decision.as_str(), "interpreter-fallback");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "lowering-unsupported"));

        let strict = DuckDbRuntimePlanInput {
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: false,
                test_native_facts: None,
            },
            ..native_input()
        };
        let report = plan_duckdb_runtime(strict).expect("strict runtime plan");
        assert_eq!(report.decision, DuckDbRouteDecision::FailClosed);
        assert_eq!(report.decision.as_str(), "fail-closed");
    }

    #[test]
    fn planning_never_sets_predicate_pushdown() {
        let mut input = native_input();
        input.projection = DuckDbProjection::Columns(vec![0]);
        let report = plan_duckdb_runtime(input).expect("runtime plan");

        assert_eq!(report.runtime_plan.predicate, PredicateEnvelope::None);
        assert!(report.cache_key.canonical_input.contains("predicate=none"));
    }
}
