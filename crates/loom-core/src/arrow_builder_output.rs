//! Arrow typed builder output stage.
//!
//! [`OutputBuilder`] wraps `arrow-rs` typed builders (`BooleanBuilder`,
//! `Int32Builder`, `Int64Builder`) and exposes a narrow append API:
//! [`append_bool`](OutputBuilder::append_bool),
//! [`append_i32`](OutputBuilder::append_i32),
//! [`append_i64`](OutputBuilder::append_i64),
//! [`append_null`](OutputBuilder::append_null).
//!
//! The only way to emit an Arrow array from `loom-core` is through these typed
//! builder calls — **no raw buffer writes** (ARROW-01). This ensures the Arrow
//! null bitmap is always consistent with the appended values (the builder
//! manages `null_count` and bitmap alignment automatically).
//!
//! # Finish chain
//!
//! ```text
//! OutputBuilder::finish(self)
//!   → PrimitiveArray::into_data()
//!   → ArrayData
//!   → arrow::ffi::to_ffi(&array_data)   (in loom-ffi)
//!   → FFI_ArrowArray + FFI_ArrowSchema
//! ```
//!
//! This is the same chain proven in `loom-ffi/src/ffi.rs` (Phase 2). The
//! [`finish`](OutputBuilder::finish) method is intentionally identical to the
//! `into_data()` call in `ffi.rs:138` so the two code paths stay in sync.

use arrow::array::{Array, BooleanBuilder, Int32Builder, Int64Builder, StringBuilder};
use arrow_data::ArrayData;
use arrow_schema::DataType;

// ---------------------------------------------------------------------------
// OutputBuilder
// ---------------------------------------------------------------------------

/// A typed Arrow builder wrapper that accumulates values and nulls.
///
/// # Variant selection
///
/// Construct via [`OutputBuilder::new`], passing the target Arrow
/// [`DataType`]. Supported types: `Boolean`, `Int32`, `Int64`.
///
/// # Thread safety
///
/// `OutputBuilder` is not `Send` (arrow-rs builders are not either). Each
/// decode invocation constructs its own builder.
pub enum OutputBuilder {
    /// Wraps `arrow::array::BooleanBuilder`.
    Boolean(BooleanBuilder),
    /// Wraps `arrow::array::Int32Builder`.
    Int32(Int32Builder),
    /// Wraps `arrow::array::Int64Builder`.
    Int64(Int64Builder),
    /// Wraps `arrow::array::StringBuilder`.
    Utf8(StringBuilder),
}

impl OutputBuilder {
    /// Construct an [`OutputBuilder`] for the given Arrow [`DataType`].
    ///
    /// # Panics
    ///
    /// Panics if `data_type` is not one of the supported types (`Boolean`,
    /// `Int32`, `Int64`). For MVP0 only these scalar types are supported; this panic is
    /// intentional as unsupported types indicate a programming error in the
    /// caller, not malformed input.
    pub fn new(data_type: &DataType) -> Self {
        match data_type {
            DataType::Boolean => OutputBuilder::Boolean(BooleanBuilder::new()),
            DataType::Int32 => OutputBuilder::Int32(Int32Builder::new()),
            DataType::Int64 => OutputBuilder::Int64(Int64Builder::new()),
            DataType::Utf8 => OutputBuilder::Utf8(StringBuilder::new()),
            other => panic!("OutputBuilder: unsupported DataType {other:?}"),
        }
    }

    /// Return the Arrow data type produced by this builder.
    pub fn data_type(&self) -> DataType {
        match self {
            OutputBuilder::Boolean(_) => DataType::Boolean,
            OutputBuilder::Int32(_) => DataType::Int32,
            OutputBuilder::Int64(_) => DataType::Int64,
            OutputBuilder::Utf8(_) => DataType::Utf8,
        }
    }

    /// Append a non-null boolean value.
    ///
    /// # Panics
    ///
    /// Panics if the builder is not `Boolean`.
    pub fn append_bool(&mut self, v: bool) {
        match self {
            OutputBuilder::Boolean(b) => b.append_value(v),
            OutputBuilder::Int32(_) | OutputBuilder::Int64(_) | OutputBuilder::Utf8(_) => {
                panic!("append_bool called on integer builder")
            }
        }
    }

    /// Append a non-null `i32` value.
    ///
    /// # Panics
    ///
    /// Panics if the builder is `Int64` — use [`append_i64`](Self::append_i64)
    /// for `Int64` builders.
    pub fn append_i32(&mut self, v: i32) {
        match self {
            OutputBuilder::Int32(b) => b.append_value(v),
            OutputBuilder::Boolean(_) | OutputBuilder::Int64(_) | OutputBuilder::Utf8(_) => {
                panic!("append_i32 called on non-Int32 builder")
            }
        }
    }

    /// Append a non-null `i64` value.
    ///
    /// # Panics
    ///
    /// Panics if the builder is `Int32` — use [`append_i32`](Self::append_i32)
    /// for `Int32` builders.
    pub fn append_i64(&mut self, v: i64) {
        match self {
            OutputBuilder::Int64(b) => b.append_value(v),
            OutputBuilder::Boolean(_) | OutputBuilder::Int32(_) | OutputBuilder::Utf8(_) => {
                panic!("append_i64 called on non-Int64 builder")
            }
        }
    }

    /// Append a non-null UTF-8 value.
    ///
    /// # Panics
    ///
    /// Panics if the builder is not `Utf8`.
    pub fn append_string(&mut self, v: &str) {
        match self {
            OutputBuilder::Utf8(b) => b.append_value(v),
            OutputBuilder::Boolean(_) | OutputBuilder::Int32(_) | OutputBuilder::Int64(_) => {
                panic!("append_string called on non-Utf8 builder")
            }
        }
    }

    /// Append a null value.
    ///
    /// The Arrow builder records a null entry in the null bitmap and stores
    /// the type's default value in the values buffer. `null_count` is
    /// incremented automatically.
    pub fn append_null(&mut self) {
        match self {
            OutputBuilder::Boolean(b) => b.append_null(),
            OutputBuilder::Int32(b) => b.append_null(),
            OutputBuilder::Int64(b) => b.append_null(),
            OutputBuilder::Utf8(b) => b.append_null(),
        }
    }

    /// Return the native type bit-width for this builder (32 or 64).
    ///
    /// Used by the bit-unpack path to select the correct `t_bits` value for
    /// [`crate::l1_model::bitpack::unpack_all`].
    ///
    /// # Panics
    ///
    /// Panics for `Boolean` builders. BitPack and FrameOfReference are integer
    /// encodings in MVP0; boolean RunEnd expansion must not call this method.
    pub fn t_bits(&self) -> usize {
        match self {
            OutputBuilder::Boolean(_) | OutputBuilder::Utf8(_) => {
                panic!("t_bits called on non-integer builder; only integer builders have t_bits")
            }
            OutputBuilder::Int32(_) => 32,
            OutputBuilder::Int64(_) => 64,
        }
    }

    /// Finalise the builder and return the [`ArrayData`].
    ///
    /// Consumes the builder. The returned [`ArrayData`] can be passed directly
    /// to `arrow::ffi::to_ffi(&array_data)` in `loom-ffi`.
    ///
    /// This matches the chain in `loom-ffi/src/ffi.rs`:
    /// ```text
    /// let array = builder.finish();          // PrimitiveArray<T>
    /// let array_data = array.into_data();    // ArrayData
    /// ```
    pub fn finish(self) -> ArrayData {
        match self {
            OutputBuilder::Boolean(mut b) => b.finish().into_data(),
            OutputBuilder::Int32(mut b) => b.finish().into_data(),
            OutputBuilder::Int64(mut b) => b.finish().into_data(),
            OutputBuilder::Utf8(mut b) => b.finish().into_data(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A freshly created Int32 builder produces an empty ArrayData with zero
    /// length and zero null_count.
    #[test]
    fn int32_builder_empty_finish() {
        let b = OutputBuilder::new(&DataType::Int32);
        let data = b.finish();
        assert_eq!(data.len(), 0);
        assert_eq!(data.null_count(), 0);
    }

    /// A freshly created Int64 builder produces an empty ArrayData.
    #[test]
    fn int64_builder_empty_finish() {
        let b = OutputBuilder::new(&DataType::Int64);
        let data = b.finish();
        assert_eq!(data.len(), 0);
        assert_eq!(data.null_count(), 0);
    }

    /// Boolean builder works end-to-end.
    #[test]
    fn boolean_builder_values_and_null() {
        use arrow::array::BooleanArray;
        let mut b = OutputBuilder::new(&DataType::Boolean);
        b.append_bool(true);
        b.append_null();
        b.append_bool(false);
        let data = b.finish();
        let array = BooleanArray::from(data);
        assert_eq!(array.len(), 3);
        assert_eq!(array.null_count(), 1);
        assert!(array.value(0));
        assert!(array.is_null(1));
        assert!(!array.value(2));
    }

    /// append_i32 then finish produces correct len and null_count.
    #[test]
    fn append_i32_produces_correct_len() {
        let mut b = OutputBuilder::new(&DataType::Int32);
        b.append_i32(1);
        b.append_i32(2);
        b.append_i32(3);
        let data = b.finish();
        assert_eq!(data.len(), 3);
        assert_eq!(data.null_count(), 0);
    }

    /// append_null increments null_count correctly.
    #[test]
    fn append_null_increments_null_count() {
        let mut b = OutputBuilder::new(&DataType::Int32);
        b.append_i32(10);
        b.append_null();
        b.append_i32(30);
        b.append_null();
        let data = b.finish();
        assert_eq!(data.len(), 4);
        assert_eq!(data.null_count(), 2);
    }

    /// The produced ArrayData has the correct values at non-null positions.
    #[test]
    fn finish_produces_correct_values() {
        use arrow::array::Int32Array;
        let mut b = OutputBuilder::new(&DataType::Int32);
        b.append_i32(42);
        b.append_null();
        b.append_i32(-7);
        let data = b.finish();
        let array = Int32Array::from(data);
        assert_eq!(array.len(), 3);
        assert!(!array.is_null(0));
        assert!(array.is_null(1));
        assert!(!array.is_null(2));
        assert_eq!(array.value(0), 42);
        assert_eq!(array.value(2), -7);
    }

    /// Int64 builder works end-to-end.
    #[test]
    fn int64_builder_values_and_null() {
        use arrow::array::Int64Array;
        let mut b = OutputBuilder::new(&DataType::Int64);
        b.append_i64(i64::MAX);
        b.append_null();
        b.append_i64(-1);
        let data = b.finish();
        let array = Int64Array::from(data);
        assert_eq!(array.len(), 3);
        assert_eq!(array.null_count(), 1);
        assert_eq!(array.value(0), i64::MAX);
        assert!(array.is_null(1));
        assert_eq!(array.value(2), -1);
    }

    #[test]
    fn utf8_builder_values_and_null() {
        use arrow::array::StringArray;
        let mut b = OutputBuilder::new(&DataType::Utf8);
        b.append_string("alpha");
        b.append_null();
        b.append_string("beta");
        let data = b.finish();
        let array = StringArray::from(data);
        assert_eq!(array.len(), 3);
        assert_eq!(array.null_count(), 1);
        assert_eq!(array.value(0), "alpha");
        assert!(array.is_null(1));
        assert_eq!(array.value(2), "beta");
    }

    /// t_bits returns 32 for Int32 and 64 for Int64.
    #[test]
    fn t_bits_correct() {
        let b32 = OutputBuilder::new(&DataType::Int32);
        assert_eq!(b32.t_bits(), 32);
        let b64 = OutputBuilder::new(&DataType::Int64);
        assert_eq!(b64.t_bits(), 64);
    }

    #[test]
    fn data_type_reports_builder_type() {
        assert_eq!(
            OutputBuilder::new(&DataType::Boolean).data_type(),
            DataType::Boolean
        );
        assert_eq!(
            OutputBuilder::new(&DataType::Int32).data_type(),
            DataType::Int32
        );
        assert_eq!(
            OutputBuilder::new(&DataType::Int64).data_type(),
            DataType::Int64
        );
    }
}
