//! Wave-0 BLOCKING de-risk checks for Phase 3.
//!
//! These tests must ALL pass before the rest of the wiring is trusted.
//! A failure here indicates a fundamental assumption is wrong.
//!
//! # Check #1 (compile): BufferHandle byte access + as_opt downcast
//!
//! Verified by `cargo build -p loom-fixtures` succeeding: the
//! `packed_bytes` function in `vortex_reader` calls
//! `packed_buf.as_host().as_ref()` and `array.as_opt::<BitPacked>()`.
//! If either method does not exist, the build fails.
//!
//! # Check #2: fl_transpose_index == fastlanes::transpose for all i in 0..1024
//!
//! [`fl_transpose_matches_fastlanes`] — loom-core's reimplementation of the
//! FastLanes transpose formula must match the published `fastlanes` crate
//! byte-for-byte. This is the load-bearing de-risker for the entire BitPack
//! decode path.
//!
//! # Check #3: 11-bit roundtrip (1025-element two-chunk)
//!
//! [`bitpack_11bit_roundtrip`] — build a Vortex BitPackedArray with 11-bit
//! packing, translate to `LayoutNode` via `vortex_reader`, decode via
//! `loom_core::synthesized_read_loop`, and assert values match the oracle
//! row-for-row including both the first and second 1024-element block.
//!
//! # Check #4: nullable roundtrip (null-position bit-for-bit)
//!
//! [`nullable_roundtrip`] — same as check #3 but with a `Validity::Array`
//! containing scattered nulls; asserts `ArrayData::nulls().is_null(i)` matches
//! the Vortex validity bit for every index.

use std::sync::LazyLock;

use arrow::array::Int32Array;
use arrow_schema::DataType;
use fastlanes::transpose;
use loom_core::arrow_builder_output::OutputBuilder;
use loom_core::l1_model::bitpack::fl_transpose_index;
use loom_core::l1_model::synthesized_read_loop;
use vortex_array::arrays::PrimitiveArray;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
use vortex_fastlanes::BitPackedData;

use loom_fixtures::oracle;
use loom_fixtures::vortex_reader;

// ---------------------------------------------------------------------------
// Session (must include fastlanes encodings for BitPackedData::encode)
// ---------------------------------------------------------------------------

static SESSION: LazyLock<vortex_session::VortexSession> = LazyLock::new(|| {
    let session = vortex_session::VortexSession::empty();
    vortex_fastlanes::initialize(&session);
    session
});

// ---------------------------------------------------------------------------
// Check #2: fl_transpose_index == fastlanes::transpose for all i in 0..1024
// ---------------------------------------------------------------------------

/// BLOCKING: loom-core's `fl_transpose_index(i)` must equal
/// `fastlanes::transpose(i)` for every `i` in 0..1024.
///
/// If this fails, the entire BitPack decode path is wrong and no downstream
/// test result can be trusted.
#[test]
fn fl_transpose_matches_fastlanes() {
    for i in 0..1024usize {
        let loom_result = fl_transpose_index(i);
        let fastlanes_result = transpose(i);
        assert_eq!(
            loom_result, fastlanes_result,
            "fl_transpose_index({i}) = {loom_result} but fastlanes::transpose({i}) = {fastlanes_result}"
        );
    }
}

// ---------------------------------------------------------------------------
// Check #3: 11-bit two-chunk roundtrip (1025 elements)
// ---------------------------------------------------------------------------

/// BLOCKING: Decode a non-byte-aligned 11-bit BitPackedArray via loom-core
/// and assert values match the Vortex oracle row-for-row, including both the
/// first full 1024-element block and the single-element second block.
#[test]
fn bitpack_11bit_roundtrip() {
    let mut ctx = SESSION.create_execution_ctx();

    // Build a 1025-element input that spans two 1024-element blocks.
    // Values are i32, fitting in 11 bits (0..2047).
    let values_input: Vec<i32> = (0i32..1025).map(|i| i % 2047).collect();
    let parray = PrimitiveArray::from_iter(values_input.iter().copied());
    let packed = BitPackedData::encode(&parray.into_array(), 11, &mut ctx)
        .expect("BitPackedData::encode failed for 11-bit test");

    // Oracle: Vortex decode.
    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&packed.as_array().clone());
    assert_eq!(oracle_values.len(), 1025, "oracle length mismatch");
    assert!(
        oracle_nulls.iter().all(|&n| !n),
        "oracle has unexpected nulls"
    );

    // loom-core decode.
    let node = vortex_reader::from_bitpacked_array(&packed);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), 1025, "loom-core output length mismatch");
    assert_eq!(
        array_data.null_count(),
        0,
        "unexpected nulls in loom-core output"
    );

    let decoded = Int32Array::from(array_data);
    for i in 0..1025 {
        assert_eq!(
            decoded.value(i),
            oracle_values[i],
            "bitpack_11bit_roundtrip: mismatch at index {i}: loom-core={}, oracle={}",
            decoded.value(i),
            oracle_values[i]
        );
    }
}

// ---------------------------------------------------------------------------
// Check #4: nullable roundtrip (null positions bit-for-bit)
// ---------------------------------------------------------------------------

/// BLOCKING: Decode a BitPackedArray with scattered nulls via loom-core and
/// assert `ArrayData::nulls().is_null(i)` matches the Vortex validity for
/// every index (L1-07).
#[test]
fn nullable_roundtrip() {
    let mut ctx = SESSION.create_execution_ctx();

    // Build a 64-element array with a scattered null pattern.
    // Null at positions where i % 5 == 0.
    let count = 64usize;
    let input_opts: Vec<Option<i32>> = (0i32..count as i32)
        .map(|i| if i % 5 == 0 { None } else { Some(i % 2047) })
        .collect();
    let parray = PrimitiveArray::from_option_iter(input_opts.iter().copied());
    let packed = BitPackedData::encode(&parray.into_array(), 11, &mut ctx)
        .expect("BitPackedData::encode failed for nullable test");

    // Oracle: get null positions from Vortex.
    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&packed.as_array().clone());
    assert_eq!(oracle_values.len(), count, "oracle length mismatch");

    // loom-core decode.
    let node = vortex_reader::from_bitpacked_array(&packed);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("synthesized_read_loop failed");
    let array_data = builder.finish();

    assert_eq!(array_data.len(), count, "loom-core output length mismatch");

    let decoded = Int32Array::from(array_data.clone());
    // Check null positions bit-for-bit.
    for i in 0..count {
        let loom_is_null = array_data.nulls().map_or(false, |nulls| nulls.is_null(i));
        let oracle_is_null = oracle_nulls[i];
        assert_eq!(
            loom_is_null, oracle_is_null,
            "nullable_roundtrip: null mismatch at index {i}: loom={loom_is_null}, oracle={oracle_is_null}"
        );
        // Also check values for non-null positions.
        if !oracle_is_null {
            assert_eq!(
                decoded.value(i),
                oracle_values[i],
                "nullable_roundtrip: value mismatch at index {i}: loom={}, oracle={}",
                decoded.value(i),
                oracle_values[i]
            );
        }
    }
}
