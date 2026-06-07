//! FrameOfReference roundtrip test suite (success criteria 2, 3 closure).
//!
//! Tests verify that `loom_core::synthesized_read_loop` decodes FoRArrays
//! (FoR over BitPacking) to Arrow output matching the Vortex oracle row-for-row.
//!
//! Fixture construction pattern (from vortex-fastlanes tests/for_compress.rs):
//!   1. Build a PrimitiveArray of deltas (non-negative, fitting in bit_width bits).
//!   2. BitPackedData::encode the deltas.
//!   3. FoR::try_new(bp.into_array(), reference_scalar) to layer FoR over BitPack.
//!
//! All fixtures are in-memory. No on-disk Vortex fixture is ever opened.

use std::sync::LazyLock;

use arrow::array::Int32Array;
use arrow_schema::DataType;
use loom_core::arrow_builder_output::OutputBuilder;
use loom_core::l1_model::synthesized_read_loop;
use vortex_array::arrays::PrimitiveArray;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
use vortex_fastlanes::BitPackedData;
use vortex_fastlanes::FoR;

use loom_fixtures::oracle;
use loom_fixtures::vortex_reader;

static SESSION: LazyLock<vortex_session::VortexSession> = LazyLock::new(|| {
    let session = vortex_session::VortexSession::empty();
    vortex_fastlanes::initialize(&session);
    session
});

// ---------------------------------------------------------------------------
// for_roundtrip — FoR(reference=1000) over 3-bit BitPack (success criterion 2)
// ---------------------------------------------------------------------------

/// Decode a FoR-over-BitPack array with a positive reference and assert
/// values match the Vortex oracle row-for-row (success criterion 2).
///
/// Construction: deltas [0..100] bitpacked at 7 bits, wrapped with reference=1000.
/// Decoded values should be [1000..1100].
#[test]
fn for_roundtrip() {
    let mut ctx = SESSION.create_execution_ctx();

    // Deltas: 0..100, fitting in 7 bits. Reference = 1000.
    let deltas: Vec<i32> = (0i32..100).collect();
    let reference: i32 = 1000;
    let parray = PrimitiveArray::from_iter(deltas.iter().copied());
    let bp = BitPackedData::encode(&parray.into_array(), 7, &mut ctx)
        .expect("BitPackedData::encode failed for for_roundtrip deltas");
    let for_array = FoR::try_new(bp.into_array(), reference.into())
        .expect("FoR::try_new failed for for_roundtrip");

    // Oracle: Vortex decode.
    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&for_array.as_array().clone());
    assert_eq!(oracle_values.len(), 100);
    assert!(oracle_nulls.iter().all(|&n| !n));

    // loom-core decode.
    let node = vortex_reader::from_for_array(&for_array);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), 100);
    assert_eq!(array_data.null_count(), 0);
    let decoded = Int32Array::from(array_data);
    for i in 0..100 {
        assert_eq!(
            decoded.value(i),
            oracle_values[i],
            "for_roundtrip: mismatch at index {i}: loom={}, oracle={}",
            decoded.value(i),
            oracle_values[i]
        );
    }
}

// ---------------------------------------------------------------------------
// for_negative_reference — FoR(reference=-500) over BitPack (Open Q2)
// ---------------------------------------------------------------------------

/// Decode a FoR-over-BitPack array with a negative reference (Open Question 2).
///
/// Construction: deltas [0..100] (non-negative, 7-bit), wrapped with reference=-500.
/// Decoded values should be [-500..-400].
#[test]
fn for_negative_reference() {
    let mut ctx = SESSION.create_execution_ctx();

    // Deltas: 0..100 (non-negative, fitting in 7 bits). Reference = -500.
    let deltas: Vec<i32> = (0i32..100).collect();
    let reference: i32 = -500;
    let parray = PrimitiveArray::from_iter(deltas.iter().copied());
    let bp = BitPackedData::encode(&parray.into_array(), 7, &mut ctx)
        .expect("BitPackedData::encode failed for for_negative_reference deltas");
    let for_array = FoR::try_new(bp.into_array(), reference.into())
        .expect("FoR::try_new failed for for_negative_reference");

    // Oracle.
    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&for_array.as_array().clone());
    assert_eq!(oracle_values.len(), 100);
    assert!(oracle_nulls.iter().all(|&n| !n));

    // Expected: deltas[i] + reference = i - 500.
    let expected: Vec<i32> = (0i32..100).map(|d| d + reference).collect();

    // loom-core decode.
    let node = vortex_reader::from_for_array(&for_array);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), 100);
    assert_eq!(array_data.null_count(), 0);
    let decoded = Int32Array::from(array_data);
    for i in 0..100 {
        assert_eq!(
            decoded.value(i),
            expected[i],
            "for_negative_reference: mismatch at index {i}: loom={}, expected={}, oracle={}",
            decoded.value(i),
            expected[i],
            oracle_values[i]
        );
        assert_eq!(
            decoded.value(i),
            oracle_values[i],
            "for_negative_reference: oracle mismatch at {i}"
        );
    }
}

// ---------------------------------------------------------------------------
// for_nullable — FoR over BitPack with nulls (validity from inner BitPack — Pitfall 3)
// ---------------------------------------------------------------------------

/// Decode a FoR-over-BitPack array with nulls and assert null positions match
/// the oracle (success criterion 3).
///
/// Validity lives in the inner `BitPackedArray` child (Pitfall 3 / RESEARCH Q3).
/// Construction: nullable deltas [0..64] with nulls at i%4==0, wrapped with reference=200.
#[test]
fn for_nullable() {
    let mut ctx = SESSION.create_execution_ctx();

    // 64 elements, null at positions where i % 4 == 0, deltas in [0..63].
    let count = 64usize;
    let reference: i32 = 200;
    let input_opts: Vec<Option<i32>> = (0i32..count as i32)
        .map(|i| if i % 4 == 0 { None } else { Some(i) })
        .collect();
    let parray = PrimitiveArray::from_option_iter(input_opts.iter().copied());
    let bp = BitPackedData::encode(&parray.into_array(), 6, &mut ctx)
        .expect("BitPackedData::encode failed for for_nullable deltas");
    let for_array = FoR::try_new(bp.into_array(), reference.into())
        .expect("FoR::try_new failed for for_nullable");

    // Oracle.
    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&for_array.as_array().clone());
    assert_eq!(oracle_values.len(), count);

    // loom-core decode.
    let node = vortex_reader::from_for_array(&for_array);
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
            "for_nullable: null mismatch at index {i}: loom={loom_is_null}, oracle={oracle_is_null}"
        );
        if !oracle_is_null {
            assert_eq!(
                decoded.value(i),
                oracle_values[i],
                "for_nullable: value mismatch at index {i}: loom={}, oracle={}",
                decoded.value(i),
                oracle_values[i]
            );
        }
    }
}
