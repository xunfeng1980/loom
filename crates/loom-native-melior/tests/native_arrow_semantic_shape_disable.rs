//! Tests for per-shape native-route disable (Phase 48-02).
//!
//! When the `melior` feature is enabled, these tests drive the full production
//! route and verify that a trace divergence disables the shape for the process
//! lifetime.  When `melior` is unavailable, only the registry-level smoke tests
//! run.

use std::sync::Arc;

use arrow_array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::encode_arrow_semantic_container_payload;
use loom_core::native_arrow_semantic::prepare_native_arrow_semantic_codegen_support;
use loom_native_melior::backend::NativeBackendCancellation;
use loom_native_melior::jit::{
    disable_shape, is_shape_disabled, reset_disabled_shapes,
};

// ---------------------------------------------------------------------------
// Registry smoke tests (no melior required)
// ---------------------------------------------------------------------------

// Registry-level smoke test moved to jit.rs #[cfg(test)] to avoid racing
// with melior tests that also call reset_disabled_shapes().

// ---------------------------------------------------------------------------
// Full production-route tests (require melior + K oracle)
// ---------------------------------------------------------------------------

#[cfg(feature = "melior")]
mod melior_tests {
    use super::*;
    use arrow_array::Array;
    use loom_core::runtime_abi::{
        PredicateEnvelope, ProjectionSet, RuntimeSafetyPolicy, SplitDescriptor,
    };
    use loom_native_melior::jit::{
        validate_arrow_semantic_codegen_production_route_output_with_cancellation,
        ArrowSemanticCodegenJitOutput,
    };
    use loom_native_melior::backend::NativeBackendDiagnosticCode;
    use loom_native_melior::jit::ArrowSemanticCodegenRouteStatus;
    use loom_native_melior::jit::{
        disable_shape, execute_arrow_semantic_codegen_jit,
        execute_arrow_semantic_codegen_production_route,
        is_shape_disabled, reset_disabled_shapes,
    };

    #[test]
    fn divergence_disables_shape_and_fails_closed() {
        reset_disabled_shapes();

        let batch = full_primitive_nullable_batch();
        let bytes = encode_lmc2(&batch);
        let support = prepare_native_arrow_semantic_codegen_support(&bytes);
        assert!(support.is_supported(), "{support:?}");

        let jit = execute_arrow_semantic_codegen_jit(&support, &NativeBackendCancellation::default())
            .expect("melior JIT must succeed for this test");

        // Mutate the JIT output to create a divergence.
        //
        // We flip TWO bits in the first column's validity buffer so the null
        // count stays the same (2) and passes `null_buffer_from_codegen_column`,
        // but the builder-event trace changes (row0: append-value → append-null,
        // row1: append-null → append-value).  This guarantees a
        // NativeModelTraceMismatch.
        //
        // Boolean column original validity (byte0, LSB=row0):
        //   bit0=1(valid), bit1=0(null), bit5=0(null) → null_count = 2
        // After flipping bit0 and bit1:
        //   bit0=0(null), bit1=1(valid), bit5=0(null) → null_count = 2
        let mut mutated_columns = jit.columns.clone();
        assert!(!mutated_columns.is_empty());
        let first_col = &mut mutated_columns[0];
        if let Some(ref mut validity) = first_col.validity_buffer {
            if !validity.is_empty() {
                validity[0] ^= 0x03; // flip bits 0 and 1
            }
        } else {
            panic!("first column must have a validity buffer for this test");
        }

        let report = validate_arrow_semantic_codegen_production_route_output_with_cancellation(
            &bytes,
            support.clone(),
            ArrowSemanticCodegenJitOutput {
                entry_symbol: jit.entry_symbol,
                row_count: jit.row_count,
                column_count: jit.column_count,
                backend_identity: jit.backend_identity,
                columns: mutated_columns,
            },
            &NativeBackendCancellation::default(),
            ProjectionSet::All,
            PredicateEnvelope::None,
            SplitDescriptor::FullScan {
                row_count: batch.num_rows() as u64,
            },
            RuntimeSafetyPolicy::default(),
        );

        // Divergence should disable the shape and return fallback/fail-closed.
        assert!(
            matches!(
                report.status,
                ArrowSemanticCodegenRouteStatus::InterpreterFallback
                    | ArrowSemanticCodegenRouteStatus::FailClosed
            ),
            "expected fallback/fail-closed on divergence, got {:?}",
            report.status
        );
        assert!(
            !report.cacheable,
            "divergent shape must not be cacheable"
        );
        assert!(
            report.replay_evidence.is_none(),
            "divergent shape must not produce replay evidence"
        );
        assert!(
            report.diagnostics.iter().any(|d| d.code == NativeBackendDiagnosticCode::NativeShapeDisabled),
            "expected NativeShapeDisabled diagnostic, got {:?}",
            report.diagnostics
        );
        assert!(
            is_shape_disabled(&support.schema_fingerprint),
            "shape must be recorded as disabled after divergence"
        );
    }

    #[test]
    fn pre_check_fast_fallback_on_disabled_shape() {
        reset_disabled_shapes();

        // Use a batch with a *different* schema fingerprint from the divergence
        // test so the two tests can run concurrently without racing on the
        // shared global disable registry.
        let batch = full_primitive_nullable_batch_v3();
        let bytes = encode_lmc2_v3(&batch);
        let support = prepare_native_arrow_semantic_codegen_support(&bytes);
        assert!(support.is_supported());

        // Artificially disable the shape before running the route.
        disable_shape(&support.schema_fingerprint);

        // Use the full production route entry point so the pre-check fires.
        let report = execute_arrow_semantic_codegen_production_route(
            &bytes,
            &NativeBackendCancellation::default(),
            ProjectionSet::All,
            PredicateEnvelope::None,
            SplitDescriptor::FullScan {
                row_count: batch.num_rows() as u64,
            },
            RuntimeSafetyPolicy::default(),
        );

        assert!(
            matches!(
                report.status,
                ArrowSemanticCodegenRouteStatus::InterpreterFallback
                    | ArrowSemanticCodegenRouteStatus::FailClosed
            ),
            "expected fallback on pre-check for disabled shape, got {:?}",
            report.status
        );
        assert!(
            report.diagnostics.iter().any(|d| d.code == NativeBackendDiagnosticCode::NativeShapeDisabled),
            "expected NativeShapeDisabled diagnostic on pre-check, got {:?}",
            report.diagnostics
        );
        assert!(
            !report.cacheable,
            "disabled shape must not be cacheable"
        );
    }

    #[test]
    fn skip_does_not_disable_shape() {
        reset_disabled_shapes();

        // Use a slightly different batch so the schema fingerprint is unique
        // to this test, avoiding race conditions with the divergence test
        // that shares the same static disable registry.
        // Use a batch with different column names so the schema fingerprint
        // is unique to this test, avoiding race conditions with the divergence
        // test that shares the same static disable registry.
        let batch = full_primitive_nullable_batch_v2();

        let bytes = encode_lmc2(&batch);
        let support = prepare_native_arrow_semantic_codegen_support(&bytes);
        assert!(support.is_supported());

        // Run the full production route (JIT + K-oracle validation).
        let report = execute_arrow_semantic_codegen_production_route(
            &bytes,
            &NativeBackendCancellation::default(),
            ProjectionSet::All,
            PredicateEnvelope::None,
            SplitDescriptor::FullScan {
                row_count: batch.num_rows() as u64,
            },
            RuntimeSafetyPolicy::default(),
        );

        assert!(
            !is_shape_disabled(&support.schema_fingerprint),
            "a clean run must not disable the shape"
        );
        assert!(
            !report.diagnostics.iter().any(|d| d.code == NativeBackendDiagnosticCode::NativeShapeDisabled),
            "clean run must not emit NativeShapeDisabled, got {:?}",
            report.diagnostics
        );
    }

    fn full_primitive_nullable_batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("ok", DataType::Boolean, true),
            Field::new("id", DataType::Int32, true),
            Field::new("count", DataType::Int64, true),
            Field::new("ratio", DataType::Float32, true),
            Field::new("score", DataType::Float64, true),
        ]));
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(BooleanArray::from(vec![
                    Some(true),
                    None,
                    Some(false),
                    Some(true),
                    Some(false),
                    None,
                    Some(true),
                    Some(false),
                    Some(true),
                ])) as ArrayRef,
                Arc::new(Int32Array::from(vec![
                    Some(7),
                    None,
                    Some(-1),
                    Some(128),
                    Some(-2048),
                    None,
                    Some(33),
                    Some(44),
                    Some(55),
                ])) as ArrayRef,
                Arc::new(Int64Array::from(vec![
                    Some(70),
                    None,
                    Some(-10),
                    Some(7000),
                    Some(-9000),
                    None,
                    Some(330),
                    Some(440),
                    Some(550),
                ])) as ArrayRef,
                Arc::new(Float32Array::from(vec![
                    Some(0.25),
                    None,
                    Some(-1.5),
                    Some(3.75),
                    Some(-8.5),
                    None,
                    Some(9.25),
                    Some(10.5),
                    Some(11.75),
                ])) as ArrayRef,
                Arc::new(Float64Array::from(vec![
                    Some(1.5),
                    None,
                    Some(-2.25),
                    Some(4.5),
                    Some(-16.75),
                    None,
                    Some(18.25),
                    Some(20.5),
                    Some(22.75),
                ])) as ArrayRef,
            ],
        )
        .expect("full primitive nullable batch")
    }

    fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
        let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
        encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn full_primitive_nullable_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int32, true),
        Field::new("count", DataType::Int64, true),
        Field::new("ratio", DataType::Float32, true),
        Field::new("score", DataType::Float64, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![
                Some(true),
                None,
                Some(false),
                Some(true),
                Some(false),
                None,
                Some(true),
                Some(false),
                Some(true),
            ])) as ArrayRef,
            Arc::new(Int32Array::from(vec![
                Some(7),
                None,
                Some(-1),
                Some(128),
                Some(-2048),
                None,
                Some(33),
                Some(44),
                Some(55),
            ])) as ArrayRef,
            Arc::new(Int64Array::from(vec![
                Some(70),
                None,
                Some(-10),
                Some(7000),
                Some(-9000),
                None,
                Some(330),
                Some(440),
                Some(550),
            ])) as ArrayRef,
            Arc::new(Float32Array::from(vec![
                Some(0.25),
                None,
                Some(-1.5),
                Some(3.75),
                Some(-8.5),
                None,
                Some(9.25),
                Some(10.5),
                Some(11.75),
            ])) as ArrayRef,
            Arc::new(Float64Array::from(vec![
                Some(1.5),
                None,
                Some(-2.25),
                Some(4.5),
                Some(-16.75),
                None,
                Some(18.25),
                Some(20.5),
                Some(22.75),
            ])) as ArrayRef,
        ],
    )
    .expect("full primitive nullable batch")
}

/// Same data shape as `full_primitive_nullable_batch` but with different column
/// names so the schema fingerprint is distinct.  Used by tests that must not
/// share a disable-registry entry with the divergence test.
fn full_primitive_nullable_batch_v2() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok_v2", DataType::Boolean, true),
        Field::new("id_v2", DataType::Int32, true),
        Field::new("count_v2", DataType::Int64, true),
        Field::new("ratio_v2", DataType::Float32, true),
        Field::new("score_v2", DataType::Float64, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![
                Some(true),
                None,
                Some(false),
                Some(true),
                Some(false),
                None,
                Some(true),
                Some(false),
                Some(true),
            ])) as ArrayRef,
            Arc::new(Int32Array::from(vec![
                Some(7),
                None,
                Some(-1),
                Some(128),
                Some(-2048),
                None,
                Some(33),
                Some(44),
                Some(55),
            ])) as ArrayRef,
            Arc::new(Int64Array::from(vec![
                Some(70),
                None,
                Some(-10),
                Some(7000),
                Some(-9000),
                None,
                Some(330),
                Some(440),
                Some(550),
            ])) as ArrayRef,
            Arc::new(Float32Array::from(vec![
                Some(0.25),
                None,
                Some(-1.5),
                Some(3.75),
                Some(-8.5),
                None,
                Some(9.25),
                Some(10.5),
                Some(11.75),
            ])) as ArrayRef,
            Arc::new(Float64Array::from(vec![
                Some(1.5),
                None,
                Some(-2.25),
                Some(4.5),
                Some(-16.75),
                None,
                Some(18.25),
                Some(20.5),
                Some(22.75),
            ])) as ArrayRef,
        ],
    )
    .expect("full primitive nullable batch v2")
}

fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}

/// Third variant with distinct column names so tests that manipulate the
/// disable registry can run concurrently without fingerprint collisions.
fn full_primitive_nullable_batch_v3() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok_v3", DataType::Boolean, true),
        Field::new("id_v3", DataType::Int32, true),
        Field::new("count_v3", DataType::Int64, true),
        Field::new("ratio_v3", DataType::Float32, true),
        Field::new("score_v3", DataType::Float64, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![
                Some(true), None, Some(false),
                Some(true), Some(false), None,
                Some(true), Some(false), Some(true),
            ])) as ArrayRef,
            Arc::new(Int32Array::from(vec![
                Some(7), None, Some(-1),
                Some(128), Some(-2048), None,
                Some(33), Some(44), Some(55),
            ])) as ArrayRef,
            Arc::new(Int64Array::from(vec![
                Some(70), None, Some(-10),
                Some(7000), Some(-9000), None,
                Some(330), Some(440), Some(550),
            ])) as ArrayRef,
            Arc::new(Float32Array::from(vec![
                Some(0.25), None, Some(-1.5),
                Some(3.75), Some(-8.5), None,
                Some(9.25), Some(10.5), Some(11.75),
            ])) as ArrayRef,
            Arc::new(Float64Array::from(vec![
                Some(1.5), None, Some(-2.25),
                Some(4.5), Some(-16.75), None,
                Some(18.25), Some(20.5), Some(22.75),
            ])) as ArrayRef,
        ],
    )
    .expect("full primitive nullable batch v3")
}

fn encode_lmc2_v3(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2 v3")
}
