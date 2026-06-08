//! Emit deterministic Loom layout payloads for the DuckDB SQL smoke gate.

use std::fs;
use std::path::Path;

use arrow_schema::DataType;
use loom_core::alp_params::{AlpOutputType, AlpParams};
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload};
use loom_core::l1_model::LayoutDescription;
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use loom_fixtures::vortex_reader;
use vortex_array::arrays::{DictArray, PrimitiveArray, VarBinArray};
use vortex_array::dtype::{DType, Nullability};
use vortex_array::{IntoArray, VortexSessionExecute, LEGACY_SESSION};
use vortex_fastlanes::{BitPackedData, FoR, RLEData};

const OUT_DIR: &str = "target/loom-duckdb-fixtures";

fn main() {
    let out_dir = Path::new(OUT_DIR);
    fs::create_dir_all(out_dir).expect("create fixture output directory");

    let mut manifest = String::from(
        "name\tpayload_kind\tcontainer\ttype\trows\tcount\tnon_null_count\tsum\tmin\tmax\n",
    );
    emit_bitpack(out_dir, &mut manifest);
    emit_nullable_bitpack(out_dir, &mut manifest);
    emit_for(out_dir, &mut manifest);
    emit_dict(out_dir, &mut manifest);
    emit_rle(out_dir, &mut manifest);
    emit_fsst(out_dir, &mut manifest);
    emit_fsst_edge(out_dir, &mut manifest);
    emit_dict_fsst(out_dir, &mut manifest);
    emit_alp_f32(out_dir, &mut manifest);
    emit_alp_f64(out_dir, &mut manifest);
    emit_mixed_table(out_dir);
    emit_native_primitives_table(out_dir, &mut manifest);

    fs::write(out_dir.join("manifest.tsv"), manifest).expect("write manifest");
    println!("wrote {}", out_dir.display());
}

fn emit_mixed_table(out_dir: &Path) {
    let id_desc = {
        let values = [1i32, 2, 3, 4, 5];
        let input = PrimitiveArray::from_iter(values);
        let mut ctx = LEGACY_SESSION.create_execution_ctx();
        let packed = BitPackedData::encode(&input.into_array(), 3, &mut ctx)
            .expect("BitPackedData::encode failed");
        LayoutDescription {
            data_type: DataType::Int32,
            root: vortex_reader::from_bitpacked_array(&packed),
            row_count: values.len(),
        }
    };

    let flag_desc = {
        LayoutDescription {
            data_type: DataType::Boolean,
            root: loom_core::l1_model::LayoutNode::Raw {
                data: vec![1, 0, 1, 1, 0],
                elem_size: 1,
                count: 5,
            },
            row_count: 5,
        }
    };

    let label_desc = {
        let rows = [
            Some("alpha"),
            None,
            Some("beta"),
            Some("gamma"),
            Some("delta"),
        ];
        let fsst = make_fsst(&rows);
        LayoutDescription {
            data_type: DataType::Utf8,
            root: vortex_reader::from_fsst_array(&fsst),
            row_count: rows.len(),
        }
    };

    let table = TableDescription {
        row_count: 5,
        columns: vec![
            TableColumn {
                name: "id".to_string(),
                layout: id_desc,
            },
            TableColumn {
                name: "flag".to_string(),
                layout: flag_desc,
            },
            TableColumn {
                name: "label".to_string(),
                layout: label_desc,
            },
        ],
    };

    let payload = encode_table_payload(&table).expect("encode mixed table payload");
    let payload = wrap_table_payload(&payload).expect("wrap mixed table payload");
    fs::write(out_dir.join("mixed-table.loom"), payload).expect("write table payload");
}

fn emit_native_primitives_table(out_dir: &Path, manifest: &mut String) {
    let row_count = 4usize;
    let table = TableDescription {
        row_count,
        columns: vec![
            TableColumn {
                name: "i32_col".to_string(),
                layout: raw_zeros(DataType::Int32, 4, row_count),
            },
            TableColumn {
                name: "i64_col".to_string(),
                layout: raw_zeros(DataType::Int64, 8, row_count),
            },
            TableColumn {
                name: "f32_col".to_string(),
                layout: raw_zeros(DataType::Float32, 4, row_count),
            },
            TableColumn {
                name: "f64_col".to_string(),
                layout: raw_zeros(DataType::Float64, 8, row_count),
            },
        ],
    };

    let payload = encode_table_payload(&table).expect("encode native primitives table payload");
    let payload = wrap_table_payload(&payload).expect("wrap native primitives table payload");
    fs::write(out_dir.join("native-primitives-table.loom"), payload)
        .expect("write native primitives table payload");
    manifest.push_str(
        "native-primitives-table\tLMT1\tLMC1\ti32|i64|f32|f64\t0,0,0.0,0.0|0,0,0.0,0.0|0,0,0.0,0.0|0,0,0.0,0.0\t4\t4\t0|0|0.0|0.0\t0|0|0.0|0.0\t0|0|0.0|0.0\n",
    );
}

fn raw_zeros(data_type: DataType, elem_size: u8, row_count: usize) -> LayoutDescription {
    LayoutDescription {
        data_type,
        root: loom_core::l1_model::LayoutNode::Raw {
            data: vec![0; row_count * elem_size as usize],
            elem_size,
            count: row_count,
        },
        row_count,
    }
}

fn emit_nullable_bitpack(out_dir: &Path, manifest: &mut String) {
    let values = [Some(1i32), None, Some(7), Some(3), None];
    let input = PrimitiveArray::from_option_iter(values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 3, &mut ctx)
        .expect("BitPackedData::encode failed");
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_bitpacked_array(&packed),
        row_count: values.len(),
    };
    write_payload(out_dir, "bitpack-nullable-i32", &desc);
    manifest.push_str("bitpack-nullable-i32\tLMP1\tLMC1\ti32\t1|NULL|7|3|NULL\t5\t3\t11\t1\t7\n");
}

fn emit_bitpack(out_dir: &Path, manifest: &mut String) {
    let values = [1i32, 2, 3, 4];
    let input = PrimitiveArray::from_iter(values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 3, &mut ctx)
        .expect("BitPackedData::encode failed");
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_bitpacked_array(&packed),
        row_count: values.len(),
    };
    write_payload(out_dir, "bitpack-i32", &desc);
    manifest.push_str("bitpack-i32\tLMP1\tLMC1\ti32\t1|2|3|4\t4\t4\t10\t1\t4\n");
}

fn emit_for(out_dir: &Path, manifest: &mut String) {
    let deltas = [0i32, 1, 2];
    let reference = 10i32;
    let input = PrimitiveArray::from_iter(deltas);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 2, &mut ctx)
        .expect("BitPackedData::encode failed");
    let for_array = FoR::try_new(packed.into_array(), reference.into()).expect("FoR::try_new");
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_for_array(&for_array),
        row_count: deltas.len(),
    };
    write_payload(out_dir, "for-i32", &desc);
    manifest.push_str("for-i32\tLMP1\tLMC1\ti32\t10|11|12\t3\t3\t33\t10\t12\n");
}

fn emit_dict(out_dir: &Path, manifest: &mut String) {
    let values = PrimitiveArray::from_iter([10i32, 20, 30]);
    let codes = PrimitiveArray::from_iter([2i32, 0, 1, 2]);
    let dict = DictArray::try_new(codes.into_array(), values.into_array()).expect("dict");
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_dict_array(&dict),
        row_count: 4,
    };
    write_payload(out_dir, "dict-i32", &desc);
    manifest.push_str("dict-i32\tLMP1\tLMC1\ti32\t30|10|20|30\t4\t4\t90\t10\t30\n");
}

fn emit_rle(out_dir: &Path, manifest: &mut String) {
    let input = PrimitiveArray::from_iter([1u32, 1, 2, 2, 2, 3]);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let rle = RLEData::encode(input.as_view(), &mut ctx).expect("RLEData::encode failed");
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_rle_array(&rle),
        row_count: 6,
    };
    write_payload(out_dir, "rle-i32", &desc);
    manifest.push_str("rle-i32\tLMP1\tLMC1\ti32\t1|1|2|2|2|3\t6\t6\t11\t1\t3\n");
}

fn emit_fsst(out_dir: &Path, manifest: &mut String) {
    let rows = [Some("alpha"), None, Some("beta")];
    let fsst = make_fsst(&rows);
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: vortex_reader::from_fsst_array(&fsst),
        row_count: rows.len(),
    };
    write_payload(out_dir, "fsst-utf8", &desc);
    manifest.push_str("fsst-utf8\tLMP1\tLMC1\tutf8\talpha|NULL|beta\t3\t2\t\talpha\tbeta\n");
}

fn emit_fsst_edge(out_dir: &Path, manifest: &mut String) {
    let rows = [Some(""), Some("abcdefgh"), Some("escape-heavy-zzzz")];
    let fsst = make_fsst(&rows);
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: vortex_reader::from_fsst_array(&fsst),
        row_count: rows.len(),
    };
    write_payload(out_dir, "fsst-edge-utf8", &desc);
    manifest
        .push_str("fsst-edge-utf8\tLMP1\tLMC1\tutf8\t|abcdefgh|escape-heavy-zzzz\t3\t3\t\t\t\n");
}

fn emit_dict_fsst(out_dir: &Path, manifest: &mut String) {
    let values = VarBinArray::from_iter(
        [Some("alpha"), Some("beta"), Some("gamma")],
        DType::Utf8(Nullability::Nullable),
    );
    let compressor = vortex_fsst::fsst_train_compressor(&values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let fsst_values =
        vortex_fsst::fsst_compress(&values, values.len(), values.dtype(), &compressor, &mut ctx);
    let codes = PrimitiveArray::from_iter([1i32, 0, 2, 1]);
    let dict = DictArray::try_new(codes.into_array(), fsst_values.into_array()).expect("dict");
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: vortex_reader::from_dict_array(&dict),
        row_count: 4,
    };
    write_payload(out_dir, "dict-fsst-utf8", &desc);
    manifest.push_str(
        "dict-fsst-utf8\tLMP1\tLMC1\tutf8\tbeta|alpha|gamma|beta\t4\t4\t\talpha\tgamma\n",
    );
}

fn emit_alp_f32(out_dir: &Path, manifest: &mut String) {
    let params = AlpParams {
        output_type: AlpOutputType::Float32,
        decimal_exponent: -2,
        mantissas: vec![125, -250, 0, 125, -250],
        validity: Some(vec![true, true, true, true, false]),
    };
    let desc = LayoutDescription {
        data_type: DataType::Float32,
        root: loom_core::l1_model::LayoutNode::KernelEscape {
            kernel_id: 1,
            params: params.encode(),
            count: 5,
        },
        row_count: 5,
    };
    write_payload(out_dir, "alp-f32", &desc);
    manifest.push_str("alp-f32\tLMP1\tLMC1\tf32\t1.25|-2.5|0|1.25|NULL\t5\t4\t0\t-2.5\t1.25\n");
}

fn emit_alp_f64(out_dir: &Path, manifest: &mut String) {
    let params = AlpParams {
        output_type: AlpOutputType::Float64,
        decimal_exponent: -3,
        mantissas: vec![10125, -3500, 0, -3500, 10125],
        validity: Some(vec![true, true, true, false, true]),
    };
    let desc = LayoutDescription {
        data_type: DataType::Float64,
        root: loom_core::l1_model::LayoutNode::KernelEscape {
            kernel_id: 1,
            params: params.encode(),
            count: 5,
        },
        row_count: 5,
    };
    write_payload(out_dir, "alp-f64", &desc);
    manifest.push_str(
        "alp-f64\tLMP1\tLMC1\tf64\t10.125|-3.5|0|NULL|10.125\t5\t4\t16.75\t-3.5\t10.125\n",
    );
}

fn make_fsst(rows: &[Option<&str>]) -> vortex_fsst::FSSTArray {
    let values = VarBinArray::from_iter(rows.iter().copied(), DType::Utf8(Nullability::Nullable));
    let compressor = vortex_fsst::fsst_train_compressor(&values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    vortex_fsst::fsst_compress(&values, values.len(), values.dtype(), &compressor, &mut ctx)
}

fn write_payload(out_dir: &Path, name: &str, desc: &LayoutDescription) {
    let payload = encode_layout_payload(desc);
    let payload = wrap_layout_payload(&payload).expect("wrap layout payload");
    fs::write(out_dir.join(format!("{name}.loom")), payload).expect("write payload");
}
