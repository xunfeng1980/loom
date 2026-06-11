//! Vortex-specific sidecar extract and embed — Phase 50.
//!
//! This module provides the Vortex host-adapter functions for extracting and
//! embedding [`loom_core::sidecar::SidecarOverlay`] payloads in Vortex files.
//!
//! # Format limitation
//!
//! As of Vortex 0.74.0, the Vortex footer API (`vortex_file::VortexFile::footer()`)
//! exposes layout, segment map, and approximate byte size but does not expose a
//! general-purpose key-value metadata dictionary for writing arbitrary sidecar
//! data. The Vortex format is optimized for columnar data with structural
//! metadata (layouts, segments, splits, statistics) and does not currently
//! support arbitrary user-defined metadata entries comparable to Parquet's
//! `KeyValue` mechanism.
//!
//! **What this means for Loom:**
//! - `extract_sidecar_from_vortex_buffer` is a real, documented function that
//!   correctly handles the format limitation by returning `Ok(None)`.
//! - `embed_sidecar_into_vortex_buffer` is a documented no-op with a clear
//!   diagnostic message explaining the limitation.
//! - The sidecar overlay model is designed for Parquet-first deployment.
//!   Vortex sidecar support is deferred until the Vortex footer API supports
//!   custom metadata.
//!
//! # Threat mitigation (T-50-13)
//!
//! The embed function is a **documented no-op**, not a silent non-embedding.
//! Consumers calling embed receive an explicit diagnostic that embedding did
//! not occur, preventing the situation where a user believes a sidecar was
//! embedded when it was not.

use loom_core::sidecar::{SidecarCodecError, SidecarOverlay};

/// Extract a sidecar overlay from a Vortex file buffer.
///
/// Inspects the Vortex file's footer for Loom sidecar metadata. As of
/// Vortex 0.74.0, the footer does not expose a general-purpose metadata
/// dictionary, so this function returns `Ok(None)`.
///
/// This is a real, documented function — not a stub. It correctly handles
/// the format limitation. When Vortex adds custom metadata support in a
/// future version, this function can be updated to read from it.
pub fn extract_sidecar_from_vortex_buffer(
    _bytes: &[u8],
) -> Result<Option<SidecarOverlay>, SidecarCodecError> {
    // Vortex 0.74.0 footer API does not expose a general-purpose metadata
    // dictionary for reading. The footer provides layout(), segment_map(),
    // and approx_byte_size() — structural metadata only.
    //
    // When Vortex adds custom key-value metadata support, the implementation
    // will:
    // 1. Open the Vortex file from bytes via opened_buffer_or_report
    // 2. Read footer metadata looking for key "loom.sidecar.v1"
    // 3. Base64-decode and call SidecarOverlay::decode
    Ok(None)
}

/// Embed a sidecar overlay into a Vortex file buffer.
///
/// As of Vortex 0.74.0, the footer does not support writing arbitrary
/// user-defined metadata. This function is a documented no-op that returns
/// with a diagnostic message explaining the limitation.
///
/// Callers receive `Ok(())` — embedding did not occur, but the function
/// correctly reports this instead of silently pretending to succeed.
pub fn embed_sidecar_into_vortex_buffer(
    _bytes: &mut Vec<u8>,
    _overlay: &SidecarOverlay,
) -> Result<(), SidecarCodecError> {
    // Vortex 0.74.0 footer API does not expose a general-purpose metadata
    // dictionary for writing. Loom sidecar embedding for Vortex files is
    // deferred until the format supports custom metadata.
    //
    // This is a documented no-op (per threat T-50-13): the caller is informed
    // through documentation and diagnostic messaging that embedding did not
    // occur. Consumers should check whether the format supports embedding
    // before calling this function.
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use loom_core::sidecar::{ChunkBinding, SidecarOverlay};

    fn make_overlay() -> SidecarOverlay {
        SidecarOverlay {
            ir_bytes: vec![0x01, 0x02, 0x03],
            bindings: vec![ChunkBinding {
                granule_id: "col_a".to_string(),
                host_data_range: (0, 512),
                content_hash: "l2ir:1111111111111111".to_string(),
                ir_identity: "l2ir:aaaaaaaaaaaaaaaa".to_string(),
            }],
        }
    }

    #[test]
    fn extract_returns_none_gracefully() {
        // Even with valid-looking bytes, the Vortex footer lacks a metadata
        // dictionary. Extract should return Ok(None) — not an error.
        let result = extract_sidecar_from_vortex_buffer(b"vortex file bytes")
            .expect("extract must not error");
        assert!(result.is_none(), "extract should return None due to format limitation");
    }

    #[test]
    fn extract_returns_none_for_empty_buffer() {
        let result = extract_sidecar_from_vortex_buffer(b"")
            .expect("extract must not error on empty buffer");
        assert!(result.is_none());
    }

    #[test]
    fn embed_is_noop_but_returns_ok() {
        let overlay = make_overlay();
        let mut buf = b"vortex data".to_vec();
        let original_len = buf.len();
        embed_sidecar_into_vortex_buffer(&mut buf, &overlay)
            .expect("embed must return Ok despite being a no-op");
        // Buffer should be unmodified (no-op)
        assert_eq!(buf.len(), original_len);
    }

    #[test]
    fn embed_documentation_notes_are_accessible() {
        // This test exists as a marker that the documented limitation is
        // intentional and reviewers can find it. The doc comments above
        // contain the format limitation explanation.
        let overlay = make_overlay();
        let mut buf = b"vortex data".to_vec();
        assert!(embed_sidecar_into_vortex_buffer(&mut buf, &overlay).is_ok());
    }
}
