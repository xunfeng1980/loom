use arrow::datatypes::DataType;
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use loom_ffi::duckdb_runtime::{
    plan_duckdb_runtime, prepare_duckdb_runtime, DuckDbProjection, DuckDbRouteDecision,
    DuckDbRuntimePlanInput, DuckDbRuntimePolicy, DuckDbTestNativeFacts,
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

fn primitive_value_bytes(data_type: &DataType, row_count: usize) -> Vec<u8> {
    match data_type {
        DataType::Int32 => (0..row_count as i32).flat_map(i32::to_le_bytes).collect(),
        DataType::Int64 => (0..row_count as i64).flat_map(i64::to_le_bytes).collect(),
        DataType::Float32 => (0..row_count)
            .map(|idx| idx as f32 + 0.25)
            .flat_map(f32::to_le_bytes)
            .collect(),
        DataType::Float64 => (0..row_count)
            .map(|idx| idx as f64 + 0.5)
            .flat_map(f64::to_le_bytes)
            .collect(),
        other => panic!("unsupported primitive fixture type {other:?}"),
    }
}

fn primitive_byte_width(data_type: &DataType) -> usize {
    match data_type {
        DataType::Int32 | DataType::Float32 => 4,
        DataType::Int64 | DataType::Float64 => 8,
        other => panic!("unsupported primitive fixture type {other:?}"),
    }
}

fn primitive_table_lmc1(row_count: usize, columns: Vec<(&str, DataType)>) -> Vec<u8> {
    let table = TableDescription {
        row_count,
        columns: columns
            .into_iter()
            .map(|(name, data_type)| TableColumn {
                name: name.to_string(),
                layout: LayoutDescription {
                    root: LayoutNode::Raw {
                        data: primitive_value_bytes(&data_type, row_count),
                        elem_size: primitive_byte_width(&data_type) as u8,
                        count: row_count,
                    },
                    data_type,
                    row_count,
                },
            })
            .collect(),
    };
    let payload = encode_table_payload(&table).expect("valid primitive table payload");
    wrap_table_payload(&payload).expect("valid primitive LMC1 table")
}

fn bitpack_i32_lmc1(row_count: usize) -> Vec<u8> {
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::BitPack {
            values_buf: vec![0; 128],
            bit_width: 1,
            offset: 0,
            count: row_count,
            validity: None,
            all_null: false,
        },
        row_count,
    };
    let payload = encode_layout_payload(&desc);
    wrap_layout_payload(&payload).expect("valid bitpack LMC1 layout")
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
                test_jit_value_buffers: None,
            }),
        },
    }
}

mod equivalence_matrix {
    use super::*;
    use loom_native_melior::backend::NativeBackendCancellation;

    fn prepare_native(
        input: DuckDbRuntimePlanInput,
    ) -> loom_ffi::duckdb_runtime::DuckDbPreparedRoute {
        let plan = plan_duckdb_runtime(input).expect("native runtime plan");
        let route = prepare_duckdb_runtime(&plan, NativeBackendCancellation::default());
        assert_eq!(route.decision, DuckDbRouteDecision::NativeCandidate);
        route
    }

    #[test]
    fn raw_non_null_i32_single_column_matches_artifact_value_bytes() {
        let route = prepare_native(DuckDbRuntimePlanInput {
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
        });

        assert_eq!(route.native_buffers.len(), 1);
        assert_eq!(route.native_buffers[0].builder_id, "col0");
        assert_eq!(route.native_buffers[0].arrow_type, DataType::Int32);
        assert_eq!(route.native_buffers[0].value_buffer.len(), 16);
        assert_eq!(
            route.native_buffers[0].value_buffer,
            primitive_value_bytes(&DataType::Int32, 4)
        );
    }

    #[test]
    fn raw_non_null_primitive_table_matches_artifact_value_bytes_by_column() {
        let column_types = vec![
            DataType::Int32,
            DataType::Int64,
            DataType::Float32,
            DataType::Float64,
        ];
        let expected_buffers = column_types
            .iter()
            .map(|data_type| primitive_value_bytes(data_type, 4))
            .collect::<Vec<_>>();
        let route = prepare_native(DuckDbRuntimePlanInput {
            artifact_bytes: primitive_table_lmc1(
                4,
                vec![
                    ("i32s", DataType::Int32),
                    ("i64s", DataType::Int64),
                    ("f32s", DataType::Float32),
                    ("f64s", DataType::Float64),
                ],
            ),
            projection: DuckDbProjection::All,
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: false,
                test_native_facts: Some(DuckDbTestNativeFacts {
                    row_count: 4,
                    columns: column_types.clone(),
                    test_jit_value_buffers: None,
                }),
            },
        });

        assert_eq!(route.native_buffers.len(), column_types.len());
        for (idx, data_type) in column_types.iter().enumerate() {
            let buffer = &route.native_buffers[idx];
            assert_eq!(buffer.builder_id, format!("col{idx}"));
            assert_eq!(&buffer.arrow_type, data_type);
            assert_eq!(buffer.value_buffer, expected_buffers[idx]);
        }
    }

    #[test]
    fn projected_table_output_order_maps_source_columns_before_buffer_comparison() {
        let column_types = vec![
            DataType::Int32,
            DataType::Int64,
            DataType::Float32,
            DataType::Float64,
        ];
        let expected_buffers = column_types
            .iter()
            .map(|data_type| primitive_value_bytes(data_type, 4))
            .collect::<Vec<_>>();
        let input = DuckDbRuntimePlanInput {
            artifact_bytes: primitive_table_lmc1(
                4,
                vec![
                    ("i32s", DataType::Int32),
                    ("i64s", DataType::Int64),
                    ("f32s", DataType::Float32),
                    ("f64s", DataType::Float64),
                ],
            ),
            projection: DuckDbProjection::Columns(vec![3, 0, 2]),
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: false,
                test_native_facts: Some(DuckDbTestNativeFacts {
                    row_count: 4,
                    columns: column_types.clone(),
                    test_jit_value_buffers: None,
                }),
            },
        };
        let plan = plan_duckdb_runtime(input).expect("projected native runtime plan");
        assert_eq!(plan.output_to_source, vec![3, 0, 2]);

        let route = prepare_duckdb_runtime(&plan, NativeBackendCancellation::default());
        assert_eq!(route.decision, DuckDbRouteDecision::NativeCandidate);
        let projected = plan
            .output_to_source
            .iter()
            .map(|source| {
                let source = *source as usize;
                (
                    route.native_buffers[source].arrow_type.clone(),
                    route.native_buffers[source].value_buffer.clone(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            projected,
            vec![
                (DataType::Float64, expected_buffers[3].clone()),
                (DataType::Int32, expected_buffers[0].clone()),
                (DataType::Float32, expected_buffers[2].clone()),
            ]
        );
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

    #[test]
    fn table_projection_without_test_native_facts_uses_table_column_count() {
        let input = DuckDbRuntimePlanInput {
            artifact_bytes: two_column_table_lmc1(4),
            projection: DuckDbProjection::Columns(vec![1, 0]),
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: true,
                test_native_facts: None,
            },
        };
        let report = plan_duckdb_runtime(input).expect("table projection runtime plan");

        assert_eq!(report.output_to_source, vec![1, 0]);
        assert!(
            report
                .cache_key
                .canonical_input
                .contains("projection=columns:1>0,0>1"),
            "table projection should be in runtime cache input: {}",
            report.cache_key.canonical_input
        );
    }
}

mod prepare_routes {
    use super::*;
    use loom_core::artifact_verifier::{ArtifactVerificationStatus, ConstraintDischargeStatus};
    use loom_core::runtime_abi::{
        decide_runtime_execution, RuntimeDecisionInput, RuntimeEmissionDisposition,
        RuntimeExecutionDecision, RuntimeFallbackPolicy, RuntimeLoweringDisposition,
        RuntimeReaderSupport, RuntimeSafetyPolicy, UnsupportedPredicatePolicy,
    };
    use loom_native_melior::backend::NativeBackendCancellation;

    fn native_plan_with_test_jit(
        test_jit_value_buffers: Option<Vec<Vec<u8>>>,
    ) -> loom_ffi::duckdb_runtime::DuckDbRuntimePlanReport {
        let mut input = native_input();
        input
            .policy
            .test_native_facts
            .as_mut()
            .expect("test facts")
            .test_jit_value_buffers = test_jit_value_buffers;
        plan_duckdb_runtime(input).expect("native runtime plan")
    }

    #[test]
    fn backend_prepare_runs_only_for_native_candidate_without_runtime_diagnostics() {
        let native = native_plan_with_test_jit(None);
        assert!(native.runtime_plan.diagnostics.is_empty());
        let prepared = prepare_duckdb_runtime(&native, NativeBackendCancellation::default());
        assert!(prepared.backend_report.is_some());

        let fallback = plan_duckdb_runtime(DuckDbRuntimePlanInput {
            artifact_bytes: raw_i32_lmc1(4),
            projection: DuckDbProjection::All,
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: true,
                test_native_facts: None,
            },
        })
        .expect("fallback runtime plan");
        assert_eq!(fallback.decision, DuckDbRouteDecision::InterpreterFallback);
        let prepared = prepare_duckdb_runtime(&fallback, NativeBackendCancellation::default());
        assert_eq!(prepared.decision, DuckDbRouteDecision::InterpreterFallback);
        assert!(prepared.backend_report.is_none());
        assert!(prepared.native_buffers.is_empty());
    }

    #[test]
    fn cancelled_prepare_returns_cancelled_diagnostic_and_no_native_buffers() {
        let native = native_plan_with_test_jit(None);
        let prepared = prepare_duckdb_runtime(
            &native,
            NativeBackendCancellation::cancelled("duckdb interrupt"),
        );

        assert_eq!(prepared.decision, DuckDbRouteDecision::Cancelled);
        assert!(prepared.native_buffers.is_empty());
        assert!(prepared
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "cancelled"));
        assert!(prepared
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "cache-non-cacheable"));
    }

    #[test]
    fn native_output_mismatch_fails_closed_without_interpreter_fallback() {
        let native = native_plan_with_test_jit(Some(vec![vec![0xff; 16]]));
        let prepared = prepare_duckdb_runtime(&native, NativeBackendCancellation::default());

        assert_eq!(prepared.decision, DuckDbRouteDecision::FailClosed);
        assert!(prepared.native_buffers.is_empty());
        assert!(prepared
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "native-output-mismatch"));
        assert_ne!(prepared.decision, DuckDbRouteDecision::InterpreterFallback);
    }

    #[test]
    fn skipped_or_failed_toolchain_is_diagnostic_only_without_native_buffers() {
        let native = native_plan_with_test_jit(None);
        let prepared = prepare_duckdb_runtime(&native, NativeBackendCancellation::default());

        if prepared.native_buffers.is_empty() {
            assert!(prepared.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "toolchain-skipped" || diagnostic.code == "toolchain-failed"
            }));
        } else {
            assert_eq!(prepared.decision, DuckDbRouteDecision::NativeCandidate);
        }
    }

    #[test]
    fn unsupported_strings_and_compressed_layouts_are_fallback_or_fail_closed_evidence() {
        let string_fallback = DuckDbRuntimePlanInput {
            artifact_bytes: raw_i32_lmc1(4),
            projection: DuckDbProjection::All,
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: true,
                test_native_facts: Some(DuckDbTestNativeFacts {
                    row_count: 4,
                    columns: vec![DataType::Utf8],
                    test_jit_value_buffers: None,
                }),
            },
        };
        let report = plan_duckdb_runtime(string_fallback).expect("string fallback plan");
        assert_eq!(report.decision, DuckDbRouteDecision::InterpreterFallback);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported-type"));
        let prepared = prepare_duckdb_runtime(&report, NativeBackendCancellation::default());
        assert_eq!(prepared.decision, DuckDbRouteDecision::InterpreterFallback);
        assert!(prepared.native_buffers.is_empty());

        let compressed_fallback = DuckDbRuntimePlanInput {
            artifact_bytes: bitpack_i32_lmc1(4),
            projection: DuckDbProjection::All,
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: true,
                test_native_facts: None,
            },
        };
        let report = plan_duckdb_runtime(compressed_fallback).expect("compressed fallback plan");
        assert_eq!(report.decision, DuckDbRouteDecision::InterpreterFallback);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "unsupported-kernel"
                || diagnostic.code == "missing-l2-facts"
                || diagnostic.code == "lowering-unsupported"
        }));
        let prepared = prepare_duckdb_runtime(&report, NativeBackendCancellation::default());
        assert_eq!(prepared.decision, DuckDbRouteDecision::InterpreterFallback);
        assert!(prepared.native_buffers.is_empty());

        let string_strict = DuckDbRuntimePlanInput {
            artifact_bytes: raw_i32_lmc1(4),
            projection: DuckDbProjection::All,
            policy: DuckDbRuntimePolicy {
                allow_interpreter_fallback: false,
                test_native_facts: Some(DuckDbTestNativeFacts {
                    row_count: 4,
                    columns: vec![DataType::Utf8],
                    test_jit_value_buffers: None,
                }),
            },
        };
        let report = plan_duckdb_runtime(string_strict).expect("string strict plan");
        assert_eq!(report.decision, DuckDbRouteDecision::FailClosed);
        let prepared = prepare_duckdb_runtime(&report, NativeBackendCancellation::default());
        assert_eq!(prepared.decision, DuckDbRouteDecision::FailClosed);
        assert!(prepared.native_buffers.is_empty());
    }

    #[test]
    fn runtime_helper_injection_keeps_projection_predicate_and_split_routes_fail_closed() {
        let base = RuntimeDecisionInput {
            artifact_status: ArtifactVerificationStatus::Accepted,
            constraint_status: ConstraintDischargeStatus::Discharged,
            production_lowering_supported: true,
            reader_support: RuntimeReaderSupport::Accepted,
            emission_disposition: RuntimeEmissionDisposition::CanonicalTable,
            lowering_disposition: RuntimeLoweringDisposition::ProductionLoweringSupported,
            projection_supported: true,
            predicate_supported: true,
            split_supported: true,
            concurrency_safe: true,
            policy: RuntimeSafetyPolicy {
                fallback: RuntimeFallbackPolicy::FailClosedOnly,
                unsupported_predicate: UnsupportedPredicatePolicy::FailClosed,
                ..RuntimeSafetyPolicy::default()
            },
        };

        let mut unsupported_projection = base.clone();
        unsupported_projection.projection_supported = false;
        let report = decide_runtime_execution(&unsupported_projection);
        assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "unsupported-projection"));

        let mut unsupported_predicate = base.clone();
        unsupported_predicate.predicate_supported = false;
        let report = decide_runtime_execution(&unsupported_predicate);
        assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "unsupported-predicate"));

        let mut unsupported_split = base;
        unsupported_split.split_supported = false;
        let report = decide_runtime_execution(&unsupported_split);
        assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "invalid-split"));
    }
}
