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
use std::path::Path;

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

/// Embed a sidecar overlay into an existing Parquet file on disk.
///
/// Reads the file, extracts existing metadata, embeds the sidecar overlay
/// via [`embed_sidecar_into_key_value_metadata`], and rewrites the file
/// with the modified metadata. All existing row data is preserved.
///
/// This convenience function handles file I/O so downstream consumers
/// (e.g., the CLI) don't need to depend directly on `parquet`.
pub fn embed_sidecar_into_parquet_file(
    path: &Path,
    overlay: &SidecarOverlay,
) -> Result<(), SidecarCodecError> {
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;
    use std::fs::File;

    let file = File::open(path).map_err(|e| {
        SidecarCodecError::Malformed(format!("read Parquet file {}: {e}", path.display()))
    })?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
        SidecarCodecError::Malformed(format!("open Parquet file {}: {e}", path.display()))
    })?;

    let schema = builder.schema().clone();
    let metadata = builder.metadata().clone();
    let file_meta = metadata.file_metadata();

    // Build key_value_metadata from existing metadata
    let mut kv_metadata: Vec<KeyValue> = file_meta
        .key_value_metadata()
        .map(|kv_list| kv_list.to_vec())
        .unwrap_or_default();

    embed_sidecar_into_key_value_metadata(&mut kv_metadata, overlay);

    // Read all batches
    let reader_file = File::open(path).map_err(|e| {
        SidecarCodecError::Malformed(format!("re-open Parquet file {}: {e}", path.display()))
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(reader_file)
        .map_err(|e| {
            SidecarCodecError::Malformed(format!("re-open Parquet file {}: {e}", path.display()))
        })?
        .build()
        .map_err(|e| {
            SidecarCodecError::Malformed(format!("build Parquet reader for {}: {e}", path.display()))
        })?;

    let batches: Vec<_> = reader
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            SidecarCodecError::Malformed(format!("read batches from {}: {e}", path.display()))
        })?;

    // Write to a temporary file first, then atomically rename.
    // This prevents data loss: if File::create(path) truncates the
    // original and the writer subsequently fails mid-batch, the
    // original Parquet data would be irrecoverably lost.
    let tmp_path = path.with_extension("tmp.loom-sidecar");
    let out_file = File::create(&tmp_path).map_err(|e| {
        SidecarCodecError::Malformed(format!(
            "create temp output file {}: {e}",
            tmp_path.display()
        ))
    })?;
    let props = WriterProperties::builder()
        .set_key_value_metadata(Some(kv_metadata))
        .build();
    let mut writer = ArrowWriter::try_new(out_file, schema.clone(), Some(props))
        .map_err(|e| {
            SidecarCodecError::Malformed(format!(
                "create Parquet writer for {}: {e}",
                tmp_path.display()
            ))
        })?;
    for batch in &batches {
        writer.write(batch).map_err(|e| {
            SidecarCodecError::Malformed(format!(
                "write batch to {}: {e}",
                tmp_path.display()
            ))
        })?;
    }
    writer.close().map_err(|e| {
        SidecarCodecError::Malformed(format!(
            "close Parquet writer for {}: {e}",
            tmp_path.display()
        ))
    })?;

    // Atomically replace the original file with the new one.
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        SidecarCodecError::Malformed(format!(
            "rename temp {} to {}: {e}",
            tmp_path.display(),
            path.display()
        ))
    })?;

    Ok(())
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

    /// Write a minimal Parquet file with the given key_value_metadata and
    /// return its metadata. Uses real ArrowWriter → ParquetRecordBatchReaderBuilder.
    fn write_parquet_and_read_metadata(
        kv_metadata: Option<Vec<KeyValue>>,
    ) -> ParquetMetaData {
        use arrow_array::{ArrayRef, Int32Array, RecordBatch};
        use arrow_schema::{DataType, Field, Schema};
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;
        use std::fs::File;
        use std::sync::Arc;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "x",
            DataType::Int32,
            false,
        )]));
        let col: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3]));
        let batch = RecordBatch::try_new(schema.clone(), vec![col]).unwrap();

        let temp = tempfile::NamedTempFile::new().expect("tempfile");
        let file = File::create(temp.path()).expect("create");

        let mut props_builder = WriterProperties::builder();
        if let Some(kv) = kv_metadata {
            props_builder = props_builder.set_key_value_metadata(Some(kv));
        }
        let props = props_builder.build();

        let mut writer = ArrowWriter::try_new(file, schema, Some(props)).expect("writer");
        writer.write(&batch).expect("write");
        writer.close().expect("close");

        let file = File::open(temp.path()).expect("reopen");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("builder");
        Arc::unwrap_or_clone(Arc::clone(builder.metadata()))
    }

    #[test]
    fn extract_returns_none_when_no_sidecar_key() {
        let metadata = write_parquet_and_read_metadata(Some(vec![
            KeyValue::new("other.key".to_string(), Some("value".to_string())),
        ]));
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn extract_returns_none_when_empty_kv() {
        let metadata = write_parquet_and_read_metadata(Some(vec![]));
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn extract_returns_none_when_no_kv_metadata() {
        let metadata = write_parquet_and_read_metadata(None);
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }

    #[test]
    fn embed_extract_roundtrip() {
        let overlay = make_overlay();
        let mut kv_metadata = Vec::new();
        embed_sidecar_into_key_value_metadata(&mut kv_metadata, &overlay);

        let metadata = write_parquet_and_read_metadata(Some(kv_metadata));
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

        let metadata = write_parquet_and_read_metadata(Some(kv_metadata));
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

        let metadata = write_parquet_and_read_metadata(Some(kv_metadata));
        let extracted = extract_sidecar_from_parquet_metadata(&metadata)
            .expect("extract must succeed")
            .expect("sidecar must be present");
        assert_eq!(overlay, extracted);
    }

    #[test]
    fn extract_rejects_bad_base64() {
        let metadata = write_parquet_and_read_metadata(Some(vec![KeyValue::new(
            SIDECAR_KEY.to_string(),
            Some("not-valid-base64!!!".to_string()),
        )]));
        let result = extract_sidecar_from_parquet_metadata(&metadata);
        assert!(result.is_err());
    }

    #[test]
    fn extract_rejects_corrupt_sidecar_bytes() {
        // Valid base64, but the decoded bytes aren't a valid sidecar.
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"garbage");
        let metadata = write_parquet_and_read_metadata(Some(vec![KeyValue::new(
            SIDECAR_KEY.to_string(),
            Some(b64),
        )]));
        let result = extract_sidecar_from_parquet_metadata(&metadata);
        // Should fail with Truncated or Malformed
        assert!(result.is_err());
    }

    #[test]
    fn extract_skips_empty_value() {
        // KeyValue with no value should be skipped.
        let metadata = write_parquet_and_read_metadata(Some(vec![KeyValue::new(
            SIDECAR_KEY.to_string(),
            None::<String>,
        )]));
        let result = extract_sidecar_from_parquet_metadata(&metadata).expect("no error");
        assert!(result.is_none());
    }
}
