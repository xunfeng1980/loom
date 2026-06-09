use std::sync::Arc;

use arrow_array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
    StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::{
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
};
use loom_core::native_arrow_semantic::{
    native_arrow_semantic_codegen_replay_evidence, prepare_native_arrow_semantic_codegen_support,
    validate_native_arrow_semantic_codegen_output,
    validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape,
    NativeArrowSemanticCodegenOutputColumn, NativeArrowSemanticDiagnosticCode,
};
use loom_core::runtime_abi::{
    PredicateEnvelope, PredicateOperator, ProjectionColumn, ProjectionSet, RuntimeSafetyPolicy,
    SplitDescriptor,
};

#[test]
fn replay_evidence_is_deterministic_for_lmc2_and_direct_lma1() {
    let batch = full_primitive_nullable_batch();
    let lmc2 = encode_lmc2(&batch);
    let direct_lma1 = encode_lma1(&batch);

    let first = replay_evidence_for(
        &lmc2,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );
    let second = replay_evidence_for(
        &lmc2,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );
    assert_eq!(first, second);
    assert_eq!(first.artifact_kind, "LMC2");
    assert!(first
        .runtime_cache_canonical_input
        .contains("predicate=none"));
    assert!(first.runtime_cache_canonical_input.contains("split=full:9"));

    let direct = replay_evidence_for(
        &direct_lma1,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );
    assert_eq!(direct.artifact_kind, "LMA1");
    assert_ne!(direct.artifact_digest, first.artifact_digest);
    assert_ne!(direct.replay_fingerprint, first.replay_fingerprint);
    assert_ne!(
        direct.runtime_cache_canonical_input,
        first.runtime_cache_canonical_input
    );
}

#[test]
fn replay_evidence_detects_projection_predicate_split_backend_and_artifact_drift() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let base = replay_evidence_for(
        &bytes,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );

    let projected = replay_evidence_for(
        &bytes,
        "melior-jit:test-pipeline",
        ProjectionSet::Columns(vec![ProjectionColumn {
            source_index: 1,
            output_index: 0,
        }]),
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );
    assert_ne!(
        projected.runtime_cache_canonical_input,
        base.runtime_cache_canonical_input
    );
    assert_ne!(projected.replay_fingerprint, base.replay_fingerprint);

    let predicated = replay_evidence_for(
        &bytes,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::PrimitiveComparison {
            column_index: 1,
            op: PredicateOperator::GtEq,
            literal_i64: 0,
        },
        SplitDescriptor::FullScan { row_count: 9 },
    );
    assert_ne!(
        predicated.runtime_cache_canonical_input,
        base.runtime_cache_canonical_input
    );
    assert!(predicated
        .runtime_cache_canonical_input
        .contains("predicate=cmp:1:gt-eq:0"));

    let split = replay_evidence_for(
        &bytes,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::RowRange { start: 2, end: 7 },
    );
    assert_ne!(
        split.runtime_cache_canonical_input,
        base.runtime_cache_canonical_input
    );
    assert!(split
        .runtime_cache_canonical_input
        .contains("split=range:2:7"));

    let backend = replay_evidence_for(
        &bytes,
        "melior-jit:alternate-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );
    assert_ne!(
        backend.runtime_cache_canonical_input,
        base.runtime_cache_canonical_input
    );
    assert_ne!(backend.replay_fingerprint, base.replay_fingerprint);

    let drifted_artifact = encode_lmc2(&drifted_primitive_nullable_batch());
    let artifact = replay_evidence_for(
        &drifted_artifact,
        "melior-jit:test-pipeline",
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
    );
    assert_ne!(artifact.artifact_digest, base.artifact_digest);
    assert_ne!(
        artifact.output_buffer_fingerprint,
        base.output_buffer_fingerprint
    );
    assert_ne!(artifact.replay_fingerprint, base.replay_fingerprint);
}

#[test]
fn unsupported_and_divergent_outputs_cannot_produce_replay_evidence() {
    let unsupported = encode_lmc2(&utf8_batch());
    let support = prepare_native_arrow_semantic_codegen_support(&unsupported);
    assert!(!support.is_supported());
    let err = native_arrow_semantic_codegen_replay_evidence(
        &unsupported,
        &support,
        &validate_native_arrow_semantic_codegen_output(
            &unsupported,
            &support,
            "melior-jit:test-pipeline",
            Vec::new(),
        ),
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 3 },
        RuntimeSafetyPolicy::default(),
    )
    .expect_err("unsupported support cannot produce replay evidence");
    assert_eq!(err.code, NativeArrowSemanticDiagnosticCode::UnsupportedType);

    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    let mut outputs = mirrored_output_columns(&support);
    outputs[1].value_buffer[0] ^= 0x7f;
    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        "melior-jit:test-pipeline",
        outputs,
    );
    assert!(!execution.is_supported());

    let err = native_arrow_semantic_codegen_replay_evidence(
        &bytes,
        &support,
        &execution,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    )
    .expect_err("divergent execution cannot produce replay evidence");
    assert_eq!(
        err.code,
        NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
    );
}

#[test]
fn shape_aware_cache_key_records_predicate_and_split_drift() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        "melior-jit:test-pipeline",
        mirrored_output_columns(&support),
    );
    assert!(execution.is_supported(), "{:?}", execution.diagnostics());

    let full = validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
        &bytes,
        &execution,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    )
    .expect("full scan key");
    let ranged = validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
        &bytes,
        &execution,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::RowRange { start: 0, end: 4 },
        RuntimeSafetyPolicy::default(),
    )
    .expect("range key");
    assert_ne!(full.canonical_input, ranged.canonical_input);
    assert!(full.canonical_input.contains("split=full:9"));
    assert!(ranged.canonical_input.contains("split=range:0:4"));

    let predicated = validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
        &bytes,
        &execution,
        ProjectionSet::All,
        PredicateEnvelope::PrimitiveComparison {
            column_index: 1,
            op: PredicateOperator::Lt,
            literal_i64: 100,
        },
        SplitDescriptor::FullScan { row_count: 9 },
        RuntimeSafetyPolicy::default(),
    )
    .expect("predicate key");
    assert_ne!(full.canonical_input, predicated.canonical_input);
    assert!(predicated
        .canonical_input
        .contains("predicate=cmp:1:lt:100"));
}

fn replay_evidence_for(
    bytes: &[u8],
    backend_identity: &str,
    projection: ProjectionSet,
    predicate: PredicateEnvelope,
    split: SplitDescriptor,
) -> loom_core::native_arrow_semantic::NativeArrowSemanticCodegenReplayEvidence {
    let support = prepare_native_arrow_semantic_codegen_support(bytes);
    assert!(support.is_supported(), "{:?}", support.diagnostics());
    let execution = validate_native_arrow_semantic_codegen_output(
        bytes,
        &support,
        backend_identity,
        mirrored_output_columns(&support),
    );
    assert!(execution.is_supported(), "{:?}", execution.diagnostics());
    native_arrow_semantic_codegen_replay_evidence(
        bytes,
        &support,
        &execution,
        projection,
        predicate,
        split,
        RuntimeSafetyPolicy::default(),
    )
    .expect("replay evidence")
}

fn mirrored_output_columns(
    support: &loom_core::native_arrow_semantic::NativeArrowSemanticCodegenSupportReport,
) -> Vec<NativeArrowSemanticCodegenOutputColumn> {
    support
        .columns()
        .iter()
        .map(|column| NativeArrowSemanticCodegenOutputColumn {
            index: column.index,
            value_buffer: column.value_buffer.clone(),
            validity_buffer: column.validity_buffer.clone(),
        })
        .collect()
}

fn full_primitive_nullable_batch() -> RecordBatch {
    primitive_nullable_batch(7)
}

fn drifted_primitive_nullable_batch() -> RecordBatch {
    primitive_nullable_batch(701)
}

fn primitive_nullable_batch(first_id: i32) -> RecordBatch {
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

fn utf8_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new("name", DataType::Utf8, true)]));
    RecordBatch::try_new(
        schema,
        vec![Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])) as ArrayRef],
    )
    .expect("utf8 batch")
}

fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}

fn encode_lma1(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_payload(&payload).expect("encode LMA1")
}
