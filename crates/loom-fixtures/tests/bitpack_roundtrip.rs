//! Bit-packing roundtrip test suite (success criteria 1, 3 closure).
//!
//! Tests verify that `loom_ffi::synthesized_read_loop` decodes BitPacked
//! arrays to Arrow output matching the Vortex oracle row-for-row.
//!
//! All fixtures are built programmatically in-memory via `BitPackedData::encode`.
//! No on-disk Vortex fixture is ever opened (success criterion 5 — structural guarantee).

use std::sync::LazyLock;

use arrow::array::Int32Array;
use arrow_schema::DataType;
use loom_ffi::arrow_builder_output::OutputBuilder;
use loom_ffi::l1_model::synthesized_read_loop;
use vortex_array::arrays::PrimitiveArray;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
use vortex_fastlanes::BitPackedData;

use loom_fixtures::oracle;
use loom_fixtures::vortex_reader;

static SESSION: LazyLock<vortex_session::VortexSession> = LazyLock::new(|| {
    let session = vortex_session::VortexSession::empty();
    vortex_fastlanes::initialize(&session);
    session
});

// ---------------------------------------------------------------------------
// bitpack_non_byte_aligned — 3-bit and 17-bit widths (success criterion 1)
// ---------------------------------------------------------------------------

/// Decode a 3-bit bit-packed array (non-byte-aligned) via loom-core and
/// verify values match the Vortex oracle row-for-row.
#[test]
fn bitpack_non_byte_aligned_3bit() {
    let mut ctx = SESSION.create_execution_ctx();

    // 200 values, all fitting in 3 bits (0..7).
    let values_input: Vec<i32> = (0i32..200).map(|i| i % 8).collect();
    let parray = PrimitiveArray::from_iter(values_input.iter().copied());
    let packed = BitPackedData::encode(&parray.into_array(), 3, &mut ctx)
        .expect("BitPackedData::encode failed for 3-bit test");

    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&packed.as_array().clone());
    assert_eq!(oracle_values.len(), 200);
    assert!(oracle_nulls.iter().all(|&n| !n));

    let node = vortex_reader::from_bitpacked_array(&packed);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), 200);
    assert_eq!(array_data.null_count(), 0);
    let decoded = Int32Array::from(array_data);
    for i in 0..200 {
        assert_eq!(
            decoded.value(i),
            oracle_values[i],
            "3-bit: mismatch at index {i}"
        );
    }
}

/// Decode a 17-bit bit-packed array (non-byte-aligned, crosses 32-bit word
/// boundaries) via loom-core and verify values match the Vortex oracle.
#[test]
fn bitpack_non_byte_aligned_17bit() {
    let mut ctx = SESSION.create_execution_ctx();

    // 150 values, all fitting in 17 bits (0..131071).
    let values_input: Vec<i32> = (0i32..150).map(|i| i * 876 % 131072).collect();
    let parray = PrimitiveArray::from_iter(values_input.iter().copied());
    let packed = BitPackedData::encode(&parray.into_array(), 17, &mut ctx)
        .expect("BitPackedData::encode failed for 17-bit test");

    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&packed.as_array().clone());
    assert_eq!(oracle_values.len(), 150);
    assert!(oracle_nulls.iter().all(|&n| !n));

    let node = vortex_reader::from_bitpacked_array(&packed);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), 150);
    assert_eq!(array_data.null_count(), 0);
    let decoded = Int32Array::from(array_data);
    for i in 0..150 {
        assert_eq!(
            decoded.value(i),
            oracle_values[i],
            "17-bit: mismatch at index {i}"
        );
    }
}

// ---------------------------------------------------------------------------
// all_null_bitpack — AllInvalid fast path (success criterion 3, Open Question 3)
// ---------------------------------------------------------------------------

/// Decode a BitPacked array with `Validity::AllInvalid` and assert every
/// output position is null. The decode must not touch `values_buf`
/// (AllInvalid fast path).
#[test]
fn all_null_bitpack() {
    let mut ctx = SESSION.create_execution_ctx();

    // Build an array where every element is None (AllInvalid).
    let count = 32usize;
    let input_opts: Vec<Option<i32>> = vec![None; count];
    let parray = PrimitiveArray::from_option_iter(input_opts.iter().copied());
    let packed = BitPackedData::encode(&parray.into_array(), 5, &mut ctx)
        .expect("BitPackedData::encode failed for all_null test");

    let node = vortex_reader::from_bitpacked_array(&packed);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), count, "all_null: length mismatch");
    assert_eq!(
        array_data.null_count(),
        count,
        "all_null: every position must be null"
    );
}

// ---------------------------------------------------------------------------
// nullable_bitpack — scattered null pattern (success criterion 3)
// ---------------------------------------------------------------------------

/// Decode a BitPacked array with scattered nulls and assert both values AND
/// null positions match the Vortex oracle row-for-row.
#[test]
fn nullable_bitpack() {
    let mut ctx = SESSION.create_execution_ctx();

    // 128 elements with nulls at every position divisible by 7.
    let count = 128usize;
    let input_opts: Vec<Option<i32>> = (0i32..count as i32)
        .map(|i| if i % 7 == 0 { None } else { Some(i % 2047) })
        .collect();
    let parray = PrimitiveArray::from_option_iter(input_opts.iter().copied());
    let packed = BitPackedData::encode(&parray.into_array(), 11, &mut ctx)
        .expect("BitPackedData::encode failed for nullable_bitpack test");

    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&packed.as_array().clone());
    assert_eq!(oracle_values.len(), count);

    let node = vortex_reader::from_bitpacked_array(&packed);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), count);
    let decoded = Int32Array::from(array_data.clone());

    for i in 0..count {
        let loom_is_null = array_data.nulls().map_or(false, |n| n.is_null(i));
        let oracle_is_null = oracle_nulls[i];
        assert_eq!(
            loom_is_null, oracle_is_null,
            "nullable_bitpack: null mismatch at {i}"
        );
        if !oracle_is_null {
            assert_eq!(
                decoded.value(i),
                oracle_values[i],
                "nullable_bitpack: value mismatch at {i}"
            );
        }
    }
}
