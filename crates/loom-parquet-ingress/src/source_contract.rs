//! Source-neutral facts extracted from local Parquet files.
//!
//! Parquet SDK objects are adapter-private. Public helpers return only
//! `loom-source-ingress` contract data.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{Array, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressAcceptedArtifact, SourceIngressReport, SourceIngressStatus, SourceLayoutFact,
    SourceLoweringDisposition, SourceOracleEvidence, SourceOracleStrategy, SourceSchemaFact,
    SourceSplitFact,
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

/// Build a byte-free source ingress report for a local Parquet file.
///
/// Plan 27-02 classifies supported Parquet shapes in `SourceCoverage`, but does
/// not emit artifact bytes or construct accepted reports.
pub fn source_ingress_report_from_parquet_path(path: &Path) -> SourceIngressReport {
    match parquet_source_facts_from_path(path) {
        Ok(facts) => {
            let diagnostic = diagnostic_for_facts(&facts);
            SourceIngressReport::unsupported(Some(facts), diagnostic)
        }
        Err(report) => report,
    }
}

/// Read a local Parquet file through the official Arrow scan path.
///
/// This is source evidence only. Accepted Loom artifact bytes still come from
/// canonical Loom payload emission plus artifact verification.
pub fn parquet_arrow_oracle_batches_from_path(
    path: &Path,
) -> Result<Vec<RecordBatch>, SourceIngressReport> {
    let file = File::open(path).map_err(|error| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.oracle.open",
                "local Parquet file could not be opened for Arrow oracle scan",
            )
            .with_source_detail(sanitized_detail(error.to_string())),
        )
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .and_then(|builder| builder.build())
        .map_err(|error| {
            rejected_report(
                path,
                SourceDiagnostic::new(
                    SourceDiagnosticCode::ReadFailed,
                    "$.oracle.scan",
                    "local Parquet file could not be scanned as Arrow batches",
                )
                .with_source_detail(sanitized_detail(error.to_string())),
            )
        })?;

    reader.collect::<Result<Vec<_>, _>>().map_err(|error| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::ReadFailed,
                "$.oracle.scan",
                "local Parquet Arrow oracle scan failed",
            )
            .with_source_detail(sanitized_detail(error.to_string())),
        )
    })
}

/// Emit verifier-accepted `LMC1` bytes for the supported Parquet slice.
pub fn emit_source_ingress_lmc1_from_parquet_path(
    path: &Path,
) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let facts = parquet_source_facts_from_path(path)?;
    let coverage = facts
        .coverage
        .as_ref()
        .expect("Parquet facts always include coverage");
    if coverage.support != SourceIngressStatus::Accepted {
        let diagnostic = diagnostic_for_facts(&facts);
        return Err(SourceIngressReport::unsupported(Some(facts), diagnostic));
    }

    let batches = parquet_arrow_oracle_batches_from_path(path)
        .map_err(|report| source_oracle_failed_report(&facts, report))?;
    let artifact_bytes = loom_artifact_from_batches(&batches)
        .map_err(|diagnostic| SourceIngressReport::unsupported(Some(facts.clone()), diagnostic))?;

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return Err(source_verification_failed_report(
            &facts,
            verification.status().as_str(),
        ));
    }

    let artifact_facts = verification
        .facts()
        .expect("accepted artifact verification exposes facts");
    let artifact_summary = SourceArtifactVerificationSummary::accepted(
        artifact_bytes.len(),
        format!(
            "{} verifier accepted {}",
            artifact_facts.artifact_kind,
            artifact_facts
                .payload_kind
                .as_deref()
                .unwrap_or("unknown payload")
        ),
    );
    let oracle_evidence = arrow_oracle_evidence(&batches)
        .map_err(|diagnostic| SourceIngressReport::unsupported(Some(facts.clone()), diagnostic))?;
    let coverage = facts
        .coverage
        .as_ref()
        .expect("Parquet facts always include coverage");
    let emission_kind = coverage.emission_kind;
    let emission_disposition = coverage.emission_disposition;
    let lowering_disposition = coverage.lowering_disposition;
    let report = SourceIngressReport::accepted(
        facts,
        emission_kind,
        emission_disposition,
        lowering_disposition,
        artifact_summary,
        oracle_evidence,
    )
    .expect("accepted Parquet facts map to an accepted source report");

    Ok(SourceIngressAcceptedArtifact {
        bytes: artifact_bytes,
        report,
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
    let all_supported_primitives = field_count > 0
        && schema.fields().iter().all(|field| {
            !field.is_nullable()
                && !field_has_extension_metadata(field)
                && is_supported_primitive(field.data_type())
        });
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

fn diagnostic_for_facts(facts: &SourceFacts) -> SourceDiagnostic {
    if facts
        .coverage
        .as_ref()
        .is_some_and(|coverage| coverage.support == SourceIngressStatus::Accepted)
    {
        return SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.emission",
            "Parquet source shape is supported for canonical emission, but artifact bytes are deferred to a later plan",
        );
    }

    if facts
        .schema_facts
        .iter()
        .any(|fact| fact.nullable == Some(true))
    {
        return SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedSchema,
            "$.schema",
            "nullable Parquet fields are outside the supported emission slice",
        );
    }

    if facts.schema_facts.iter().any(|fact| {
        matches!(
            fact.logical_kind.as_str(),
            "nested" | "dictionary" | "extension"
        )
    }) {
        return SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedSchema,
            "$.schema",
            "nested, dictionary, or extension Parquet fields are outside the supported emission slice",
        );
    }

    SourceDiagnostic::new(
        SourceDiagnosticCode::UnsupportedConversion,
        "$.schema",
        "Parquet schema is valid but cannot be converted to a Phase 27 Loom artifact",
    )
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

fn is_supported_primitive(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Int32 | DataType::Int64 | DataType::Float32 | DataType::Float64
    )
}

fn loom_artifact_from_batches(batches: &[RecordBatch]) -> Result<Vec<u8>, SourceDiagnostic> {
    let first = batches.first().ok_or_else(|| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::OracleUnavailable,
            "$.oracle.scan",
            "Parquet Arrow oracle scan produced no batches",
        )
    })?;
    if first.num_columns() == 0 {
        return Err(SourceDiagnostic::new(
            SourceDiagnosticCode::SchemaUnavailable,
            "$.schema",
            "Parquet Arrow oracle scan produced an empty schema",
        ));
    }

    if first.num_columns() == 1 {
        let layout = layout_from_batches(first.schema().field(0), batches, 0)?;
        let payload = encode_layout_payload(&layout);
        return wrap_layout_payload(&payload).map_err(|err| {
            SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedConversion,
                "$.payload",
                format!("failed to wrap Parquet LMP1 payload in LMC1: {err}"),
            )
        });
    }

    let row_count = total_row_count(batches)?;
    let columns = first
        .schema()
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            layout_from_batches(field, batches, index).map(|layout| TableColumn {
                name: field.name().to_string(),
                layout,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let table = TableDescription { row_count, columns };
    let payload = encode_table_payload(&table).map_err(|err| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.payload",
            format!("failed to encode Parquet LMT1 payload: {err}"),
        )
    })?;
    wrap_table_payload(&payload).map_err(|err| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.payload",
            format!("failed to wrap Parquet LMT1 payload in LMC1: {err}"),
        )
    })
}

fn layout_from_batches(
    field: &Field,
    batches: &[RecordBatch],
    column_index: usize,
) -> Result<LayoutDescription, SourceDiagnostic> {
    if field.is_nullable() {
        return Err(SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedSchema,
            format!("$.schema.{}", field.name()),
            "nullable Parquet fields cannot emit Phase 27 Loom artifacts",
        ));
    }
    if field_has_extension_metadata(field) {
        return Err(SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedSchema,
            format!("$.schema.{}", field.name()),
            "extension Parquet fields cannot emit Phase 27 Loom artifacts",
        ));
    }
    let row_count = total_row_count(batches)?;
    let (data, elem_size) = raw_bytes_from_batches(field, batches, column_index)?;
    Ok(LayoutDescription {
        data_type: field.data_type().clone(),
        root: LayoutNode::Raw {
            data,
            elem_size,
            count: row_count,
        },
        row_count,
    })
}

fn raw_bytes_from_batches(
    field: &Field,
    batches: &[RecordBatch],
    column_index: usize,
) -> Result<(Vec<u8>, u8), SourceDiagnostic> {
    let mut out = Vec::new();
    for batch in batches {
        let column = batch.column(column_index);
        if column.null_count() != 0 {
            return Err(SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedSchema,
                format!("$.schema.{}", field.name()),
                "Parquet arrays with null values cannot emit Phase 27 Loom artifacts",
            ));
        }
        match field.data_type() {
            DataType::Int32 => {
                let array = column
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .ok_or_else(|| {
                        unsupported_type_diagnostic(field, "expected Int32 Arrow array")
                    })?;
                out.extend(array.values().iter().flat_map(|value| value.to_le_bytes()));
            }
            DataType::Int64 => {
                let array = column
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .ok_or_else(|| {
                        unsupported_type_diagnostic(field, "expected Int64 Arrow array")
                    })?;
                out.extend(array.values().iter().flat_map(|value| value.to_le_bytes()));
            }
            DataType::Float32 => {
                let array = column
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| {
                        unsupported_type_diagnostic(field, "expected Float32 Arrow array")
                    })?;
                out.extend(array.values().iter().flat_map(|value| value.to_le_bytes()));
            }
            DataType::Float64 => {
                let array = column
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| {
                        unsupported_type_diagnostic(field, "expected Float64 Arrow array")
                    })?;
                out.extend(array.values().iter().flat_map(|value| value.to_le_bytes()));
            }
            _ => return Err(unsupported_type_diagnostic(field, "unsupported Arrow type")),
        }
    }
    Ok((out, elem_size(field.data_type())?))
}

fn elem_size(data_type: &DataType) -> Result<u8, SourceDiagnostic> {
    match data_type {
        DataType::Int32 | DataType::Float32 => Ok(4),
        DataType::Int64 | DataType::Float64 => Ok(8),
        _ => Err(SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedSchema,
            "$.schema",
            "Parquet schema is outside the Phase 27 primitive emission slice",
        )),
    }
}

fn total_row_count(batches: &[RecordBatch]) -> Result<usize, SourceDiagnostic> {
    batches.iter().try_fold(0usize, |sum, batch| {
        sum.checked_add(batch.num_rows()).ok_or_else(|| {
            SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedConversion,
                "$.oracle.rows",
                "Parquet Arrow oracle row count overflowed usize",
            )
        })
    })
}

fn unsupported_type_diagnostic(field: &Field, detail: &str) -> SourceDiagnostic {
    SourceDiagnostic::new(
        SourceDiagnosticCode::UnsupportedSchema,
        format!("$.schema.{}", field.name()),
        format!("{detail}; only non-null Int32/Int64/Float32/Float64 are supported"),
    )
}

fn arrow_oracle_evidence(
    batches: &[RecordBatch],
) -> Result<SourceOracleEvidence, SourceDiagnostic> {
    let row_count = total_row_count(batches)? as u64;
    if batches
        .iter()
        .flat_map(|batch| batch.columns())
        .any(|column| column.null_count() != 0)
    {
        return Err(SourceDiagnostic::new(
            SourceDiagnosticCode::OracleUnavailable,
            "$.oracle.nulls",
            "Parquet Arrow oracle scan found null values in an accepted source",
        ));
    }
    let mut evidence = SourceOracleEvidence::accepted(SourceOracleStrategy::ArrowScan, row_count);
    evidence.nulls_checked = true;
    evidence.notes.push(
        "Parquet Arrow scan is evidence only; Loom artifact verification/decode remains the acceptance path"
            .to_string(),
    );
    Ok(evidence)
}

fn non_negative_i64_to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn source_verification_failed_report(facts: &SourceFacts, status: &str) -> SourceIngressReport {
    SourceIngressReport::unsupported(
        Some(facts.clone()),
        SourceDiagnostic::new(
            SourceDiagnosticCode::VerificationFailed,
            "$.verification",
            format!("emitted Parquet LMC1 was not accepted by Loom artifact verifier: {status}"),
        ),
    )
}

fn source_oracle_failed_report(
    facts: &SourceFacts,
    oracle_report: SourceIngressReport,
) -> SourceIngressReport {
    let detail = oracle_report
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| "Parquet Arrow oracle scan failed".to_string());
    SourceIngressReport::unsupported(
        Some(facts.clone()),
        SourceDiagnostic::new(SourceDiagnosticCode::OracleUnavailable, "$.oracle", detail),
    )
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
