//! Plan 3 Tier 1 E2E: Parquet -> auto-generated L2Core IR -> interpreter.
//!
//! Proves `generate_decode_ir_from_parquet` emits a *real* decode body (not an
//! empty program) that, when run by the production interpreter over the raw
//! host buffer `parquet_to_raw_host` produces, reproduces the Parquet file's
//! actual column values — for the Tier 1 types (non-null i32/i64/f32/f64/bool).

use std::sync::Arc;

use arrow_array::{
    Array, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
};
use arrow_schema::{DataType, Field, Schema};

use loom_ffi::l2core_interp::{interpret_l2core, InputSlices};
use loom_ffi::loom_parquet_ingress::{generate_decode_ir_from_parquet, parquet_to_raw_host};
use loom_ffi::parquet::arrow::ArrowWriter;
use loom_ir_core::full_verifier::verify_l2_core;
use loom_ir_core::l2_core::Capability;

fn write_parquet(batch: &RecordBatch, path: &std::path::Path) {
    let file = std::fs::File::create(path).expect("create parquet");
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None).expect("arrow writer");
    writer.write(batch).expect("write batch");
    writer.close().expect("close writer");
}

/// Window the host buffer into per-InputSlice byte slices keyed by capability id.
fn inputs_from_program<'a>(
    program: &loom_ir_core::l2_core::L2CoreProgram,
    host: &'a [u8],
) -> InputSlices<'a> {
    let mut inputs = InputSlices::new();
    for cap in &program.capabilities {
        if let Capability::InputSlice(s) = cap {
            let start = s.offset as usize;
            let end = start + s.length as usize;
            inputs.insert(s.id.clone(), &host[start..end]);
        }
    }
    inputs
}

#[test]
fn tier1_mixed_fixed_width_roundtrip() {
    // All Tier 1 fixed-width non-null types: i32/i64 (direct) + f32/f64/bool
    // (via Bitcast). Each must round-trip through auto IR -> interpreter.
    let schema = Arc::new(Schema::new(vec![
        Field::new("i32", DataType::Int32, false),
        Field::new("i64", DataType::Int64, false),
        Field::new("f32", DataType::Float32, false),
        Field::new("f64", DataType::Float64, false),
        Field::new("flag", DataType::Boolean, false),
    ]));
    let i32s = vec![10i32, -20, 30, 0, 2_000_000, 7];
    let i64s = vec![1i64, 2, 3, -4, 5, 6_000_000_000];
    let f32s = vec![0.0f32, -3.5, 1.25, 2.5, 100.0, -0.001];
    let f64s = vec![1.5f64, 2.5, 3.5, 4.5, 5.5, 6.5];
    let bools = vec![true, false, true, true, false, false];

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(i32s.clone())),
            Arc::new(Int64Array::from(i64s.clone())),
            Arc::new(Float32Array::from(f32s.clone())),
            Arc::new(Float64Array::from(f64s.clone())),
            Arc::new(BooleanArray::from(bools.clone())),
        ],
    )
    .expect("batch");

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tier1.parquet");
    write_parquet(&batch, &path);

    let program = generate_decode_ir_from_parquet(&path)
        .expect("gen ir")
        .expect("some program");
    assert!(!program.body.is_empty(), "generated body must not be empty");
    let report = verify_l2_core(&program);
    assert!(
        report.is_ok(),
        "generated IR must verify: {:?}",
        report.diagnostics()
    );

    let builders = program
        .capabilities
        .iter()
        .filter(|c| matches!(c, Capability::OutputBuilder(_)))
        .count();
    assert_eq!(builders, 5, "all five fixed-width columns are Tier 1");

    let host = parquet_to_raw_host(&path).expect("raw host");
    let inputs = inputs_from_program(&program, &host);
    let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
    assert_eq!(columns.len(), 5);

    let dec_i32 = Int32Array::from(columns[0].data.clone());
    assert_eq!(dec_i32.values(), &i32s[..]);
    let dec_i64 = Int64Array::from(columns[1].data.clone());
    assert_eq!(dec_i64.values(), &i64s[..]);
    let dec_f32 = Float32Array::from(columns[2].data.clone());
    assert_eq!(dec_f32.values(), &f32s[..]);
    let dec_f64 = Float64Array::from(columns[3].data.clone());
    assert_eq!(dec_f64.values(), &f64s[..]);
    let dec_bool = BooleanArray::from(columns[4].data.clone());
    for (i, want) in bools.iter().enumerate() {
        assert_eq!(dec_bool.value(i), *want, "bool row {i}");
    }
}

#[test]
fn tier2_nullable_columns_roundtrip_and_skip_utf8() {
    // Tier 2: nullable i32 + nullable f64 round-trip (null positions + values);
    // a Utf8 column (Tier 3) is skipped.
    let schema = Arc::new(Schema::new(vec![
        Field::new("maybe_i32", DataType::Int32, true),
        Field::new("maybe_f64", DataType::Float64, true),
        Field::new("name", DataType::Utf8, false),
    ]));
    let i32s = vec![Some(1), None, Some(3), None, Some(5)];
    let f64s = vec![None, Some(2.5), Some(3.5), None, Some(5.5)];
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(i32s.clone())),
            Arc::new(Float64Array::from(f64s.clone())),
            Arc::new(arrow_array::StringArray::from(vec!["a", "b", "c", "d", "e"])),
        ],
    )
    .expect("batch");

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nullable.parquet");
    write_parquet(&batch, &path);

    let program = generate_decode_ir_from_parquet(&path)
        .expect("gen ir")
        .expect("some program");
    let report = verify_l2_core(&program);
    assert!(
        report.is_ok(),
        "nullable IR must verify: {:?}",
        report.diagnostics()
    );

    // Two nullable columns decoded; Utf8 skipped.
    let builders = program
        .capabilities
        .iter()
        .filter(|c| matches!(c, Capability::OutputBuilder(_)))
        .count();
    assert_eq!(builders, 2, "the two nullable fixed-width columns are decoded");

    let host = parquet_to_raw_host(&path).expect("raw host");
    let inputs = inputs_from_program(&program, &host);
    let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
    assert_eq!(columns.len(), 2);

    let dec_i32 = Int32Array::from(columns[0].data.clone());
    assert_eq!(dec_i32.len(), 5);
    for (i, want) in i32s.iter().enumerate() {
        assert_eq!(dec_i32.is_null(i), want.is_none(), "i32 null@{i}");
        if let Some(v) = want {
            assert_eq!(dec_i32.value(i), *v, "i32 val@{i}");
        }
    }
    let dec_f64 = Float64Array::from(columns[1].data.clone());
    for (i, want) in f64s.iter().enumerate() {
        assert_eq!(dec_f64.is_null(i), want.is_none(), "f64 null@{i}");
        if let Some(v) = want {
            assert_eq!(dec_f64.value(i), *v, "f64 val@{i}");
        }
    }
}
