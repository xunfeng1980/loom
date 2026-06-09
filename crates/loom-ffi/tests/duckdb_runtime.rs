use std::sync::Arc;

use arrow::array::{Int32Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::encode_arrow_semantic_payload;
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_core::runtime_abi::{PredicateEnvelope, ProjectionSet, SplitDescriptor};
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use loom_ffi::duckdb_runtime::{
    plan_duckdb_runtime, prepare_duckdb_runtime, DuckDbProjection, DuckDbRouteDecision,
    DuckDbRuntimePlanInput, DuckDbRuntimePolicy,
};
use loom_native_melior::backend::NativeBackendCancellation;

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
    wrap_layout_payload(&encode_layout_payload(&desc)).expect("valid LMC1 layout")
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
    wrap_table_payload(&encode_table_payload(&table).expect("valid table payload"))
        .expect("valid LMC1 table")
}

fn lma1_i32(row_count: usize) -> Vec<u8> {
    let values = (0..row_count as i32).collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(vec![Field::new(
        "value",
        DataType::Int32,
        false,
    )]));
    let batch =
        RecordBatch::try_new(schema, vec![Arc::new(Int32Array::from(values))]).expect("batch");
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("semantic payload");
    encode_arrow_semantic_payload(&payload).expect("encode LMA1")
}

fn native_input(row_count: usize, allow_interpreter_fallback: bool) -> DuckDbRuntimePlanInput {
    DuckDbRuntimePlanInput {
        artifact_bytes: lma1_i32(row_count),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback,
        },
    }
}

fn expected_i32_bytes(row_count: usize) -> Vec<u8> {
    (0..row_count as i32).flat_map(i32::to_le_bytes).collect()
}

#[test]
fn arrow_semantic_lma1_is_default_native_candidate_and_prepares_real_buffers() {
    let plan = plan_duckdb_runtime(native_input(4, false)).expect("native plan");

    assert_eq!(plan.decision, DuckDbRouteDecision::NativeCandidate);
    assert_eq!(plan.runtime_plan.projection, ProjectionSet::All);
    assert_eq!(plan.runtime_plan.predicate, PredicateEnvelope::None);
    assert_eq!(
        plan.runtime_plan.split,
        SplitDescriptor::FullScan { row_count: 4 }
    );
    assert!(plan
        .cache_key
        .canonical_input
        .contains("backend=duckdb-arrow-semantic-codegen"));
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "native-arrow-semantic-codegen-supported"));

    let prepared = prepare_duckdb_runtime(&plan, NativeBackendCancellation::default());
    assert_eq!(prepared.decision, DuckDbRouteDecision::NativeCandidate);
    assert_eq!(prepared.native_buffers.len(), 1);
    let buffer = &prepared.native_buffers[0];
    assert_eq!(buffer.builder_id, "value");
    assert_eq!(buffer.arrow_type, DataType::Int32);
    assert_eq!(buffer.row_count, 4);
    assert_eq!(buffer.value_buffer, expected_i32_bytes(4));
    assert!(buffer.validity_buffer.is_none());
    assert!(prepared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "native-arrow-semantic-codegen-output"));
}

#[test]
fn lmc1_raw_copy_no_longer_enters_duckdb_native_route() {
    let fallback = plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: true,
        },
    })
    .expect("fallback plan");
    assert_eq!(fallback.decision, DuckDbRouteDecision::InterpreterFallback);
    assert!(fallback
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "lowering-unsupported"));
    let prepared = prepare_duckdb_runtime(&fallback, NativeBackendCancellation::default());
    assert_eq!(prepared.decision, DuckDbRouteDecision::InterpreterFallback);
    assert!(prepared.native_buffers.is_empty());

    let strict = plan_duckdb_runtime(DuckDbRuntimePlanInput {
        artifact_bytes: raw_i32_lmc1(4),
        projection: DuckDbProjection::All,
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: false,
        },
    })
    .expect("strict plan");
    assert_eq!(strict.decision, DuckDbRouteDecision::FailClosed);
}

#[test]
fn projection_planning_is_preserved_but_non_full_arrow_native_falls_back() {
    let mut input = native_input(4, true);
    input.projection = DuckDbProjection::Columns(vec![0]);
    let plan = plan_duckdb_runtime(input).expect("projected plan");

    assert_eq!(plan.output_to_source, vec![0]);
    assert!(plan
        .cache_key
        .canonical_input
        .contains("projection=columns:0>0"));

    let prepared = prepare_duckdb_runtime(&plan, NativeBackendCancellation::default());
    assert_eq!(prepared.decision, DuckDbRouteDecision::InterpreterFallback);
    assert!(prepared.native_buffers.is_empty());
    assert!(prepared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("projection")));
}

#[test]
fn table_projection_without_native_uses_table_column_count() {
    let input = DuckDbRuntimePlanInput {
        artifact_bytes: two_column_table_lmc1(4),
        projection: DuckDbProjection::Columns(vec![1, 0]),
        policy: DuckDbRuntimePolicy {
            allow_interpreter_fallback: true,
        },
    };
    let plan = plan_duckdb_runtime(input).expect("table projection runtime plan");

    assert_eq!(plan.decision, DuckDbRouteDecision::InterpreterFallback);
    assert_eq!(plan.output_to_source, vec![1, 0]);
    assert!(plan
        .cache_key
        .canonical_input
        .contains("projection=columns:1>0,0>1"));
}

#[test]
fn invalid_projection_fails_closed_before_prepare() {
    let mut duplicate = native_input(4, true);
    duplicate.projection = DuckDbProjection::Columns(vec![0, 0]);
    let err = plan_duckdb_runtime(duplicate).expect_err("duplicate source rejected");
    assert!(err
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported-projection"));

    let mut out_of_range = native_input(4, true);
    out_of_range.projection = DuckDbProjection::Columns(vec![3]);
    let err = plan_duckdb_runtime(out_of_range).expect_err("out of range source rejected");
    assert!(err
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported-projection"));
}

#[test]
fn cancelled_arrow_semantic_prepare_returns_cancelled_without_buffers() {
    let plan = plan_duckdb_runtime(native_input(4, false)).expect("native plan");
    let prepared = prepare_duckdb_runtime(
        &plan,
        NativeBackendCancellation::cancelled("duckdb interrupt"),
    );

    assert_eq!(prepared.decision, DuckDbRouteDecision::Cancelled);
    assert!(prepared.native_buffers.is_empty());
    assert!(prepared
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "cancelled"));
}
