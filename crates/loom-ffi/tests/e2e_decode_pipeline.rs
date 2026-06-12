//! E2E decode pipeline tests — sidecar → verify → interp decode → Arrow output.
//!
//! Exercises the complete execution loop from L2Core IR bytes through semantic
//! verification, interpreter decode, and Arrow RecordBatch reconstruction.
//! Verifies: output row counts, column schema, nullability, and value correctness.

use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array,
    RecordBatch, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use loom_ffi::arrow_semantic::ArrowSemanticPayload as Payload;
use loom_ffi::arrow_semantic_codec::encode_arrow_semantic_container_payload as encode;
use loom_ffi::native_arrow_semantic::execute_native_arrow_semantic as decode;
use loom_ffi::artifact_types::{
    verify_artifact,
    ArtifactVerificationOptions,
};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;

// ── Full roundtrip: RecordBatch → LMC2 → interp decode → RecordBatch ───────

#[test]
fn e2e_roundtrip_primitive_non_nullable() {
    let input = primitive_batch(5, false);
    let output = roundtrip(&input);
    assert_eq!(output.num_rows(), 5);
    assert_eq!(output.num_columns(), 5);
    assert_batch_eq(&input, &output, "primitive non-nullable");
}

#[test]
fn e2e_roundtrip_primitive_nullable() {
    let input = primitive_batch(9, true);
    let output = roundtrip(&input);
    assert_eq!(output.num_rows(), 9);
    assert_eq!(output.num_columns(), 5);
    assert_batch_eq(&input, &output, "primitive nullable");
}

#[test]
fn e2e_roundtrip_boolean() {
    let schema = Arc::new(Schema::new(vec![Field::new("flag", DataType::Boolean, true)]));
    let input = RecordBatch::try_new(
        schema,
        vec![Arc::new(BooleanArray::from(vec![
            Some(true), None, Some(false), Some(true), Some(false),
            None, Some(true), Some(false), None, Some(true),
        ])) as ArrayRef],
    )
    .expect("boolean batch");
    let output = roundtrip(&input);
    assert_eq!(output.num_rows(), 10);
    assert_batch_eq(&input, &output, "boolean");
}

#[test]
fn e2e_roundtrip_float32() {
    let schema = Arc::new(Schema::new(vec![Field::new("ratio", DataType::Float32, true)]));
    let input = RecordBatch::try_new(
        schema,
        vec![Arc::new(Float32Array::from(vec![
            Some(0.0), None, Some(-3.5), Some(1.25),
        ])) as ArrayRef],
    )
    .expect("float32 batch");
    let output = roundtrip(&input);
    assert_eq!(output.num_rows(), 4);
    assert_batch_eq(&input, &output, "float32");
}

#[test]
fn e2e_roundtrip_float64() {
    let schema = Arc::new(Schema::new(vec![Field::new("score", DataType::Float64, false)]));
    let input = RecordBatch::try_new(
        schema,
        vec![Arc::new(Float64Array::from(vec![1.5, 2.5, 3.5, 4.5, 5.5])) as ArrayRef],
    )
    .expect("float64 batch");
    let output = roundtrip(&input);
    assert_eq!(output.num_rows(), 5);
    assert_batch_eq(&input, &output, "float64");
}

#[test]
fn e2e_roundtrip_int32_int64_mixed() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("i32", DataType::Int32, false),
        Field::new("i64", DataType::Int64, true),
    ]));
    let input = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![10, 20, 30, 40, 50, 60])) as ArrayRef,
            Arc::new(Int64Array::from(vec![
                Some(100), None, Some(300), None, Some(500), Some(600),
            ])) as ArrayRef,
        ],
    )
    .expect("mixed batch");
    let output = roundtrip(&input);
    assert_eq!(output.num_rows(), 6);
    assert_eq!(output.num_columns(), 2);
    assert_batch_eq(&input, &output, "int32 int64 mixed");
}

// ── Sidecar-specific: external sidecar file read + verify ──────────────────

#[test]
fn e2e_sidecar_external_file_read_verify() {
    use loom_ir_core::l2_core::{
        Capability, InputSliceCapability, L2CoreProgram, L2DataType,
        OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue, L2CoreStmt,
    };
    use loom_ir_core::l2core_codec::encode_l2core_program;
    use loom_ir_core::sidecar::{ChunkBinding, SidecarOverlay};
    use loom_ir_core::full_verifier::verify_l2_core_bytes;

    // Create a minimal valid L2Core program.
    let program = L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input".to_string(),
                offset: 0,
                length: 100,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: 100,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(10),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(10)),
            body: vec![L2CoreStmt::AppendValue {
                builder: "output".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(42)),
            }],
        }],
    };

    let ir_bytes = encode_l2core_program(&program);
    let binding = ChunkBinding {
        granule_id: "output".to_string(),
        host_data_range: (0, 100),
        content_hash: loom_ir_core::sidecar::compute_chunk_hash(&[0u8; 100]),
        ir_identity: loom_ir_core::l2core_codec::l2core_program_hash(&program),
    };

    let overlay = SidecarOverlay {
        ir_bytes: ir_bytes.clone(),
        bindings: vec![binding],
    };

    // Write sidecar to a temp file, then read it back.
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let sidecar_path = tmpdir.path().join("test.loomsidecar");
    std::fs::write(&sidecar_path, overlay.encode()).expect("write sidecar");

    let raw = std::fs::read(&sidecar_path).expect("read sidecar");
    let decoded = SidecarOverlay::decode(&raw).expect("decode overlay");

    assert_eq!(decoded.ir_bytes, ir_bytes);
    assert_eq!(decoded.bindings.len(), 1);
    assert_eq!(decoded.bindings[0].granule_id, "output");

    // Verify the IR.
    let report = verify_l2_core_bytes(&decoded.ir_bytes);
    assert!(report.is_ok(), "verification failed: {:?}", report.diagnostics());

    // Verify chunk binding.
    let hv = loom_ir_core::sidecar::verify_chunk_binding(
        &decoded.bindings[0],
        &[0u8; 100],
    );
    assert!(hv.matches, "chunk hash mismatch");
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn roundtrip(input: &RecordBatch) -> RecordBatch {
    let payload = Payload::from_record_batches(&[input.clone()]).expect("payload");
    let bytes = encode(&payload).expect("encode LMC2");
    let report = decode(&bytes);

    assert!(
        report.is_supported(),
        "decode unsupported: {:?}",
        report.diagnostics()
    );
    let output = report.output().expect("decode output").clone();

    // Also verify the container format is valid.
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&bytes, &registry, &ArtifactVerificationOptions::default());
    assert!(
        verification.is_ok(),
        "artifact verification failed: {:?}",
        verification.diagnostics()
    );

    output
}

fn assert_batch_eq(expected: &RecordBatch, actual: &RecordBatch, label: &str) {
    assert_eq!(
        expected.schema(),
        actual.schema(),
        "{label}: schema mismatch"
    );
    assert_eq!(
        expected.num_rows(),
        actual.num_rows(),
        "{label}: row count mismatch"
    );
    assert_eq!(
        expected.num_columns(),
        actual.num_columns(),
        "{label}: column count mismatch"
    );

    for col_idx in 0..expected.num_columns() {
        let exp_col = expected.column(col_idx);
        let act_col = actual.column(col_idx);
        assert_eq!(
            exp_col.data_type(),
            act_col.data_type(),
            "{label}: column {col_idx} type mismatch"
        );
        assert_eq!(
            exp_col.null_count(),
            act_col.null_count(),
            "{label}: column {col_idx} null count mismatch"
        );
        for row in 0..expected.num_rows() {
            assert_eq!(
                exp_col.is_null(row),
                act_col.is_null(row),
                "{label}: column {col_idx} row {row} null mismatch"
            );
        }
        // Compare values only for non-null rows.
        match exp_col.data_type() {
            DataType::Boolean => {
                let exp = exp_col.as_any().downcast_ref::<BooleanArray>().unwrap();
                let act = act_col.as_any().downcast_ref::<BooleanArray>().unwrap();
                for row in 0..expected.num_rows() {
                    if !exp.is_null(row) {
                        assert_eq!(
                            exp.value(row),
                            act.value(row),
                            "{label}: bool col {col_idx} row {row}"
                        );
                    }
                }
            }
            DataType::Int32 => {
                let exp = exp_col.as_any().downcast_ref::<Int32Array>().unwrap();
                let act = act_col.as_any().downcast_ref::<Int32Array>().unwrap();
                for row in 0..expected.num_rows() {
                    if !exp.is_null(row) {
                        assert_eq!(
                            exp.value(row),
                            act.value(row),
                            "{label}: i32 col {col_idx} row {row}"
                        );
                    }
                }
            }
            DataType::Int64 => {
                let exp = exp_col.as_any().downcast_ref::<Int64Array>().unwrap();
                let act = act_col.as_any().downcast_ref::<Int64Array>().unwrap();
                for row in 0..expected.num_rows() {
                    if !exp.is_null(row) {
                        assert_eq!(
                            exp.value(row),
                            act.value(row),
                            "{label}: i64 col {col_idx} row {row}"
                        );
                    }
                }
            }
            DataType::Float32 => {
                let exp = exp_col.as_any().downcast_ref::<Float32Array>().unwrap();
                let act = act_col.as_any().downcast_ref::<Float32Array>().unwrap();
                for row in 0..expected.num_rows() {
                    if !exp.is_null(row) {
                        assert_eq!(
                            exp.value(row),
                            act.value(row),
                            "{label}: f32 col {col_idx} row {row}"
                        );
                    }
                }
            }
            DataType::Float64 => {
                let exp = exp_col.as_any().downcast_ref::<Float64Array>().unwrap();
                let act = act_col.as_any().downcast_ref::<Float64Array>().unwrap();
                for row in 0..expected.num_rows() {
                    if !exp.is_null(row) {
                        assert_eq!(
                            exp.value(row),
                            act.value(row),
                            "{label}: f64 col {col_idx} row {row}"
                        );
                    }
                }
            }
            DataType::Utf8 => {
                let exp = exp_col.as_any().downcast_ref::<StringArray>().unwrap();
                let act = act_col.as_any().downcast_ref::<StringArray>().unwrap();
                for row in 0..expected.num_rows() {
                    if !exp.is_null(row) {
                        assert_eq!(
                            exp.value(row),
                            act.value(row),
                            "{label}: utf8 col {col_idx} row {row}"
                        );
                    }
                }
            }
            other => panic!("{label}: unsupported type for comparison: {other:?}"),
        }
    }
}

fn primitive_batch(rows: usize, nullable: bool) -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("b", DataType::Boolean, nullable),
        Field::new("i32", DataType::Int32, nullable),
        Field::new("i64", DataType::Int64, nullable),
        Field::new("f32", DataType::Float32, nullable),
        Field::new("f64", DataType::Float64, nullable),
    ]));

    let bools: Vec<Option<bool>> = (0..rows)
        .map(|i| if nullable && i % 3 == 0 { None } else { Some(i % 2 == 0) })
        .collect();
    let i32s: Vec<Option<i32>> = (0..rows)
        .map(|i| if nullable && i % 4 == 0 { None } else { Some(i as i32 * 7 - 3) })
        .collect();
    let i64s: Vec<Option<i64>> = (0..rows)
        .map(|i| if nullable && i % 3 == 0 { None } else { Some(i as i64 * 100) })
        .collect();
    let f32s: Vec<Option<f32>> = (0..rows)
        .map(|i| {
            if nullable && i % 5 == 0 {
                None
            } else {
                Some(i as f32 * 0.75)
            }
        })
        .collect();
    let f64s: Vec<Option<f64>> = (0..rows)
        .map(|i| {
            if nullable && i % 2 == 0 {
                None
            } else {
                Some(i as f64 * 1.5)
            }
        })
        .collect();

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(bools)) as ArrayRef,
            Arc::new(Int32Array::from(i32s)) as ArrayRef,
            Arc::new(Int64Array::from(i64s)) as ArrayRef,
            Arc::new(Float32Array::from(f32s)) as ArrayRef,
            Arc::new(Float64Array::from(f64s)) as ArrayRef,
        ],
    )
    .expect("primitive batch")
}
