//! Emit a deterministic multi-column `LMC2(LMA1)` fixture for DuckDB SQL gates.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{BooleanArray, Float64Array, Int32Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::{
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
};

fn main() {
    let out_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/loom-duckdb-lmc2-sql"));
    fs::create_dir_all(&out_dir).expect("create output directory");

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("label", DataType::Utf8, true),
        Field::new("flag", DataType::Boolean, true),
        Field::new("amount", DataType::Int64, false),
        Field::new("ratio", DataType::Float64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])),
            Arc::new(StringArray::from(vec![
                Some("alpha"),
                None,
                Some("gamma"),
                Some("delta"),
                None,
            ])),
            Arc::new(BooleanArray::from(vec![
                Some(true),
                None,
                Some(false),
                Some(true),
                Some(false),
            ])),
            Arc::new(Int64Array::from(vec![10, 20, 30, 40, 50])),
            Arc::new(Float64Array::from(vec![1.5, 2.5, 3.5, 4.5, 5.5])),
        ],
    )
    .expect("record batch");
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("semantic payload");
    let direct = encode_arrow_semantic_payload(&payload).expect("encode direct LMA1");
    let wrapped = encode_arrow_semantic_container_payload(&payload).expect("encode LMC2");

    fs::write(out_dir.join("multi-column-lmc2.loom"), wrapped).expect("write LMC2 fixture");
    fs::write(out_dir.join("multi-column-direct-lma1.loom"), direct)
        .expect("write direct LMA1 regression fixture");
    fs::write(
        out_dir.join("manifest.tsv"),
        "name\tartifact\trows\tcolumns\nmulti-column-lmc2\tLMC2(LMA1)\t5\tid,label,flag,amount,ratio\nmulti-column-direct-lma1\tLMA1\t5\tid,label,flag,amount,ratio\n",
    )
    .expect("write manifest");

    println!("wrote {}", out_dir.display());
}
