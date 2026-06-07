//! Typed decode errors for the Loom L1/L2 decode pipeline.
//!
//! All decode functions return `Result<(), LoomDecodeError>`. No arm of the
//! synthesized read loop may call `todo!()`, `panic!()`, or `unimplemented!()` —
//! every error path surfaces a typed variant so the existing `catch_unwind`
//! boundary in `loom-ffi` never has to handle a panic for normal malformed input
//! (T-03-03).

use std::fmt;

/// Errors that can be produced by the L1/L2 decode pipeline.
///
/// Each variant carries enough context for a caller to log or display the
/// problem without needing to interpret an opaque integer code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoomDecodeError {
    /// An encoding type that is not yet implemented in this phase.
    ///
    /// The string is the encoding name (e.g. `"Dictionary"`, `"RunEnd"`,
    /// `"KernelEscape"`). Returned by stub arms in the read loop instead of
    /// `todo!()`/`unimplemented!()` so that callers get a typed result rather
    /// than a panic (D-04, T-03-03).
    UnimplementedEncoding(&'static str),

    /// The packed buffer is shorter than required for the given parameters.
    ///
    /// Prevents out-of-bounds reads on a short or malformed packed buffer
    /// (T-03-01).
    BufferTooShort {
        /// Bytes required by the decode parameters.
        needed: usize,
        /// Bytes actually available in the buffer.
        got: usize,
    },

    /// The native type bit-width is not supported by this decoder.
    ///
    /// `unpack_all` only supports `t_bits` ∈ {32, 64}. Any other value returns
    /// this error so the caller knows it received an unsupported encoding
    /// parameter (T-03-02).
    UnsupportedWidth(u8),

    /// The declared `bit_width` exceeds the native type width `t_bits`.
    ///
    /// Bit-packing a value wider than its container type is nonsensical; this
    /// error surfaces the mismatch without panicking.
    BitWidthExceedsType {
        /// The packed bit-width (`LayoutNode::BitPack::bit_width`).
        bit_width: u8,
        /// The native type width in bits (32 or 64).
        t_bits: u8,
    },

    /// A dictionary code is outside the decoded values array.
    InvalidDictionaryCode {
        /// Logical row containing the invalid code.
        index: usize,
        /// Code value at that row.
        code: i64,
        /// Number of decoded dictionary values available.
        values_len: usize,
    },

    /// Run ends must be strictly increasing.
    NonMonotonicRunEnd {
        /// Run index containing the invalid end.
        index: usize,
        /// Previous run end.
        previous: usize,
        /// Current run end.
        current: usize,
    },

    /// A run end exceeds the declared logical output count.
    RunEndOutOfBounds {
        /// Run index containing the invalid end.
        index: usize,
        /// Current run end.
        run_end: usize,
        /// Declared logical output count.
        count: usize,
    },

    /// The final run end does not cover the declared output count.
    RunEndTooShort {
        /// Final decoded run end.
        last_run_end: usize,
        /// Declared logical output count.
        count: usize,
    },

    /// A RunEnd node references a value row that does not exist.
    InsufficientRunValues {
        /// Run index being expanded.
        run_index: usize,
        /// Number of decoded value rows.
        values_len: usize,
    },

    /// The requested operation is not supported for the current Arrow builder type.
    UnsupportedBuilderType {
        /// Operation that was attempted.
        operation: &'static str,
        /// Builder/data type name.
        data_type: &'static str,
    },

    /// No L2 kernel is registered for the requested id.
    UnknownKernel(u32),
}

impl fmt::Display for LoomDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoomDecodeError::UnimplementedEncoding(name) => {
                write!(f, "encoding '{name}' is not implemented in this phase")
            }
            LoomDecodeError::BufferTooShort { needed, got } => {
                write!(f, "packed buffer too short: need {needed} bytes, got {got}")
            }
            LoomDecodeError::UnsupportedWidth(w) => {
                write!(
                    f,
                    "unsupported native type width: {w} bits (expected 32 or 64)"
                )
            }
            LoomDecodeError::BitWidthExceedsType { bit_width, t_bits } => {
                write!(
                    f,
                    "packed bit_width {bit_width} exceeds native type width {t_bits}"
                )
            }
            LoomDecodeError::InvalidDictionaryCode {
                index,
                code,
                values_len,
            } => {
                write!(
                    f,
                    "dictionary code at row {index} is out of range: code {code}, values_len {values_len}"
                )
            }
            LoomDecodeError::NonMonotonicRunEnd {
                index,
                previous,
                current,
            } => {
                write!(
                    f,
                    "run end at index {index} is not strictly increasing: previous {previous}, current {current}"
                )
            }
            LoomDecodeError::RunEndOutOfBounds {
                index,
                run_end,
                count,
            } => {
                write!(
                    f,
                    "run end at index {index} exceeds count: run_end {run_end}, count {count}"
                )
            }
            LoomDecodeError::RunEndTooShort {
                last_run_end,
                count,
            } => {
                write!(
                    f,
                    "run ends stop at {last_run_end}, before required count {count}"
                )
            }
            LoomDecodeError::InsufficientRunValues {
                run_index,
                values_len,
            } => {
                write!(
                    f,
                    "run index {run_index} has no value row; values_len {values_len}"
                )
            }
            LoomDecodeError::UnsupportedBuilderType {
                operation,
                data_type,
            } => {
                write!(
                    f,
                    "operation '{operation}' is unsupported for builder/data type {data_type}"
                )
            }
            LoomDecodeError::UnknownKernel(id) => {
                write!(f, "unknown L2 kernel id {id}")
            }
        }
    }
}

impl std::error::Error for LoomDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unimplemented() {
        let e = LoomDecodeError::UnimplementedEncoding("Dictionary");
        assert!(e.to_string().contains("Dictionary"));
    }

    #[test]
    fn display_buffer_too_short() {
        let e = LoomDecodeError::BufferTooShort {
            needed: 100,
            got: 50,
        };
        assert!(e.to_string().contains("100"));
        assert!(e.to_string().contains("50"));
    }

    #[test]
    fn display_unknown_kernel() {
        let e = LoomDecodeError::UnknownKernel(99);
        assert!(e.to_string().contains("99"));
    }

    #[test]
    fn display_invalid_dictionary_code() {
        let e = LoomDecodeError::InvalidDictionaryCode {
            index: 2,
            code: 5,
            values_len: 3,
        };
        assert!(e.to_string().contains("row 2"));
        assert!(e.to_string().contains("values_len 3"));
    }
}
