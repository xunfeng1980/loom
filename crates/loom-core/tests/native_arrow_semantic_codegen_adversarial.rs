use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
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
    NativeArrowSemanticCodegenBufferKind, NativeArrowSemanticCodegenOutputColumn,
    NativeArrowSemanticDiagnosticCode,
};
use loom_core::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeSafetyPolicy, SplitDescriptor,
};

#[test]
fn malformed_output_buffers_fail_before_validation_cache_or_replay() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    assert!(support.is_supported(), "{:?}", support.diagnostics());

    let cases: Vec<(
        &str,
        Vec<NativeArrowSemanticCodegenOutputColumn>,
        NativeArrowSemanticDiagnosticCode,
        &str,
    )> = vec![
        (
            "short value buffer",
            mutate_outputs(&support, |outputs| {
                outputs[1].value_buffer.pop();
            }),
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.codegen.output.columns[1].value_buffer",
        ),
        (
            "long value buffer",
            mutate_outputs(&support, |outputs| outputs[1].value_buffer.push(0)),
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.codegen.output.columns[1].value_buffer",
        ),
        (
            "short validity buffer",
            mutate_outputs(&support, |outputs| {
                outputs[1].validity_buffer.as_mut().expect("validity").pop();
            }),
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.codegen.output.columns[1].validity_buffer",
        ),
        (
            "long validity buffer",
            mutate_outputs(&support, |outputs| {
                outputs[1]
                    .validity_buffer
                    .as_mut()
                    .expect("validity")
                    .push(0)
            }),
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.codegen.output.columns[1].validity_buffer",
        ),
        (
            "missing nullable validity buffer",
            mutate_outputs(&support, |outputs| outputs[1].validity_buffer = None),
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.codegen.output.columns[1].validity_buffer",
        ),
        (
            "all-valid nullable validity buffer",
            mutate_outputs(&support, |outputs| {
                outputs[1].validity_buffer = Some(vec![0xff; outputs[1].validity_buffer_bytes()])
            }),
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.codegen.output.columns[1].validity_buffer",
        ),
        (
            "swapped column order",
            mutate_outputs(&support, |outputs| outputs.swap(0, 1)),
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.codegen.output.columns[0].index",
        ),
        (
            "dropped column",
            mutate_outputs(&support, |outputs| {
                outputs.pop();
            }),
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.codegen.output.columns",
        ),
    ];

    for (name, outputs, expected_code, expected_path) in cases {
        let execution = validate_native_arrow_semantic_codegen_output(
            &bytes,
            &support,
            format!("melior-jit:adversarial:{name}"),
            outputs,
        );
        assert!(!execution.is_supported(), "{name} unexpectedly validated");
        let diagnostic = execution.first_error().expect("diagnostic");
        assert_eq!(diagnostic.code, expected_code, "{name}");
        assert_eq!(diagnostic.path, expected_path, "{name}");
        assert_no_cache_or_replay(&bytes, &support, &execution);
    }

    let non_null = encode_lmc2(&non_null_primitive_batch());
    let non_null_support = prepare_native_arrow_semantic_codegen_support(&non_null);
    assert!(non_null_support.is_supported());
    let outputs = mutate_outputs(&non_null_support, |outputs| {
        outputs[0].validity_buffer = Some(vec![0xff; 1])
    });
    let execution = validate_native_arrow_semantic_codegen_output(
        &non_null,
        &non_null_support,
        "melior-jit:adversarial:extra-validity",
        outputs,
    );
    assert!(!execution.is_supported());
    let diagnostic = execution.first_error().expect("diagnostic");
    assert_eq!(
        diagnostic.code,
        NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
    );
    assert_eq!(
        diagnostic.path,
        "$.codegen.output.columns[0].validity_buffer"
    );
    assert_no_cache_or_replay(&non_null, &non_null_support, &execution);
}

#[test]
fn bitmap_boundaries_sliced_buffers_and_row_extremes_are_validated() {
    let batch = full_primitive_nullable_batch();
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    assert!(support.is_supported(), "{:?}", support.diagnostics());

    let bool_column = support
        .columns()
        .iter()
        .find(|column| {
            column.value_buffer_kind == NativeArrowSemanticCodegenBufferKind::BooleanValueBitmap
        })
        .expect("boolean column");
    assert_eq!(bool_column.value_buffer_bytes(), 2);

    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        "melior-jit:adversarial:boolean-bitmap",
        mutate_outputs(&support, |outputs| {
            outputs[0].value_buffer[0] ^= 0b0000_0100
        }),
    );
    assert!(!execution.is_supported());
    assert_eq!(
        execution.first_error().expect("diagnostic").code,
        NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
    );
    assert_no_cache_or_replay(&bytes, &support, &execution);

    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        "melior-jit:adversarial:validity-bitmap",
        mutate_outputs(&support, |outputs| {
            outputs[2].validity_buffer.as_mut().expect("validity")[0] ^= 0b0000_0010
        }),
    );
    assert!(!execution.is_supported());
    assert_eq!(
        execution.first_error().expect("diagnostic").path,
        "$.codegen.output.columns[2].validity_buffer"
    );
    assert_no_cache_or_replay(&bytes, &support, &execution);

    assert_round_trips(sliced_primitive_nullable_batch(), "sliced buffers");
    assert_round_trips(zero_row_primitive_batch(), "zero row");
    assert_round_trips(one_row_primitive_batch(), "one row");
    assert_round_trips(all_null_primitive_batch(), "all null");
    assert_round_trips(no_null_nullable_primitive_batch(), "nullable no null");
    assert_round_trips(non_null_primitive_batch(), "non nullable");
}

#[test]
fn artifact_schema_trace_and_bridge_adversaries_fail_closed() {
    for malformed in [
        b"LMC2".as_slice(),
        b"LMA1".as_slice(),
        b"not-a-loom-artifact".as_slice(),
    ] {
        let support = prepare_native_arrow_semantic_codegen_support(malformed);
        assert!(!support.is_supported());
        let execution = validate_native_arrow_semantic_codegen_output(
            malformed,
            &support,
            "melior-jit:adversarial:malformed",
            Vec::new(),
        );
        assert!(!execution.is_supported());
        assert_no_cache_or_replay(malformed, &support, &execution);
    }

    let batch = full_primitive_nullable_batch();
    let multi_payload = ArrowSemanticPayload::from_record_batches(&[batch.clone(), batch.clone()])
        .expect("payload");
    let multi = encode_arrow_semantic_container_payload(&multi_payload).expect("multi batch LMC2");
    let support = prepare_native_arrow_semantic_codegen_support(&multi);
    assert!(!support.is_supported());
    assert_eq!(
        support.first_error().expect("diagnostic").code,
        NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape
    );

    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    assert!(support.is_supported(), "{:?}", support.diagnostics());
    let outputs = mirrored_output_columns(&support);

    for (name, drifted_bytes) in [
        ("schema-name-drift", encode_lmc2(&renamed_primitive_batch())),
        ("type-drift", encode_lmc2(&type_drift_batch())),
        ("value-trace-drift", encode_lmc2(&value_drift_batch())),
    ] {
        let execution = validate_native_arrow_semantic_codegen_output(
            &drifted_bytes,
            &support,
            format!("melior-jit:adversarial:{name}"),
            outputs.clone(),
        );
        assert!(!execution.is_supported(), "{name} unexpectedly validated");
        assert!(execution.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == NativeArrowSemanticDiagnosticCode::NativeOutputMismatch
                || diagnostic.code == NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch
        }));
        assert_no_cache_or_replay(&drifted_bytes, &support, &execution);
    }

    let lmc2_evidence = replay_evidence_for(&bytes);
    let lma1 = encode_lma1(&batch);
    let lma1_evidence = replay_evidence_for(&lma1);
    assert_eq!(lmc2_evidence.artifact_kind, "LMC2");
    assert_eq!(lma1_evidence.artifact_kind, "LMA1");
    assert_ne!(lmc2_evidence.artifact_digest, lma1_evidence.artifact_digest);
    assert_ne!(
        lmc2_evidence.runtime_cache_canonical_input,
        lma1_evidence.runtime_cache_canonical_input
    );
    assert_ne!(
        lmc2_evidence.replay_fingerprint,
        lma1_evidence.replay_fingerprint
    );
}

fn assert_round_trips(batch: RecordBatch, name: &str) {
    let bytes = encode_lmc2(&batch);
    let support = prepare_native_arrow_semantic_codegen_support(&bytes);
    assert!(
        support.is_supported(),
        "{name} support diagnostics: {:?}",
        support.diagnostics()
    );
    let execution = validate_native_arrow_semantic_codegen_output(
        &bytes,
        &support,
        format!("melior-jit:adversarial:{name}"),
        mirrored_output_columns(&support),
    );
    assert!(
        execution.is_supported(),
        "{name} execution diagnostics: {:?}",
        execution.diagnostics()
    );
    assert_eq!(execution.output().expect("output"), &batch);
}

fn assert_no_cache_or_replay(
    bytes: &[u8],
    support: &loom_core::native_arrow_semantic::NativeArrowSemanticCodegenSupportReport,
    execution: &loom_core::native_arrow_semantic::NativeArrowSemanticCodegenExecutionReport,
) {
    let cache_err = validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape(
        bytes,
        execution,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan {
            row_count: execution.row_count,
        },
        RuntimeSafetyPolicy::default(),
    )
    .expect_err("failed output must not seed cache");
    assert_eq!(
        cache_err.code,
        NativeArrowSemanticDiagnosticCode::UnsupportedPayload
    );

    assert!(native_arrow_semantic_codegen_replay_evidence(
        bytes,
        support,
        execution,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan {
            row_count: execution.row_count,
        },
        RuntimeSafetyPolicy::default(),
    )
    .is_err());
}

fn replay_evidence_for(
    bytes: &[u8],
) -> loom_core::native_arrow_semantic::NativeArrowSemanticCodegenReplayEvidence {
    let support = prepare_native_arrow_semantic_codegen_support(bytes);
    assert!(support.is_supported(), "{:?}", support.diagnostics());
    let execution = validate_native_arrow_semantic_codegen_output(
        bytes,
        &support,
        "melior-jit:adversarial:identity",
        mirrored_output_columns(&support),
    );
    assert!(execution.is_supported(), "{:?}", execution.diagnostics());
    native_arrow_semantic_codegen_replay_evidence(
        bytes,
        &support,
        &execution,
        ProjectionSet::All,
        PredicateEnvelope::None,
        SplitDescriptor::FullScan {
            row_count: execution.row_count,
        },
        RuntimeSafetyPolicy::default(),
    )
    .expect("replay evidence")
}

fn mutate_outputs(
    support: &loom_core::native_arrow_semantic::NativeArrowSemanticCodegenSupportReport,
    f: impl FnOnce(&mut Vec<NativeArrowSemanticCodegenOutputColumn>),
) -> Vec<NativeArrowSemanticCodegenOutputColumn> {
    let mut outputs = mirrored_output_columns(support);
    f(&mut outputs);
    outputs
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

trait OutputColumnExt {
    fn validity_buffer_bytes(&self) -> usize;
}

impl OutputColumnExt for NativeArrowSemanticCodegenOutputColumn {
    fn validity_buffer_bytes(&self) -> usize {
        self.validity_buffer
            .as_ref()
            .map(|buffer| buffer.len())
            .unwrap_or(0)
    }
}

fn full_primitive_nullable_batch() -> RecordBatch {
    primitive_batch(
        vec![
            Some(true),
            None,
            Some(false),
            Some(true),
            Some(false),
            None,
            Some(true),
            Some(false),
            Some(true),
        ],
        vec![
            Some(7),
            None,
            Some(-1),
            Some(128),
            Some(-2048),
            None,
            Some(33),
            Some(44),
            Some(55),
        ],
        true,
        "id",
        DataType::Int32,
    )
}

fn non_null_primitive_batch() -> RecordBatch {
    primitive_batch(
        vec![Some(true), Some(false), Some(true), Some(false)],
        vec![Some(1), Some(2), Some(3), Some(4)],
        false,
        "id",
        DataType::Int32,
    )
}

fn no_null_nullable_primitive_batch() -> RecordBatch {
    primitive_batch(
        vec![Some(true), Some(false), Some(true), Some(false)],
        vec![Some(1), Some(2), Some(3), Some(4)],
        true,
        "id",
        DataType::Int32,
    )
}

fn one_row_primitive_batch() -> RecordBatch {
    primitive_batch(
        vec![Some(false)],
        vec![Some(-99)],
        true,
        "id",
        DataType::Int32,
    )
}

fn zero_row_primitive_batch() -> RecordBatch {
    primitive_batch(Vec::new(), Vec::new(), true, "id", DataType::Int32)
}

fn all_null_primitive_batch() -> RecordBatch {
    primitive_batch(
        vec![None, None, None, None],
        vec![None, None, None, None],
        true,
        "id",
        DataType::Int32,
    )
}

fn sliced_primitive_nullable_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int32, true),
        Field::new("count", DataType::Int64, true),
        Field::new("ratio", DataType::Float32, true),
        Field::new("score", DataType::Float64, true),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(BooleanArray::from(vec![
            Some(false),
            Some(true),
            None,
            Some(false),
            Some(true),
            None,
            Some(false),
            Some(true),
            Some(false),
            Some(true),
            Some(false),
        ])) as ArrayRef,
        Arc::new(Int32Array::from(vec![
            Some(-10),
            Some(7),
            None,
            Some(-1),
            Some(128),
            None,
            Some(33),
            Some(44),
            Some(55),
            Some(66),
            Some(77),
        ])) as ArrayRef,
        Arc::new(Int64Array::from(vec![
            Some(-100),
            Some(70),
            None,
            Some(-10),
            Some(7000),
            None,
            Some(330),
            Some(440),
            Some(550),
            Some(660),
            Some(770),
        ])) as ArrayRef,
        Arc::new(Float32Array::from(vec![
            Some(-0.5),
            Some(0.25),
            None,
            Some(-1.5),
            Some(3.75),
            None,
            Some(9.25),
            Some(10.5),
            Some(11.75),
            Some(12.25),
            Some(13.5),
        ])) as ArrayRef,
        Arc::new(Float64Array::from(vec![
            Some(-0.75),
            Some(1.5),
            None,
            Some(-2.25),
            Some(4.5),
            None,
            Some(18.25),
            Some(20.5),
            Some(22.75),
            Some(24.25),
            Some(26.5),
        ])) as ArrayRef,
    ];
    let sliced = columns
        .iter()
        .map(|column| column.slice(1, 9))
        .collect::<Vec<_>>();
    RecordBatch::try_new(schema, sliced).expect("sliced primitive batch")
}

fn renamed_primitive_batch() -> RecordBatch {
    primitive_batch(
        vec![
            Some(true),
            None,
            Some(false),
            Some(true),
            Some(false),
            None,
            Some(true),
            Some(false),
            Some(true),
        ],
        vec![
            Some(7),
            None,
            Some(-1),
            Some(128),
            Some(-2048),
            None,
            Some(33),
            Some(44),
            Some(55),
        ],
        true,
        "renamed_id",
        DataType::Int32,
    )
}

fn value_drift_batch() -> RecordBatch {
    primitive_batch(
        vec![
            Some(true),
            None,
            Some(false),
            Some(true),
            Some(false),
            None,
            Some(true),
            Some(false),
            Some(true),
        ],
        vec![
            Some(7001),
            None,
            Some(-1),
            Some(128),
            Some(-2048),
            None,
            Some(33),
            Some(44),
            Some(55),
        ],
        true,
        "id",
        DataType::Int32,
    )
}

fn type_drift_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int64, true),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int64Array::from(vec![Some(7), None, Some(-1)])) as ArrayRef,
        ],
    )
    .expect("type drift batch")
}

fn primitive_batch(
    booleans: Vec<Option<bool>>,
    ids: Vec<Option<i32>>,
    nullable: bool,
    id_name: &str,
    id_type: DataType,
) -> RecordBatch {
    assert_eq!(booleans.len(), ids.len());
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, nullable),
        Field::new(id_name, id_type.clone(), nullable),
        Field::new("count", DataType::Int64, nullable),
        Field::new("ratio", DataType::Float32, nullable),
        Field::new("score", DataType::Float64, nullable),
    ]));
    let counts = ids
        .iter()
        .map(|value| value.map(|v| v as i64 * 10))
        .collect::<Vec<_>>();
    let ratios = ids
        .iter()
        .map(|value| value.map(|v| v as f32 / 4.0))
        .collect::<Vec<_>>();
    let scores = ids
        .iter()
        .map(|value| value.map(|v| v as f64 / 2.0))
        .collect::<Vec<_>>();

    let id_column: ArrayRef = match id_type {
        DataType::Int32 => Arc::new(Int32Array::from(ids)) as ArrayRef,
        other => panic!("unsupported primitive_batch id type {other:?}"),
    };

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(booleans)) as ArrayRef,
            id_column,
            Arc::new(Int64Array::from(counts)) as ArrayRef,
            Arc::new(Float32Array::from(ratios)) as ArrayRef,
            Arc::new(Float64Array::from(scores)) as ArrayRef,
        ],
    )
    .expect("primitive batch")
}

fn encode_lmc2(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("encode LMC2")
}

fn encode_lma1(batch: &RecordBatch) -> Vec<u8> {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch.clone()]).expect("payload");
    encode_arrow_semantic_payload(&payload).expect("encode LMA1")
}
