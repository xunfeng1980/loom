//! Differential oracle: production L2Core interpreter vs. the offline LMA1 path.
//!
//! Plan 1 Task C. The general interpreter ([`interpret_l2core`]) is the single
//! production decode engine. The legacy `execute_native_arrow_semantic` LMA1
//! path is retained only as an offline differential oracle. This test pins the
//! contract: for the same logical column, both engines must agree.
//!
//! LMA1 oracle side : RecordBatch → encode (LMC2/LMA1) → execute_native_arrow_semantic.
//! Interpreter side : L2Core copy program + raw little-endian bytes → interpret_l2core.

use std::sync::Arc;

use arrow_array::{Array, ArrayRef, Int32Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};

use loom_ffi::arrow_semantic::ArrowSemanticPayload as Payload;
use loom_ffi::arrow_semantic_codec::encode_arrow_semantic_container_payload as encode;
use loom_ffi::l2core_interp::{interpret_l2core, InputSlices};
use loom_ffi::native_arrow_semantic::execute_native_arrow_semantic as oracle_decode;

use loom_ir_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
};

/// Build the canonical i32 copy program in the interpreter's byte-offset
/// convention: `for i in 0..n { v = read(in, i*4, 4); append(out, v) }`.
fn i32_copy_program(rows: u64) -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "in".to_string(),
                offset: 0,
                length: rows * 4,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output_col".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: rows,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(rows),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(rows)),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: "in".to_string(),
                    offset: ScalarExpr::Mul(
                        Box::new(ScalarExpr::Var("i".to_string())),
                        Box::new(ScalarExpr::Const(ScalarValue::UInt64(4))),
                    ),
                    width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                    bind: "v".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: "output_col".to_string(),
                    value: ScalarExpr::Var("v".to_string()),
                },
            ],
        }],
    }
}

#[test]
fn interp_matches_lma1_oracle_i32_non_null() {
    let vals: Vec<i32> = vec![10, -20, 30, 0, 2_000_000, 7];

    // ── Oracle side: LMA1 roundtrip ─────────────────────────────────────────
    let schema = Arc::new(Schema::new(vec![Field::new("col", DataType::Int32, false)]));
    let input = RecordBatch::try_new(
        schema,
        vec![Arc::new(Int32Array::from(vals.clone())) as ArrayRef],
    )
    .expect("input batch");
    let payload = Payload::from_record_batches(&[input]).expect("payload");
    let artifact = encode(&payload).expect("encode LMC2/LMA1");
    let report = oracle_decode(&artifact);
    assert!(
        report.is_supported(),
        "oracle decode unsupported: {:?}",
        report.diagnostics()
    );
    let oracle_batch = report.output().expect("oracle output").clone();
    let oracle_vals: Vec<i32> = oracle_batch
        .column(0)
        .as_any()
        .downcast_ref::<Int32Array>()
        .expect("i32 oracle column")
        .values()
        .to_vec();

    // ── Interpreter side: copy program over raw LE bytes ────────────────────
    let program = i32_copy_program(vals.len() as u64);
    let bytes: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut inputs = InputSlices::new();
    inputs.insert("in".to_string(), bytes.as_slice());
    let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
    let interp_vals: Vec<i32> = Int32Array::from(columns[0].data.clone())
        .values()
        .to_vec();

    // ── Differential assertion ──────────────────────────────────────────────
    assert_eq!(oracle_vals, vals, "oracle must reproduce input");
    assert_eq!(
        interp_vals, oracle_vals,
        "production interpreter must agree with LMA1 oracle"
    );
}
