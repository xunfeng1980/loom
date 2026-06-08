//! Descriptor roundtrip coverage for real MVP0 fixture layouts.

use arrow::array::{Array, Int32Array, StringArray};
use arrow_schema::DataType;
use loom_core::descriptor::{
    descriptor_text_to_payload, from_descriptor_text, payload_to_descriptor_text,
    to_descriptor_text,
};
use loom_core::l1_model::{decode_layout_to_array_data, LayoutDescription};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::encode_layout_payload;
use vortex_array::arrays::{DictArray, PrimitiveArray, VarBinArray};
use vortex_array::dtype::{DType, Nullability};
use vortex_array::{IntoArray, VortexSessionExecute, LEGACY_SESSION};
use vortex_fastlanes::{BitPackedData, FoR, RLEData};

use loom_fixtures::vortex_reader;

#[test]
fn descriptor_roundtrips_all_mvp0_payload_shapes() {
    let fixtures = vec![
        bitpack_desc(),
        for_desc(),
        dict_desc(),
        rle_desc(),
        fsst_desc(),
        dict_fsst_desc(),
    ];

    for (name, desc) in fixtures {
        assert_descriptor_decodes_like_original(name, &desc);

        let payload = encode_layout_payload(&desc);
        let text = payload_to_descriptor_text(&payload).expect("payload -> descriptor text");
        let encoded = descriptor_text_to_payload(&text).expect("descriptor text -> payload");
        assert_eq!(payload, encoded, "payload roundtrip mismatch for {name}");
    }
}

#[test]
fn descriptor_roundtrips_extra_nullable_and_edge_samples() {
    assert_descriptor_decodes_like_original("nullable-bitpack-extra", &nullable_bitpack_desc());
    assert_descriptor_decodes_like_original("fsst-edge-extra", &fsst_edge_desc());
}

fn assert_descriptor_decodes_like_original(name: &str, desc: &LayoutDescription) {
    let text = to_descriptor_text(desc).expect("layout -> descriptor text");
    let parsed = from_descriptor_text(&text).expect("descriptor text -> layout");

    assert_eq!(
        to_descriptor_text(&parsed).expect("parsed layout -> descriptor text"),
        text,
        "descriptor text is not deterministic for {name}"
    );

    match desc.data_type {
        DataType::Int32 => {
            let expected = decode_i32(desc);
            let actual = decode_i32(&parsed);
            assert_eq!(actual, expected, "i32 decode mismatch for {name}");
        }
        DataType::Utf8 => {
            let expected = decode_utf8(desc);
            let actual = decode_utf8(&parsed);
            assert_eq!(actual, expected, "utf8 decode mismatch for {name}");
        }
        ref other => panic!("unsupported descriptor fixture data type {other:?}"),
    }
}

fn decode_i32(desc: &LayoutDescription) -> Vec<Option<i32>> {
    let registry = L2KernelRegistry::default_for_mvp0();
    let data = decode_layout_to_array_data(desc, &registry).expect("decode i32 descriptor");
    let array = Int32Array::from(data);
    (0..array.len())
        .map(|row| {
            if array.is_null(row) {
                None
            } else {
                Some(array.value(row))
            }
        })
        .collect()
}

fn decode_utf8(desc: &LayoutDescription) -> Vec<Option<String>> {
    let registry = L2KernelRegistry::default_for_mvp0();
    let data = decode_layout_to_array_data(desc, &registry).expect("decode utf8 descriptor");
    let array = StringArray::from(data);
    (0..array.len())
        .map(|row| {
            if array.is_null(row) {
                None
            } else {
                Some(array.value(row).to_string())
            }
        })
        .collect()
}

fn bitpack_desc() -> (&'static str, LayoutDescription) {
    let values = [1i32, 2, 3, 4];
    let input = PrimitiveArray::from_iter(values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 3, &mut ctx).expect("bitpack encode");
    (
        "bitpack-i32",
        LayoutDescription {
            data_type: DataType::Int32,
            root: vortex_reader::from_bitpacked_array(&packed),
            row_count: values.len(),
        },
    )
}

fn nullable_bitpack_desc() -> LayoutDescription {
    let values = [Some(1i32), None, Some(7), Some(3), None];
    let input = PrimitiveArray::from_option_iter(values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 3, &mut ctx).expect("bitpack encode");
    LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_bitpacked_array(&packed),
        row_count: values.len(),
    }
}

fn for_desc() -> (&'static str, LayoutDescription) {
    let deltas = [0i32, 1, 2];
    let reference = 10i32;
    let input = PrimitiveArray::from_iter(deltas);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 2, &mut ctx).expect("bitpack encode");
    let for_array = FoR::try_new(packed.into_array(), reference.into()).expect("for encode");
    (
        "for-i32",
        LayoutDescription {
            data_type: DataType::Int32,
            root: vortex_reader::from_for_array(&for_array),
            row_count: deltas.len(),
        },
    )
}

fn dict_desc() -> (&'static str, LayoutDescription) {
    let values = PrimitiveArray::from_iter([10i32, 20, 30]);
    let codes = PrimitiveArray::from_iter([2i32, 0, 1, 2]);
    let dict = DictArray::try_new(codes.into_array(), values.into_array()).expect("dict");
    (
        "dict-i32",
        LayoutDescription {
            data_type: DataType::Int32,
            root: vortex_reader::from_dict_array(&dict),
            row_count: 4,
        },
    )
}

fn rle_desc() -> (&'static str, LayoutDescription) {
    let input = PrimitiveArray::from_iter([1u32, 1, 2, 2, 3]);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let rle = RLEData::encode(input.as_view(), &mut ctx).expect("rle encode");
    (
        "rle-bool",
        LayoutDescription {
            data_type: DataType::Int32,
            root: vortex_reader::from_rle_array(&rle),
            row_count: 5,
        },
    )
}

fn fsst_desc() -> (&'static str, LayoutDescription) {
    let rows = [Some("alpha"), None, Some("beta")];
    (
        "fsst-utf8",
        LayoutDescription {
            data_type: DataType::Utf8,
            root: vortex_reader::from_fsst_array(&make_fsst(&rows)),
            row_count: rows.len(),
        },
    )
}

fn fsst_edge_desc() -> LayoutDescription {
    let rows = [Some(""), Some("abcdefgh"), Some("escape-heavy-zzzz")];
    LayoutDescription {
        data_type: DataType::Utf8,
        root: vortex_reader::from_fsst_array(&make_fsst(&rows)),
        row_count: rows.len(),
    }
}

fn dict_fsst_desc() -> (&'static str, LayoutDescription) {
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
    (
        "dict-fsst-utf8",
        LayoutDescription {
            data_type: DataType::Utf8,
            root: vortex_reader::from_dict_array(&dict),
            row_count: 4,
        },
    )
}

fn make_fsst(rows: &[Option<&str>]) -> vortex_fsst::FSSTArray {
    let values = VarBinArray::from_iter(rows.iter().copied(), DType::Utf8(Nullability::Nullable));
    let compressor = vortex_fsst::fsst_train_compressor(&values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    vortex_fsst::fsst_compress(&values, values.len(), values.dtype(), &compressor, &mut ctx)
}
