#![cfg(feature = "melior")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
};
use arrow_schema::{DataType, Field, Schema};
use loom_ffi::arrow_semantic::ArrowSemanticPayload;
use loom_ffi::arrow_semantic_codec::encode_arrow_semantic_container_payload;
use loom_ffi::native_arrow_semantic::execute_native_arrow_semantic;
use loom_ffi::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeSafetyPolicy, SplitDescriptor,
};
use loom_native_melior::backend::NativeBackendDiagnosticCode;
use loom_native_melior::jit::{
    execute_arrow_semantic_codegen_production_route,
    execute_arrow_semantic_codegen_production_route_with_cancellation_checkpoint,
    ArrowSemanticCodegenCancellationCheckpoint, ArrowSemanticCodegenRouteStatus,
};

#[test]
fn replay_soak_repeated_execution_has_stable_evidence_and_drift_misses() {
    let bytes = encode_lmc2(&full_primitive_nullable_batch(7));
    let mut cached_stable_id = None;
    let mut cached_canonical_input = None;
    let mut cached_replay_fingerprint = None;
    let mut cache_misses = 0;
    let mut cache_hits = 0;
    let mut total_native_elapsed = Duration::ZERO;

    for iteration in 0..5 {
        let started = Instant::now();
        let route = execute_arrow_semantic_codegen_production_route(
            &bytes,
            &Default::default(),
            ProjectionSet::All,
            PredicateEnvelope::None,
            SplitDescriptor::FullScan { row_count: 9 },
            RuntimeSafetyPolicy::default(),
        );
        total_native_elapsed += started.elapsed();

        assert_eq!(
            route.status,
            ArrowSemanticCodegenRouteStatus::NativeCandidate,
            "iteration {iteration}: {:?}",
            route.diagnostics
        );
        assert!(route.cacheable, "iteration {iteration}");
        assert!(route.diagnostics.is_empty(), "{:?}", route.diagnostics);
        assert!(route.resource_evidence.raw_pointer_identity_used == false);
        assert_eq!(
            route.resource_evidence.output_buffer_ownership,
            "owned-rust-vec"
        );
        assert!(route.resource_evidence.output_value_buffer_bytes > 0);
        assert!(route.resource_evidence.output_validity_buffer_bytes > 0);
        assert!(route
            .resource_evidence
            .route_steps_completed
            .iter()
            .any(|step| step == "validated"));

        let replay = route.replay_evidence.expect("replay evidence");
        match (
            cached_stable_id.as_ref(),
            cached_canonical_input.as_ref(),
            cached_replay_fingerprint.as_ref(),
        ) {
            (Some(stable_id), Some(canonical_input), Some(replay_fingerprint)) => {
                assert_eq!(stable_id, &replay.runtime_cache_stable_id);
                assert_eq!(canonical_input, &replay.runtime_cache_canonical_input);
                assert_eq!(replay_fingerprint, &replay.replay_fingerprint);
                cache_hits += 1;
            }
            _ => {
                cached_stable_id = Some(replay.runtime_cache_stable_id.clone());
                cached_canonical_input = Some(replay.runtime_cache_canonical_input.clone());
                cached_replay_fingerprint = Some(replay.replay_fingerprint.clone());
                cache_misses += 1;
            }
        }
    }

    assert_eq!(cache_misses, 1);
    assert_eq!(cache_hits, 4);

    let drifted = encode_lmc2(&full_primitive_nullable_batch(701));
    let drifted_route = execute_arrow_semantic_codegen_production_route(
        &drifted,
        &Default::default(),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    );
    assert_eq!(
        drifted_route.status,
        ArrowSemanticCodegenRouteStatus::NativeCandidate
    );
    let drifted_replay = drifted_route.replay_evidence.expect("drifted replay");
    assert_ne!(
        cached_stable_id.expect("stable id"),
        drifted_replay.runtime_cache_stable_id
    );
    assert_ne!(
        cached_canonical_input.expect("canonical input"),
        drifted_replay.runtime_cache_canonical_input
    );
    assert_ne!(
        cached_replay_fingerprint.expect("replay fingerprint"),
        drifted_replay.replay_fingerprint
    );

    let reference_started = Instant::now();
    let reference = execute_native_arrow_semantic(&bytes);
    let reference_elapsed = reference_started.elapsed();
    assert!(reference.is_supported());
    eprintln!(
        "phase43.2-soak timing rows=9 columns=5 native_route_total_5={:?} reference_single={:?}",
        total_native_elapsed, reference_elapsed
    );
}

#[test]
fn cancellation_checkpoints_are_distinct_and_non_cacheable() {
    let bytes = encode_lmc2(&full_primitive_nullable_batch(7));
    for (checkpoint, expected_path, expected_steps) in [
        (
            ArrowSemanticCodegenCancellationCheckpoint::BeforeSupport,
            "$.cancellation.before_support",
            Vec::<&str>::new(),
        ),
        (
            ArrowSemanticCodegenCancellationCheckpoint::BeforeJit,
            "$.cancellation.before_jit",
            vec!["support-extracted"],
        ),
        (
            ArrowSemanticCodegenCancellationCheckpoint::BeforeValidation,
            "$.cancellation.before_validation",
            vec!["support-extracted", "jit-executed"],
        ),
    ] {
        let route = execute_arrow_semantic_codegen_production_route_with_cancellation_checkpoint(
            &bytes,
            checkpoint,
            ProjectionSet::All,
            PredicateEnvelope::None,
            SplitDescriptor::FullScan { row_count: 9 },
            RuntimeSafetyPolicy::default(),
        );

        assert_eq!(route.status, ArrowSemanticCodegenRouteStatus::Cancelled);
        assert!(!route.cacheable);
        assert!(route.replay_evidence.is_none());
        assert!(route.execution.is_none());
        assert_eq!(
            route.resource_evidence.cancellation_checkpoint.as_deref(),
            Some(checkpoint.as_str())
        );
        assert_eq!(
            route.resource_evidence.route_steps_completed,
            expected_steps
                .iter()
                .map(|step| (*step).to_string())
                .collect::<Vec<_>>()
        );
        let diagnostic = route.diagnostics.first().expect("diagnostic");
        assert_eq!(diagnostic.code, NativeBackendDiagnosticCode::Cancelled);
        assert_eq!(diagnostic.path, expected_path);
    }
}

fn full_primitive_nullable_batch(first_id: i32) -> RecordBatch {
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
                Some(first_id),
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
