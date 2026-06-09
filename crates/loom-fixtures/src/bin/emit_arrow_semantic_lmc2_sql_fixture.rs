//! Emit a deterministic multi-column `LMC2(LMA1)` fixture for DuckDB SQL gates.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
    StringArray, StructArray,
};
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

    write_lmc2_batch(
        &out_dir,
        "native-primitives-lmc2.loom",
        native_primitives_batch(),
    );
    write_lma1_batch(
        &out_dir,
        "native-primitives-direct-lma1.loom",
        native_primitives_batch(),
    );
    write_lmc2_batch(&out_dir, "logical-date32-lmc2.loom", logical_date32_batch());
    write_lmc2_batch(&out_dir, "nested-struct-lmc2.loom", nested_struct_batch());
    fs::write(
        out_dir.join("manifest.tsv"),
        "name\tartifact\trows\tcolumns\nmulti-column-lmc2\tLMC2(LMA1)\t5\tid,label,flag,amount,ratio\nmulti-column-direct-lma1\tLMA1\t5\tid,label,flag,amount,ratio\nnative-primitives-lmc2\tLMC2(LMA1)\t5\tid,flag,amount,ratio\nnative-primitives-direct-lma1\tLMA1\t5\tid,flag,amount,ratio\nlogical-date32-lmc2\tLMC2(LMA1)\t3\tevent_date\nnested-struct-lmc2\tLMC2(LMA1)\t3\trecord\n",
    )
    .expect("write manifest");

    println!("wrote {}", out_dir.display());
}

fn native_primitives_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("flag", DataType::Boolean, true),
        Field::new("amount", DataType::Int64, false),
        Field::new("ratio", DataType::Float64, false),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])),
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
    .expect("native primitive record batch")
}

fn logical_date32_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "event_date",
        DataType::Date32,
        true,
    )]));
    RecordBatch::try_new(
        schema,
        vec![Arc::new(Date32Array::from(vec![
            Some(19_000),
            None,
            Some(19_002),
        ]))],
    )
    .expect("logical Date32 record batch")
}

fn nested_struct_batch() -> RecordBatch {
    let child_id: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3]));
    let child_label: ArrayRef =
        Arc::new(StringArray::from(vec![Some("left"), None, Some("right")]));
    let struct_array = StructArray::from(vec![
        (
            Arc::new(Field::new("child_id", DataType::Int32, false)),
            child_id,
        ),
        (
            Arc::new(Field::new("child_label", DataType::Utf8, true)),
            child_label,
        ),
    ]);
    let schema = Arc::new(Schema::new(vec![Field::new(
        "record",
        struct_array.data_type().clone(),
        true,
    )]));
    RecordBatch::try_new(schema, vec![Arc::new(struct_array)]).expect("nested struct record batch")
}

fn write_lmc2_batch(out_dir: &PathBuf, file_name: &str, batch: RecordBatch) {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("semantic payload");
    let wrapped = encode_arrow_semantic_container_payload(&payload).expect("encode LMC2");
    fs::write(out_dir.join(file_name), wrapped).expect("write LMC2 fixture");
}

fn write_lma1_batch(out_dir: &PathBuf, file_name: &str, batch: RecordBatch) {
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("semantic payload");
    let direct = encode_arrow_semantic_payload(&payload).expect("encode direct LMA1");
    fs::write(out_dir.join(file_name), direct).expect("write direct LMA1 fixture");
}
