//! Parquet-specific sidecar extract and embed — Phase 50.
//!
//! This module provides host-adapter functions for extracting and embedding
//! [`loom_ir_core::sidecar::SidecarOverlay`] payloads in Parquet's
//! `KeyValue` metadata mechanism. The sidecar is strippable: unknown `loom.*`
//! keys are silently ignored by standard Parquet readers.
//!
//! # Embedding strategy
//!
//! - File-level: one `KeyValue` with key `"loom.sidecar.v1"` carrying the
//!   base64-encoded sidecar overlay bytes.
//! - Per-column: optional `KeyValue` entries with keys `"loom.hash.<granule_id>"`
//!   for column-level redundancy.
//!
//! # Why base64?
//!
//! The Parquet `KeyValue.value` field is `Option<String>`. Binary sidecar bytes
//! are base64-encoded (standard alphabet, no padding) to guarantee a lossless
//! roundtrip through the string field.

use base64::Engine as _;
use loom_ir_core::sidecar::{SidecarCodecError, SidecarOverlay};
use parquet::file::metadata::{KeyValue, ParquetMetaData};

/// Key used for the file-level sidecar payload in Parquet's KeyValue metadata.
const SIDECAR_KEY: &str = "loom.sidecar.v1";

/// Prefix for per-column content-hash KeyValue keys.
const HASH_KEY_PREFIX: &str = "loom.hash.";

/// Extract a sidecar overlay from Parquet file metadata.
///
/// Scans `FileMetaData.key_value_metadata()` for the `"loom.sidecar.v1"` key.
/// If found, base64-decodes the value and calls [`SidecarOverlay::decode`].
/// Returns `Ok(Some(overlay))` on success, `Ok(None)` if no sidecar key exists.
pub fn extract_sidecar_from_parquet_metadata(
    metadata: &ParquetMetaData,
) -> Result<Option<SidecarOverlay>, SidecarCodecError> {
    let kv_list = match metadata.file_metadata().key_value_metadata() {
        Some(list) => list,
        None => return Ok(None),
    };

    for kv in kv_list {
        if kv.key == SIDECAR_KEY {
            let encoded = match &kv.value {
                Some(v) => v.as_bytes(),
                None => continue,
            };
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| {
                    SidecarCodecError::Malformed(format!(
                        "base64 decode of sidecar KeyValue failed: {e}"
                    ))
                })?;
            return SidecarOverlay::decode(&decoded).map(Some);
        }
    }

    Ok(None)
}

/// Embed a sidecar overlay into a Parquet KeyValue metadata list.
///
/// Encodes the overlay, base64-encodes the binary bytes, and pushes a
/// `"loom.sidecar.v1"` entry. Also pushes per-column `"loom.hash.*"` entries
/// for each [`ChunkBinding`] in the overlay. Any existing `"loom.sidecar.v1"`
/// entry is removed first (idempotent re-embed). Existing `"loom.hash.*"`
/// entries are also cleaned up.
///
/// This function only modifies the provided `kv_metadata` list. It does not
/// mutate host data pages or chunks — sidecar embedding is additive metadata.
pub fn embed_sidecar_into_key_value_metadata(
    kv_metadata: &mut Vec<KeyValue>,
    overlay: &SidecarOverlay,
) {
    // Remove any existing loom sidecar entries (idempotent re-embed).
    kv_metadata.retain(|kv| {
        kv.key != SIDECAR_KEY && !kv.key.starts_with(HASH_KEY_PREFIX)
    });

    // Encode and base64-wrap the overlay.
    let encoded = overlay.encode();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&encoded);

    kv_metadata.push(KeyValue::new(
        SIDECAR_KEY.to_string(),
        Some(b64),
    ));

    // Per-column hash entries.
    for binding in &overlay.bindings {
        kv_metadata.push(KeyValue::new(
            format!("{HASH_KEY_PREFIX}{}", binding.granule_id),
            Some(binding.content_hash.clone()),
        ));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use loom_ir_core::sidecar::{ChunkBinding, SidecarOverlay};

    fn make_overlay() -> SidecarOverlay {
        SidecarOverlay {
            ir_bytes: vec![0x01, 0x02, 0x03],
            bindings: vec![
                ChunkBinding {
                    granule_id: "col_a".to_string(),
                    host_data_range: (0, 512),
                    content_hash: "l2ir:1111111111111111".to_string(),
                    ir_identity: "l2ir:aaaaaaaaaaaaaaaa".to_string(),
                },
                ChunkBinding {
                    granule_id: "col_b".to_string(),
                    host_data_range: (512, 1024),
                    content_hash: "l2ir:2222222222222222".to_string(),
                    ir_identity: "l2ir:bbbbbbbbbbbbbbbb".to_string(),
                },
            ],
        }
    }

    /// Helper: create a minimal ParquetMetaData with the given key_value_metadata.
    fn metadata_with_kv(
        kv_metadata: Option<Vec<KeyValue>>,
    ) -> ParquetMetaData {
        use parquet::file::metadata::FileMetaData;
        use parquet::schema::types::{
            ColumnDescriptor, ColumnPath, SchemaDescriptor,
            SchemaElement, Type, PrimitiveType, PhysicalType,
        };
        use std::sync::Arc;

        let schema_elements = vec![SchemaElement {
            type_: Some(Arc::new(Type::PrimitiveType(PrimitiveType {
                physical_type: PhysicalType::INT32,
                type_length: -1,
                scale: -1,
                precision: -1,
            }))),
            ..Default::default()
        }];
        let schema_descr = Arc::new(
            SchemaDescriptor::new(Arc::new(
                parquet::schema::types::SchemaType::new(vec![Arc::new(
                    Type::PrimitiveType(PrimitiveType {
                        physical_type: PhysicalType::INT32,
                        type_length: -1,
                        scale: -1,
                        precision: -1,
                    }),
                )]),
            )),
        );
        let file_meta = FileMetaData::new(
            1,       // version
            0,       // num_rows
            None,    // created_by
            kv_metadata,
            schema_descr,
            None,    // column_orders
        );
        ParquetMetaData::new(file_meta, vec![])
    }

    #[test]
    fn extract_returns_none_when_no_sidecar_key() {
        let metadata = metadata_with_kv(Some(vec![
            KeyValue::new("other.key".to_string(), Some("value".to_string())),
        ]));
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn extract_returns_none_when_empty_kv() {
        let metadata = metadata_with_kv(Some(vec![]));
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn extract_returns_none_when_no_kv_metadata() {
        let metadata = metadata_with_kv(None);
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn embed_extract_roundtrip() {
        let overlay = make_overlay();
        let mut kv_metadata = Vec::new();
        embed_sidecar_into_key_value_metadata(&mut kv_metadata, &overlay);

        // Build metadata with the modified KV list.
        let metadata = metadata_with_kv(Some(kv_metadata));
        let extracted = extract_sidecar_from_parquet_metadata(&metadata)
            .expect("extract must succeed")
            .expect("sidecar must be present");

        assert_eq!(overlay, extracted);
    }

    #[test]
    fn embed_preserves_non_loom_keys() {
        let mut kv_metadata = vec![
            KeyValue::new("pandas".to_string(), Some("{}".to_string())),
            KeyValue::new("writer".to_string(), Some("my-writer".to_string())),
        ];
        let overlay = make_overlay();
        embed_sidecar_into_key_value_metadata(&mut kv_metadata, &overlay);

        let metadata = metadata_with_kv(Some(kv_metadata));
        let kv_list = metadata.file_metadata().key_value_metadata().unwrap();

        // Non-loom keys should be preserved.
        assert!(kv_list.iter().any(|kv| kv.key == "pandas"));
        assert!(kv_list.iter().any(|kv| kv.key == "writer"));

        // Loom keys should be present.
        assert!(kv_list.iter().any(|kv| kv.key == SIDECAR_KEY));
        assert!(kv_list
            .iter()
            .any(|kv| kv.key == format!("{HASH_KEY_PREFIX}col_a")));
        assert!(kv_list
            .iter()
            .any(|kv| kv.key == format!("{HASH_KEY_PREFIX}col_b")));
    }

    #[test]
    fn idempotent_re_embed() {
        let overlay = make_overlay();
        let mut kv_metadata = Vec::new();

        // First embed
        embed_sidecar_into_key_value_metadata(&mut kv_metadata, &overlay);
        let count_after_first = kv_metadata.len();

        // Second embed (should be idempotent — remove old, add new)
        embed_sidecar_into_key_value_metadata(&mut kv_metadata, &overlay);
        assert_eq!(kv_metadata.len(), count_after_first);

        let metadata = metadata_with_kv(Some(kv_metadata));
        let extracted = extract_sidecar_from_parquet_metadata(&metadata)
            .expect("extract must succeed")
            .expect("sidecar must be present");
        assert_eq!(overlay, extracted);
    }

    #[test]
    fn extract_rejects_bad_base64() {
        let kv_metadata = vec![KeyValue::new(
            SIDECAR_KEY.to_string(),
            Some("not-valid-base64!!!".to_string()),
        )];
        let metadata = metadata_with_kv(Some(kv_metadata));
        let result = extract_sidecar_from_parquet_metadata(&metadata);
        assert!(result.is_err());
    }

    #[test]
    fn extract_rejects_corrupt_sidecar_bytes() {
        // Valid base64, but the decoded bytes aren't a valid sidecar.
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"garbage");
        let kv_metadata = vec![KeyValue::new(
            SIDECAR_KEY.to_string(),
            Some(b64),
        )];
        let metadata = metadata_with_kv(Some(kv_metadata));
        let result = extract_sidecar_from_parquet_metadata(&metadata);
        // Should fail with Truncated or Malformed
        assert!(result.is_err());
    }

    #[test]
    fn extract_skips_empty_value() {
        // KeyValue with no value should be skipped.
        let kv_metadata = vec![KeyValue::new(
            SIDECAR_KEY.to_string(),
            None::<String>,
        )];
        let metadata = metadata_with_kv(Some(kv_metadata));
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }
}
