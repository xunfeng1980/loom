//! RLE/RunEnd fixture tests for Phase 4.

use std::sync::LazyLock;

use arrow::array::{Array, BooleanArray, Int32Array};
use arrow_schema::DataType;
use loom_ffi::arrow_builder_output::OutputBuilder;
use loom_ffi::l1_model::{synthesized_read_loop, LayoutNode};
use vortex_array::arrays::PrimitiveArray;
use vortex_array::VortexSessionExecute;
use vortex_buffer::buffer;
use vortex_fastlanes::RLEData;

use loom_fixtures::oracle;
use loom_fixtures::vortex_reader;

static SESSION: LazyLock<vortex_session::VortexSession> = LazyLock::new(|| {
    let session = vortex_session::VortexSession::empty();
    vortex_fastlanes::initialize(&session);
    session
});

#[test]
fn rle_integer_roundtrip() {
    let mut ctx = SESSION.create_execution_ctx();

    let input = PrimitiveArray::from_iter([1u32, 1, 2, 2, 2, 3]);
    let rle = RLEData::encode(input.as_view(), &mut ctx).expect("RLEData::encode failed");
    let (oracle_values, oracle_nulls) = oracle::decode_u32_oracle(&rle.as_array().clone());
    assert!(oracle_nulls.iter().all(|&is_null| !is_null));

    let node = vortex_reader::from_rle_array(&rle);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("loom RLE decode failed");
    let data = builder.finish();

    assert_eq!(data.len(), oracle_values.len());
    assert_eq!(data.null_count(), 0);
    let decoded = Int32Array::from(data);
    for (idx, expected) in oracle_values.iter().enumerate() {
        assert_eq!(
            decoded.value(idx),
            *expected as i32,
            "value mismatch at {idx}"
        );
    }
}

#[test]
fn rle_integer_oracle_fixture_matches_expected() {
    let mut ctx = SESSION.create_execution_ctx();

    let input = PrimitiveArray::from_iter(buffer![1u32, 1, 2, 2, 2, 3]);
    let rle = RLEData::encode(input.as_view(), &mut ctx).expect("RLEData::encode failed");
    let (oracle_values, oracle_nulls) = oracle::decode_u32_oracle(&rle.as_array().clone());

    assert_eq!(oracle_values, vec![1, 1, 2, 2, 2, 3]);
    assert_eq!(oracle_nulls, vec![false; 6]);
}

#[test]
fn run_end_boolean_expansion_matches_expected() {
    let node = LayoutNode::RunEnd {
        run_ends: Box::new(LayoutNode::Raw {
            data: vec![2i64, 5].iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 8,
            count: 2,
        }),
        values: Box::new(LayoutNode::Raw {
            data: vec![1u8, 0],
            elem_size: 1,
            count: 2,
        }),
        count: 5,
    };

    let mut builder = OutputBuilder::new(&DataType::Boolean);
    synthesized_read_loop(&node, &mut builder).expect("loom boolean RunEnd decode failed");
    let data = builder.finish();
    let decoded = BooleanArray::from(data);

    assert_eq!(decoded.len(), 5);
    assert!(decoded.value(0));
    assert!(decoded.value(1));
    assert!(!decoded.value(2));
    assert!(!decoded.value(3));
    assert!(!decoded.value(4));
}

#[test]
fn nullable_rle_preserves_nulls() {
    let nullable_values = LayoutNode::Dictionary {
        codes: Box::new(LayoutNode::BitPack {
            values_buf: vec![0; 128],
            bit_width: 1,
            offset: 0,
            count: 3,
            validity: Some(vec![true, false, true]),
            all_null: false,
        }),
        values: Box::new(LayoutNode::Raw {
            data: vec![7i32].iter().flat_map(|v| v.to_le_bytes()).collect(),
            elem_size: 4,
            count: 1,
        }),
    };
    let node = LayoutNode::RunEnd {
        run_ends: Box::new(LayoutNode::Raw {
            data: vec![2i64, 4, 5]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            elem_size: 8,
            count: 3,
        }),
        values: Box::new(nullable_values),
        count: 5,
    };

    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("loom nullable RunEnd decode failed");
    let data = builder.finish();
    let decoded = Int32Array::from(data);

    assert_eq!(decoded.len(), 5);
    assert_eq!(decoded.null_count(), 2);
    assert_eq!(decoded.value(0), 7);
    assert_eq!(decoded.value(1), 7);
    assert!(decoded.is_null(2));
    assert!(decoded.is_null(3));
    assert_eq!(decoded.value(4), 7);
}
