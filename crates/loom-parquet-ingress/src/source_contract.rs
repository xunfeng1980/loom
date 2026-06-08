//! Source-neutral facts extracted from local Parquet files.
//!
//! Parquet SDK objects are adapter-private. Public helpers return only
//! `loom-source-ingress` contract data.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef};
use loom_source_ingress::{
    SourceCoverage, SourceDiagnostic, SourceDiagnosticCode, SourceEmissionDisposition,
    SourceEmissionKind, SourceFacts, SourceIdentity, SourceIngressReport, SourceIngressStatus,
    SourceLayoutFact, SourceLoweringDisposition, SourceSchemaFact, SourceSplitFact,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::file::metadata::{ParquetMetaData, RowGroupMetaData};

/// Extract source-neutral facts from a local Parquet file.
pub fn parquet_source_facts_from_path(path: &Path) -> Result<SourceFacts, SourceIngressReport> {
    let file = File::open(path).map_err(|error| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Parquet file could not be opened",
            )
            .with_source_detail(sanitized_detail(error.to_string())),
        )
    })?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|error| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::ReadFailed,
                "$.metadata",
                "local Parquet metadata could not be read",
            )
            .with_source_detail(sanitized_detail(error.to_string())),
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
        logical_kind(field.data_type()),
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
    let all_supported_primitives = field_count > 0
        && schema
            .fields()
            .iter()
            .all(|field| !field.is_nullable() && is_supported_primitive(field.data_type()));
    let mut coverage = SourceCoverage::new(
        if field_count == 1 {
            logical_kind(schema.fields()[0].data_type()).to_string()
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

    if all_supported_primitives {
        coverage.support = SourceIngressStatus::Accepted;
        coverage.emission_kind = if field_count == 1 {
            SourceEmissionKind::Lmp1
        } else {
            SourceEmissionKind::Lmt1
        };
        coverage.emission_disposition = if field_count == 1 {
            SourceEmissionDisposition::CanonicalRaw
        } else {
            SourceEmissionDisposition::CanonicalTable
        };
        coverage.lowering_disposition = SourceLoweringDisposition::ProductionLoweringSupported;
        coverage.notes.push(
            "supported Parquet shape is classified only; artifact emission is deferred".to_string(),
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

fn is_supported_primitive(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Int32 | DataType::Int64 | DataType::Float32 | DataType::Float64
    )
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

fn sanitized_detail(detail: String) -> String {
    detail
        .lines()
        .next()
        .unwrap_or("Parquet adapter error")
        .chars()
        .take(240)
        .collect()
}
