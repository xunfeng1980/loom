//! Host-neutral sidecar overlay data model — Phase 50.
//!
//! This module defines the central sidecar contract that all three host adapters
//! (Parquet, Vortex, Lance) implement. One [`SidecarOverlay`] type, one encoding
//! format, consumed identically by every adapter. The sidecar is strippable:
//! unknown `loom.*` KeyValue entries are silently ignored by host file readers.
//!
//! # Encoding format
//!
//! ```text
//! [0..4]   ir_bytes_len  = u32 LE
//! [4..]    ir_bytes      = raw L2Core IR bytes (L2IR magic + version + payload)
//! [N..N+1] bindings_len  = u16 LE
//! For each ChunkBinding:
//!   [1]     granule_id_len  = u8
//!   [..]    granule_id      = UTF-8 bytes
//!   [8]     offset          = u64 LE
//!   [8]     length          = u64 LE
//!   [1]     content_hash_len = u8
//!   [..]    content_hash    = UTF-8 bytes ("blake3:<hex>")
//!   [1]     ir_identity_len = u8
//!   [..]    ir_identity     = UTF-8 bytes ("blake3:<hex>")
//! ```
//!
//! All multi-byte integers are little-endian. Field order is fixed by the struct
//! definition. The encoding is deterministic: the same [`SidecarOverlay`] always
//! produces identical bytes.

use std::fmt;

use crate::l2core_codec;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A host-neutral sidecar overlay that rides on a host file (Parquet, Vortex,
/// Lance). Contains the encoded L2Core IR bytes plus per-granule content-hash
/// bindings that tie host data ranges to the IR identity.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct SidecarOverlay {
    /// Encoded L2Core IR bytes (including L2IR magic + version header, as
    /// produced by `l2core_codec::encode_l2core_program`).
    pub ir_bytes: Vec<u8>,
    /// Per-granule content-hash bindings tying host data ranges to the IR.
    pub bindings: Vec<ChunkBinding>,
}

/// A content-hash binding for one host data granule (column chunk, fragment,
/// row group).
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct ChunkBinding {
    /// Identifier for this granule (column name, column index, fragment id).
    pub granule_id: String,
    /// Byte range `(offset, length)` of the host data this binding covers.
    pub host_data_range: (u64, u64),
    /// FNV-1a hash of the host data bytes, formatted as `"blake3:<hex>"`.
    pub content_hash: String,
    /// The L2Core IR program hash (must match `l2core_program_hash` of the
    /// decoded `ir_bytes`).
    pub ir_identity: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Failure modes for the sidecar overlay codec.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum SidecarCodecError {
    /// Malformed or unexpected format during decode.
    Malformed(String),
    /// Bytes end before the expected length.
    Truncated,
    /// A hash field does not match the expected `blake3:<hex>` format.
    BadHashFormat(String),
}

impl fmt::Display for SidecarCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SidecarCodecError::Malformed(msg) => write!(f, "malformed sidecar: {msg}"),
            SidecarCodecError::Truncated => write!(f, "truncated sidecar data"),
            SidecarCodecError::BadHashFormat(field) => {
                write!(f, "bad hash format in sidecar field `{field}`")
            }
        }
    }
}

impl std::error::Error for SidecarCodecError {}

// ---------------------------------------------------------------------------
// Hash verification types
// ---------------------------------------------------------------------------

/// Result of verifying one granule's content-hash binding against host data.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct HashVerificationResult {
    /// Identifier for this granule (column name, fragment id, etc.).
    pub granule_id: String,
    /// The original binding from the sidecar.
    pub binding: ChunkBinding,
    /// The hash recomputed from actual host data bytes.
    pub recomputed_hash: String,
    /// Whether `recomputed_hash` equals `binding.content_hash`.
    pub matches: bool,
}

// ---------------------------------------------------------------------------
// Content-hash computation
// ---------------------------------------------------------------------------

/// Compute the BLAKE3-256 content-hash of raw host data bytes.
///
/// Uses BLAKE3 for tamper-resistant content-hash identity. The hash
/// is formatted as `"blake3:<hex>"` where hex is the lowercase 256-bit
/// (32-byte) hash zero-padded to 64 hexadecimal characters.
pub fn compute_chunk_hash(data: &[u8]) -> String {
    let hash = blake3::hash(data);
    format!("blake3:{hash}")
}

/// Verify a single [`ChunkBinding`] against actual host data bytes.
///
/// Recomputes the FNV-1a hash of `host_data` and compares it with
/// `binding.content_hash`. Returns a [`HashVerificationResult`] with
/// the granule id, binding, recomputed hash, and match status.
pub fn verify_chunk_binding(
    binding: &ChunkBinding,
    host_data: &[u8],
) -> HashVerificationResult {
    let recomputed_hash = compute_chunk_hash(host_data);
    let matches = recomputed_hash == binding.content_hash;
    HashVerificationResult {
        granule_id: binding.granule_id.clone(),
        binding: binding.clone(),
        recomputed_hash,
        matches,
    }
}

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------

impl SidecarOverlay {
    /// Encode this sidecar overlay into its deterministic binary representation.
    ///
    /// # Pre-conditions
    ///
    /// * `ir_bytes.len()` must not exceed `u32::MAX` (4 GiB) — panic otherwise.
    /// * Every [`ChunkBinding`] `granule_id`, `content_hash`, and `ir_identity`
    ///   string must be ≤ 255 bytes in UTF-8 — panic otherwise.
    ///
    /// These limits are structural to the binary encoding (u32 length prefix
    /// for IR bytes, u8 length prefix for string fields). To avoid a panic,
    /// either validate these pre-conditions before calling `encode`, or use the
    /// `SidecarOverlay::decode` return-`Result` style instead (future API).
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // ir_bytes: u32 length prefix + raw bytes
        // Fail-closed: IR bytes larger than u32::MAX cannot be encoded
        // in this format; silent truncation would produce a corrupt prefix.
        let ir_len = self.ir_bytes.len();
        let ir_len_u32 = u32::try_from(ir_len).unwrap_or_else(|_| {
            panic!(
                "sidecar IR bytes exceed u32::MAX ({} bytes); encoding not supported",
                ir_len
            )
        });
        l2core_codec::write_u32(&mut buf, ir_len_u32);
        buf.extend_from_slice(&self.ir_bytes);

        // bindings: u16 count
        l2core_codec::write_u16(&mut buf, self.bindings.len() as u16);

        for binding in &self.bindings {
            // granule_id: u8 length prefix + UTF-8 bytes (max 255)
            write_u8_len_str(&mut buf, &binding.granule_id);
            // host_data_range: (offset, length) as two u64 LE
            l2core_codec::write_u64(&mut buf, binding.host_data_range.0);
            l2core_codec::write_u64(&mut buf, binding.host_data_range.1);
            // content_hash: u8 length prefix + UTF-8 bytes
            write_u8_len_str(&mut buf, &binding.content_hash);
            // ir_identity: u8 length prefix + UTF-8 bytes
            write_u8_len_str(&mut buf, &binding.ir_identity);
        }

        buf
    }
}

// ---------------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------------

impl SidecarOverlay {
    /// Decode a sidecar overlay from its binary representation.
    ///
    /// Returns a typed error on any malformed, truncated, or invalid input.
    /// This function never panics on untrusted input — all read paths are
    /// bounds-checked.
    pub fn decode(bytes: &[u8]) -> Result<Self, SidecarCodecError> {
        let mut pos: usize = 0;

        // ir_bytes: u32 length prefix
        let ir_bytes_len = read_u32(bytes, &mut pos)? as usize;
        if pos + ir_bytes_len > bytes.len() {
            return Err(SidecarCodecError::Truncated);
        }
        let ir_bytes = bytes[pos..pos + ir_bytes_len].to_vec();
        pos += ir_bytes_len;

        // bindings: u16 count
        let bindings_count = read_u16(bytes, &mut pos)? as usize;

        let mut bindings = Vec::with_capacity(bindings_count);
        for _ in 0..bindings_count {
            // granule_id: u8 len + UTF-8 bytes
            let granule_id = read_u8_len_str(bytes, &mut pos)?;

            // host_data_range: (offset, length)
            let offset = read_u64(bytes, &mut pos)?;
            let length = read_u64(bytes, &mut pos)?;

            // content_hash: u8 len + UTF-8 bytes
            let content_hash = read_u8_len_str(bytes, &mut pos)?;
            validate_hash_format(&content_hash, "content_hash")?;

            // ir_identity: u8 len + UTF-8 bytes
            let ir_identity = read_u8_len_str(bytes, &mut pos)?;
            validate_hash_format(&ir_identity, "ir_identity")?;

            bindings.push(ChunkBinding {
                granule_id,
                host_data_range: (offset, length),
                content_hash,
                ir_identity,
            });
        }

        // If there are trailing bytes after the last binding, that's malformed
        if pos != bytes.len() {
            return Err(SidecarCodecError::Malformed(format!(
                "trailing bytes after last binding: expected {} bytes, got {}",
                pos,
                bytes.len()
            )));
        }

        Ok(SidecarOverlay {
            ir_bytes,
            bindings,
        })
    }
}

// ---------------------------------------------------------------------------
// Low-level read helpers (bounds-checked at every step)
// ---------------------------------------------------------------------------

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, SidecarCodecError> {
    if *pos + 4 > bytes.len() {
        return Err(SidecarCodecError::Truncated);
    }
    let value = u32::from_le_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
    ]);
    *pos += 4;
    Ok(value)
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> Result<u16, SidecarCodecError> {
    if *pos + 2 > bytes.len() {
        return Err(SidecarCodecError::Truncated);
    }
    let value = u16::from_le_bytes([bytes[*pos], bytes[*pos + 1]]);
    *pos += 2;
    Ok(value)
}

fn read_u64(bytes: &[u8], pos: &mut usize) -> Result<u64, SidecarCodecError> {
    if *pos + 8 > bytes.len() {
        return Err(SidecarCodecError::Truncated);
    }
    let value = u64::from_le_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
        bytes[*pos + 4],
        bytes[*pos + 5],
        bytes[*pos + 6],
        bytes[*pos + 7],
    ]);
    *pos += 8;
    Ok(value)
}

fn read_u8_len_str(bytes: &[u8], pos: &mut usize) -> Result<String, SidecarCodecError> {
    if *pos >= bytes.len() {
        return Err(SidecarCodecError::Truncated);
    }
    let len = bytes[*pos] as usize;
    *pos += 1;
    if *pos + len > bytes.len() {
        return Err(SidecarCodecError::Truncated);
    }
    let s = std::str::from_utf8(&bytes[*pos..*pos + len])
        .map_err(|_| SidecarCodecError::Malformed("invalid UTF-8 in string field".to_string()))?;
    *pos += len;
    Ok(s.to_string())
}

// ---------------------------------------------------------------------------
// Write helpers (complementing l2core_codec pub(crate) exports)
// ---------------------------------------------------------------------------

fn write_u8_len_str(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    assert!(
        bytes.len() <= u8::MAX as usize,
        "sidecar string field exceeds 255 bytes"
    );
    buf.push(bytes.len() as u8);
    buf.extend_from_slice(bytes);
}

// ---------------------------------------------------------------------------
// Hash format validation
// ---------------------------------------------------------------------------

/// Validate that a hash string follows the `blake3:<hex>` format.
fn validate_hash_format(hash: &str, field_name: &str) -> Result<(), SidecarCodecError> {
    let rest = hash
        .strip_prefix("blake3:")
        .ok_or_else(|| SidecarCodecError::BadHashFormat(field_name.to_string()))?;
    if rest.len() != 64 || !rest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(SidecarCodecError::BadHashFormat(field_name.to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_overlay(ir_bytes: Vec<u8>, bindings: Vec<ChunkBinding>) -> SidecarOverlay {
        SidecarOverlay {
            ir_bytes,
            bindings,
        }
    }

    fn make_binding() -> ChunkBinding {
        ChunkBinding {
            granule_id: "col_int32".to_string(),
            host_data_range: (0, 1024),
            content_hash: "blake3:0000000000000000000000000000000000000000000000000000000000000001".to_string(),
            ir_identity: "blake3:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        }
    }

    // —— roundtrip tests ——

    #[test]
    fn test_roundtrip_empty_bindings() {
        let original = make_overlay(vec![0xAA, 0xBB, 0xCC], vec![]);
        let encoded = original.encode();
        let decoded = SidecarOverlay::decode(&encoded).expect("decode must succeed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_roundtrip_with_bindings() {
        let b1 = ChunkBinding {
            granule_id: "col_a".to_string(),
            host_data_range: (0, 512),
            content_hash: "blake3:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            ir_identity: "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        };
        let b2 = ChunkBinding {
            granule_id: "col_b".to_string(),
            host_data_range: (512, 1024),
            content_hash: "blake3:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            ir_identity: "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        };
        let original = make_overlay(vec![0x01, 0x02, 0x03], vec![b1, b2]);
        let encoded = original.encode();
        let decoded = SidecarOverlay::decode(&encoded).expect("decode must succeed");
        assert_eq!(original, decoded);
    }

    // —— determinism test ——

    #[test]
    fn test_deterministic_encode() {
        let overlay = make_overlay(vec![0x42; 16], vec![make_binding()]);
        let buf1 = overlay.encode();
        let buf2 = overlay.encode();
        assert_eq!(buf1, buf2);
    }

    // —— negative tests ——

    #[test]
    fn test_decode_truncated() {
        let overlay = make_overlay(vec![0x00; 8], vec![make_binding()]);
        let full = overlay.encode();
        // Take only the first 4 bytes (ir_bytes_len field)
        let truncated = &full[..4];
        let result = SidecarOverlay::decode(truncated);
        assert!(matches!(result, Err(SidecarCodecError::Truncated)));
    }

    #[test]
    fn test_decode_truncated_mid_binding() {
        let overlay = make_overlay(vec![0x00; 8], vec![make_binding()]);
        let full = overlay.encode();
        // Truncate in the middle of a binding
        let truncated = &full[..full.len() - 8];
        let result = SidecarOverlay::decode(truncated);
        assert!(matches!(
            result,
            Err(SidecarCodecError::Truncated) | Err(SidecarCodecError::Malformed(_))
        ));
    }

    #[test]
    fn test_decode_malformed() {
        // Random bytes that won't parse coherently
        let random = vec![0xFFu8; 64];
        let result = SidecarOverlay::decode(&random);
        // Should either return Truncated (if we run off the end) or Malformed
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_bad_hash_format_content_hash() {
        let mut binding = make_binding();
        binding.content_hash = "not-a-valid-hash".to_string();
        let overlay = make_overlay(vec![0x00; 8], vec![binding]);
        let encoded = overlay.encode();
        let result = SidecarOverlay::decode(&encoded);
        assert!(matches!(result, Err(SidecarCodecError::BadHashFormat(ref f)) if f == "content_hash"));
    }

    #[test]
    fn test_decode_bad_hash_format_ir_identity() {
        let mut binding = make_binding();
        binding.ir_identity = "wrong:0000000000000000".to_string();
        let overlay = make_overlay(vec![0x00; 8], vec![binding]);
        let encoded = overlay.encode();
        let result = SidecarOverlay::decode(&encoded);
        assert!(matches!(result, Err(SidecarCodecError::BadHashFormat(ref f)) if f == "ir_identity"));
    }

    #[test]
    fn test_large_ir_bytes() {
        let overlay = make_overlay(vec![0xAB; 65536], vec![make_binding()]);
        let encoded = overlay.encode();
        let decoded = SidecarOverlay::decode(&encoded).expect("decode must succeed");
        assert_eq!(overlay, decoded);
    }

    #[test]
    fn test_large_ir_bytes_roundtrip_deterministic() {
        let overlay = make_overlay(vec![0xCD; 65536], vec![make_binding()]);
        let buf1 = overlay.encode();
        let buf2 = overlay.encode();
        assert_eq!(buf1, buf2);
    }
}
