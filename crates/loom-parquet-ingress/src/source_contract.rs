//! Source-neutral facts extracted from local Parquet files.
//!
//! Parquet SDK objects are adapter-private. Public helpers return only
//! `loom-source-ingress` contract data.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use loom_source_ingress::{
    SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLayoutFact,
    SourceLoweringDisposition, SourceSchemaFact,
    SourceSplitFact,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::metadata::{ParquetMetaData, RowGroupMetaData};

/// Extract source-neutral facts from a local Parquet file.
pub fn parquet_source_facts_from_path(path: &Path) -> Result<SourceFacts, SourceIngressReport> {
    let file = File::open(path).map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Parquet file could not be opened",
                error.to_string(),
            ),
        )
    })?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.metadata",
                "local Parquet metadata could not be read",
                error.to_string(),
            ),
        )
    })?;

    let schema = Arc::clone(builder.schema());
    if schema.fields().is_empty() {
        return Err(rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::SchemaUnavailable,
                "$.schema",
                "Parquet file did not expose an Arrow schema",
            ),
        ));
    }

    Ok(source_facts_from_metadata(
        path,
        &schema,
        builder.metadata(),
    ))
}

/// Extract sidecar bytes from a Parquet file via its KeyValue metadata.
///
/// Loads the Parquet file's metadata and delegates to
/// [`sidecar_parquet::extract_sidecar_from_parquet_metadata`].
/// On success, re-encodes the overlay bytes. Returns `Ok(None)` when
/// no `"loom.sidecar.v1"` KeyValue entry exists.
pub fn extract_sidecar_bytes_from_parquet_path(
    path: &Path,
) -> Result<Option<Vec<u8>>, SourceIngressReport> {
    let file = File::open(path).map_err(|error| {
        rejected_report(path, diagnostic_with_detail(
            SourceDiagnosticCode::OpenFailed,
            "$.open",
            "local Parquet file could not be opened",
            error.to_string(),
        ))
    })?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|error| {
        rejected_report(path, diagnostic_with_detail(
            SourceDiagnosticCode::ReadFailed,
            "$.metadata",
            "local Parquet metadata could not be read",
            error.to_string(),
        ))
    })?;

    match crate::sidecar_parquet::extract_sidecar_from_parquet_metadata(builder.metadata()) {
        Ok(Some(overlay)) => Ok(Some(overlay.encode())),
        Ok(None) => Ok(None),
        Err(err) => Err(rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.sidecar.decode",
                "failed to decode sidecar overlay from Parquet metadata",
                err.to_string(),
            ),
        )),
    }
}

/// Bind the L2Core IR content-hash to a host data range (Phase 50 placeholder).
pub fn bind_content_hash_to_parquet_data(
    _ir_hash: &str,
    _host_data_range: (u64, u64),
) -> Result<(), SourceIngressReport> {
    Ok(())
}

/// Error reading Parquet physical bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParquetPhysicalError(pub String);

impl std::fmt::Display for ParquetPhysicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parquet physical read: {}", self.0)
    }
}

impl std::error::Error for ParquetPhysicalError {}

/// Read the **raw physical bytes** of one Parquet column chunk by seeking to
/// its `byte_range` and reading directly from the file — no Arrow
/// materialization, no decode. These are the on-disk (encoded, possibly
/// compressed) column-chunk bytes (page headers + data pages).
///
/// Plan 4 building block: binds a sidecar's content hash to real physical
/// bytes rather than to Arrow-materialized values.
pub fn read_column_chunk_physical_bytes(
    path: &Path,
    row_group: usize,
    column: usize,
) -> Result<Vec<u8>, ParquetPhysicalError> {
    use std::io::{Read, Seek, SeekFrom};

    let file = File::open(path).map_err(|e| ParquetPhysicalError(format!("open: {e}")))?;
    let handle_for_meta = file
        .try_clone()
        .map_err(|e| ParquetPhysicalError(format!("clone handle: {e}")))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(handle_for_meta)
        .map_err(|e| ParquetPhysicalError(format!("reader: {e}")))?;
    let metadata = builder.metadata();

    let rg = metadata
        .row_groups()
        .get(row_group)
        .ok_or_else(|| ParquetPhysicalError(format!("row group {row_group} out of range")))?;
    let col = rg
        .columns()
        .get(column)
        .ok_or_else(|| ParquetPhysicalError(format!("column {column} out of range")))?;

    let (start, length) = col.byte_range();
    let mut handle = file;
    handle
        .seek(SeekFrom::Start(start))
        .map_err(|e| ParquetPhysicalError(format!("seek {start}: {e}")))?;
    let mut buf = vec![0u8; length as usize];
    handle
        .read_exact(&mut buf)
        .map_err(|e| ParquetPhysicalError(format!("read {length} bytes: {e}")))?;
    Ok(buf)
}

/// Content-hash identity of a column chunk's raw physical bytes (same hash
/// function the sidecar uses for chunk bindings).
pub fn parquet_column_chunk_hash(
    path: &Path,
    row_group: usize,
    column: usize,
) -> Result<String, ParquetPhysicalError> {
    let bytes = read_column_chunk_physical_bytes(path, row_group, column)?;
    Ok(loom_ir_core::sidecar::compute_chunk_hash(&bytes))
}

/// Read a local Parquet file through the official Arrow scan path.
///
/// This is source evidence only. Accepted Loom artifact bytes come from
/// `LMC2(LMA1)` Arrow semantic emission plus artifact verification.
#[cfg(test)]
pub fn parquet_arrow_oracle_batches_from_path(
    path: &Path,
) -> Result<Vec<RecordBatch>, SourceIngressReport> {
    let file = File::open(path).map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::OpenFailed,
                "$.oracle.open",
                "local Parquet file could not be opened for Arrow oracle scan",
                error.to_string(),
            ),
        )
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .and_then(|builder| builder.build())
        .map_err(|error| {
            rejected_report(
                path,
                diagnostic_with_detail(
                    SourceDiagnosticCode::ReadFailed,
                    "$.oracle.scan",
                    "local Parquet file could not be scanned as Arrow batches",
                    error.to_string(),
                ),
            )
        })?;

    reader.collect::<Result<Vec<_>, _>>().map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.oracle.scan",
                "local Parquet Arrow oracle scan failed",
                error.to_string(),
            ),
        )
    })
}

fn source_facts_from_metadata(
    path: &Path,
    schema: &SchemaRef,
    metadata: &Arc<ParquetMetaData>,
) -> SourceFacts {
    let file_metadata = metadata.file_metadata();
    let row_count = non_negative_i64_to_u64(file_metadata.num_rows());
    let mut facts = SourceFacts::new(
        SourceIdentity::new("parquet", "external-source")
            .with_format_version(file_metadata.version().to_string())
            .with_path_display(path.display().to_string()),
        row_count,
    );

    let root_schema = root_schema_fact(schema.as_ref());
    facts.root_schema = Some(root_schema.clone());
    facts.schema_facts.push(root_schema);
    facts
        .schema_facts
        .extend(schema.fields().iter().map(|field| field_schema_fact(field)));
    facts.layout_facts = layout_facts(schema.as_ref(), metadata);
    facts.split_facts = split_facts(metadata);
    facts.coverage = Some(coverage_from_schema(schema.as_ref(), metadata));
    facts
}

fn root_schema_fact(schema: &Schema) -> SourceSchemaFact {
    let mut fact = SourceSchemaFact::new("$.schema", "struct");
    fact.nullable = Some(schema.fields().iter().any(|field| field.is_nullable()));
    fact.field_count = Some(schema.fields().len());
    fact.field_names = schema
        .fields()
        .iter()
        .map(|field| field.name().to_string())
        .collect();
    fact.arrow_summary = Some(format!("{schema:?}"));
    fact
}

fn field_schema_fact(field: &Field) -> SourceSchemaFact {
    let mut fact = SourceSchemaFact::new(
        format!("$.schema.{}", field.name()),
        logical_kind_for_field(field),
    );
    fact.nullable = Some(field.is_nullable());
    fact.field_count = child_field_count(field.data_type());
    fact.field_names = child_field_names(field.data_type());
    fact.arrow_summary = Some(format!("{field:?}"));
    fact
}

fn layout_facts(schema: &Schema, metadata: &ParquetMetaData) -> Vec<SourceLayoutFact> {
    let mut facts = Vec::with_capacity(metadata.num_row_groups() + 1);
    let mut file_layout = SourceLayoutFact::new("$.metadata", "parquet-file");
    file_layout.row_count = Some(non_negative_i64_to_u64(metadata.file_metadata().num_rows()));
    file_layout.child_count = metadata.num_row_groups();
    file_layout.child_names = (0..metadata.num_row_groups())
        .map(|index| format!("row_group[{index}]"))
        .collect();
    file_layout.physical_refs = vec![
        format!("version={}", metadata.file_metadata().version()),
        format!("schema_fields={}", schema.fields().len()),
        format!("row_groups={}", metadata.num_row_groups()),
        format!("column_index_loaded={}", metadata.column_index().is_some()),
        format!("offset_index_loaded={}", metadata.offset_index().is_some()),
    ];
    if let Some(created_by) = metadata.file_metadata().created_by() {
        file_layout
            .physical_refs
            .push(format!("created_by={created_by}"));
    }
    file_layout.metadata_byte_len = Some(metadata.memory_size());
    facts.push(file_layout);

    for (index, row_group) in metadata.row_groups().iter().enumerate() {
        let mut layout = SourceLayoutFact::new(format!("$.row_groups[{index}]"), "row-group");
        layout.row_count = Some(non_negative_i64_to_u64(row_group.num_rows()));
        layout.child_count = row_group.num_columns();
        layout.child_names = row_group
            .columns()
            .iter()
            .map(|column| column.column_path().string())
            .collect();
        layout.physical_refs = row_group
            .columns()
            .iter()
            .enumerate()
            .map(|(column_index, column)| {
                let (start, length) = column.byte_range();
                format!(
                    "column[{column_index}] path={} physical={:?} compression={:?} statistics={} byte_range={start}..{}",
                    column.column_path().string(),
                    column.column_type(),
                    column.compression(),
                    column.statistics().is_some(),
                    start.saturating_add(length)
                )
            })
            .collect();
        layout.metadata_byte_len = row_group_metadata_byte_len(row_group);
        facts.push(layout);
    }

    facts
}

fn row_group_metadata_byte_len(row_group: &RowGroupMetaData) -> Option<usize> {
    let total = row_group.total_byte_size();
    if total >= 0 {
        Some(total as usize)
    } else {
        None
    }
}

fn split_facts(metadata: &ParquetMetaData) -> Vec<SourceSplitFact> {
    let mut start_row = 0u64;
    metadata
        .row_groups()
        .iter()
        .enumerate()
        .map(|(index, row_group)| {
            let row_count = non_negative_i64_to_u64(row_group.num_rows());
            let end_row = start_row.saturating_add(row_count);
            let split = SourceSplitFact::new(index, start_row, end_row);
            start_row = end_row;
            split
        })
        .collect()
}

fn coverage_from_schema(schema: &Schema, metadata: &ParquetMetaData) -> SourceCoverage {
    let field_count = schema.fields().len();
    let has_nullable = schema.fields().iter().any(|field| field.is_nullable());
    let mut coverage = SourceCoverage::new(
        if field_count == 1 {
            logical_kind_for_field(&schema.fields()[0]).to_string()
        } else {
            "struct".to_string()
        },
        "parquet-row-groups",
        "arrow-record-batch",
    );
    coverage.nullability = Some(has_nullable);
    coverage.has_splits = metadata.num_row_groups() > 0;
    coverage.has_statistics = metadata
        .row_groups()
        .iter()
        .flat_map(|row_group| row_group.columns())
        .any(|column| column.statistics().is_some());

    if field_count > 0 {
        coverage.support = SourceIngressStatus::Accepted;
        coverage.emission_kind = SourceEmissionKind::ArrowSemantic;
        coverage.emission_disposition = SourceEmissionDisposition::SemanticArrow;
        coverage.lowering_disposition = SourceLoweringDisposition::InterpreterOnly;
        coverage.notes.push(
            "Parquet Arrow reader materializes this schema for LMC2-wrapped LMA1 semantic emission"
                .to_string(),
        );
    } else {
        coverage.support = SourceIngressStatus::Unsupported;
        coverage.notes.push(unsupported_note(schema));
    }

    coverage
}

fn unsupported_note(schema: &Schema) -> String {
    if schema.fields().iter().any(|field| field.is_nullable()) {
        "nullable Parquet fields are unsupported for Phase 27 emission".to_string()
    } else if schema
        .fields()
        .iter()
        .any(|field| field_has_extension_metadata(field))
    {
        "extension Parquet fields are unsupported for Phase 27 emission".to_string()
    } else if schema
        .fields()
        .iter()
        .any(|field| matches!(field.data_type(), DataType::Utf8 | DataType::LargeUtf8))
    {
        "string Parquet fields are unsupported for Phase 27 emission".to_string()
    } else if schema
        .fields()
        .iter()
        .any(|field| matches!(field.data_type(), DataType::Struct(_) | DataType::List(_)))
    {
        "nested Parquet fields are unsupported for Phase 27 emission".to_string()
    } else {
        "Parquet schema is outside the non-null Int32/Int64/Float32/Float64 slice".to_string()
    }
}

fn logical_kind(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Int32 | DataType::Int64 | DataType::Float32 | DataType::Float64 => "primitive",
        DataType::Utf8 | DataType::LargeUtf8 => "utf8",
        DataType::Struct(_)
        | DataType::List(_)
        | DataType::LargeList(_)
        | DataType::FixedSizeList(_, _) => "nested",
        DataType::Dictionary(_, _) => "dictionary",
        DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _)
        | DataType::Date32
        | DataType::Date64
        | DataType::Timestamp(_, _) => "logical",
        _ => "unsupported",
    }
}

fn logical_kind_for_field(field: &Field) -> &'static str {
    if field_has_extension_metadata(field) {
        "extension"
    } else {
        logical_kind(field.data_type())
    }
}

fn field_has_extension_metadata(field: &Field) -> bool {
    field
        .metadata()
        .keys()
        .any(|key| key.eq_ignore_ascii_case("ARROW:extension:name"))
}

fn child_field_count(data_type: &DataType) -> Option<usize> {
    match data_type {
        DataType::Struct(fields) => Some(fields.len()),
        _ => None,
    }
}

fn child_field_names(data_type: &DataType) -> Vec<String> {
    match data_type {
        DataType::Struct(fields) => fields
            .iter()
            .map(|field| field.name().to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn non_negative_i64_to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn rejected_report(path: &Path, diagnostic: SourceDiagnostic) -> SourceIngressReport {
    SourceIngressReport::rejected(
        SourceIdentity::new("parquet", "external-source")
            .with_path_display(path.display().to_string()),
        diagnostic,
    )
}

fn diagnostic_with_detail(
    code: SourceDiagnosticCode,
    path: impl Into<String>,
    message: impl Into<String>,
    detail: String,
) -> SourceDiagnostic {
    let sanitized = sanitized_detail(detail);
    if sanitized.is_empty() {
        SourceDiagnostic::new(code, path, message)
    } else {
        SourceDiagnostic::new(code, path, message).with_source_detail(sanitized)
    }
}

fn sanitized_detail(detail: String) -> String {
    let first_line = detail
        .lines()
        .next()
        .unwrap_or("Parquet adapter error")
        .trim();
    let lowered = first_line.to_ascii_lowercase();
    if lowered.contains("credential")
        || lowered.contains("secret")
        || lowered.contains("token")
        || lowered.contains("access_key")
        || lowered.contains("://")
    {
        return String::new();
    }

    first_line.chars().take(240).collect()
}
