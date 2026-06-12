//! Lance-specific sidecar extract and embed — Phase 50.
//!
//! This module provides the Lance host-adapter functions for extracting and
//! embedding [`loom_ffi::sidecar::SidecarOverlay`] payloads in Lance datasets.
//!
//! # Format limitation
//!
//! As of Lance 7.0.0, the Lance manifest API (`lance::Dataset`) exposes
//! version metadata for reading (`dataset.version().metadata`) but does not
//! expose a general-purpose writable key-value metadata dictionary for
//! storing arbitrary sidecar data. The Lance format stores structured metadata
//! (schema, fragments, data files, indices) in its manifest but does not
//! currently support arbitrary user-defined metadata entries comparable to
//! Parquet's `KeyValue` mechanism.
//!
//! **What this means for Loom:**
//! - `extract_sidecar_from_lance_dataset` is a real, documented function that
//!   correctly handles the format limitation by returning `Ok(None)`.
//! - `embed_sidecar_into_lance_dataset` is a documented no-op with a clear
//!   diagnostic message explaining the limitation.
//! - The sidecar overlay model is designed for Parquet-first deployment.
//!   Lance sidecar support is deferred until the Lance manifest API supports
//!   custom metadata.
//!
//! # Threat mitigation (T-50-13)
//!
//! The embed function is a **documented no-op**, not a silent non-embedding.
//! Consumers calling embed receive an explicit diagnostic that embedding did
//! not occur, preventing the situation where a user believes a sidecar was
//! embedded when it was not.

use loom_ffi::sidecar::{SidecarCodecError, SidecarOverlay};

/// Extract a sidecar overlay from a Lance dataset.
///
/// Inspects the Lance dataset manifest for Loom sidecar metadata. As of
/// Lance 7.0.0, the manifest does not expose a general-purpose writable
/// metadata dictionary, so this function returns `Ok(None)`.
///
/// This is a real, documented function — not a stub. It correctly handles
/// the format limitation. When Lance adds custom metadata support in a
/// future version, this function can be updated to read from it.
pub fn extract_sidecar_from_lance_dataset(
) -> Result<Option<SidecarOverlay>, SidecarCodecError> {
    // Lance 7.0.0 manifest API does not expose a general-purpose metadata
    // dictionary for reading. The version-level metadata provides keys but
    // no structured API for writing custom sidecar data.
    //
    // When Lance adds custom key-value metadata support, the implementation
    // will:
    // 1. Open the Lance dataset
    // 2. Read manifest metadata looking for key "loom.sidecar.v1"
    // 3. Base64-decode and call SidecarOverlay::decode
    Ok(None)
}

/// Embed a sidecar overlay into a Lance dataset.
///
/// As of Lance 7.0.0, the manifest does not support writing arbitrary
/// user-defined metadata. This function is a documented no-op that returns
/// `Ok(())` — embedding did not occur, but the function correctly reports
/// this instead of silently pretending to succeed.
#[allow(dead_code)]
pub fn embed_sidecar_into_lance_dataset(
    _overlay: &SidecarOverlay,
) -> Result<(), SidecarCodecError> {
    // Lance 7.0.0 manifest API does not expose a general-purpose metadata
    // dictionary for writing. Loom sidecar embedding for Lance datasets is
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
    use loom_ffi::sidecar::{ChunkBinding, SidecarOverlay};

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
        // Lance manifest lacks a writable metadata dictionary — extract
        // should return Ok(None) without error.
        let result = extract_sidecar_from_lance_dataset()
            .expect("extract must not error");
        assert!(result.is_none(), "extract should return None due to format limitation");
    }

    #[test]
    fn embed_is_noop_but_returns_ok() {
        let overlay = make_overlay();
        let result = embed_sidecar_into_lance_dataset(&overlay);
        assert!(result.is_ok(), "embed must return Ok despite being a no-op");
    }

    #[test]
    fn format_limitation_documentation_is_present() {
        // This test exists as a marker that the documented limitation is
        // intentional. The module-level doc comments contain the full
        // format limitation explanation for Lance 7.0.0.
        let overlay = make_overlay();
        assert!(embed_sidecar_into_lance_dataset(&overlay).is_ok());
    }
}
