//! Source-neutral facts extracted from local Lance datasets.
//!
//! Lance SDK objects are adapter-private. Public helpers return only
//! `loom-source-ingress` contract data.

use std::path::Path;
use std::sync::Arc;

use arrow_array::{Array, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures::TryStreamExt;
use lance::Dataset;
use loom_core::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_core::arrow_semantic_codec::encode_arrow_semantic_payload;
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

/// Extract source-neutral facts from a local Lance dataset path.
pub async fn lance_source_facts_from_path(path: &Path) -> Result<SourceFacts, SourceIngressReport> {
    let uri = path.to_str().ok_or_else(|| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Lance dataset path is not valid UTF-8",
            ),
        )
    })?;

    if uri.contains("://") {
        return Err(rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "only local Lance dataset paths are supported by this adapter",
            ),
        ));
    }

    let dataset = Dataset::open(uri).await.map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::OpenFailed,
                "$.open",
                "local Lance dataset could not be opened",
                error.to_string(),
            ),
        )
    })?;

    let schema = Schema::from(dataset.schema());
    if schema.fields().is_empty() {
        return Err(rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::SchemaUnavailable,
                "$.schema",
                "Lance dataset did not expose an Arrow schema",
            ),
        ));
    }

    let row_count = dataset.count_rows(None).await.map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.manifest",
                "Lance dataset row count could not be read",
                error.to_string(),
            ),
        )
    })?;

    Ok(source_facts_from_dataset(path, &dataset, &schema, row_count as u64).await)
}

/// Build a byte-free source ingress report for a local Lance dataset.
///
/// Plan 27-03 classifies supported Lance shapes in `SourceCoverage`, but does
/// not emit artifact bytes or construct accepted reports.
pub async fn source_ingress_report_from_lance_path(path: &Path) -> SourceIngressReport {
    match lance_source_facts_from_path(path).await {
        Ok(facts) => {
            let diagnostic = diagnostic_for_facts(&facts);
            SourceIngressReport::unsupported(Some(facts), diagnostic)
        }
        Err(report) => report,
    }
}

/// Read a local Lance dataset through Lance's native scan path.
///
/// This is source evidence only. Accepted Loom artifact bytes come from `LMA1`
/// Arrow semantic emission plus artifact verification.
pub async fn lance_native_oracle_batches_from_path(
    path: &Path,
) -> Result<Vec<RecordBatch>, SourceIngressReport> {
    let dataset = open_local_dataset(path, "$.oracle.open").await?;
    let scanner = dataset.scan();
    let stream = scanner.try_into_stream().await.map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.oracle.scan",
                "local Lance dataset could not be scanned as native Arrow batches",
                error.to_string(),
            ),
        )
    })?;
    stream.try_collect::<Vec<_>>().await.map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::ReadFailed,
                "$.oracle.scan",
                "local Lance native oracle scan failed",
                error.to_string(),
            ),
        )
    })
}

async fn lance_arrow_schema_from_path(path: &Path) -> Result<SchemaRef, SourceIngressReport> {
    let dataset = open_local_dataset(path, "$.schema.open").await?;
    Ok(Arc::new(Schema::from(dataset.schema())))
}

/// Emit verifier-accepted `LMC1` bytes for the supported Lance slice.
pub async fn emit_source_ingress_lma1_from_lance_path(
    path: &Path,
) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let facts = lance_source_facts_from_path(path).await?;
    let coverage = facts
        .coverage
        .as_ref()
        .expect("Lance facts always include coverage");
    if coverage.support != SourceIngressStatus::Accepted {
        let diagnostic = diagnostic_for_facts(&facts);
        return Err(SourceIngressReport::unsupported(Some(facts), diagnostic));
    }

    let schema = lance_arrow_schema_from_path(path)
        .await
        .map_err(|report| source_oracle_failed_report(&facts, report))?;
    let batches = lance_native_oracle_batches_from_path(path)
        .await
        .map_err(|report| source_oracle_failed_report(&facts, report))?;
    let artifact_bytes = loom_artifact_from_batches(schema, &batches)
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
    let oracle_evidence = lance_oracle_evidence(&batches)
        .map_err(|diagnostic| SourceIngressReport::unsupported(Some(facts.clone()), diagnostic))?;
    let coverage = facts
        .coverage
        .as_ref()
        .expect("Lance facts always include coverage");
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
    .expect("accepted Lance facts map to an accepted source report");

    Ok(SourceIngressAcceptedArtifact {
        bytes: artifact_bytes,
        report,
    })
}

async fn source_facts_from_dataset(
    path: &Path,
    dataset: &Dataset,
    schema: &Schema,
    row_count: u64,
) -> SourceFacts {
    let mut facts = SourceFacts::new(
        SourceIdentity::new("lance", "external-source")
            .with_format_version(dataset.version_id().to_string())
            .with_path_display(path.display().to_string()),
        row_count,
    );

    let root_schema = root_schema_fact(schema);
    facts.root_schema = Some(root_schema.clone());
    facts.schema_facts.push(root_schema);
    facts
        .schema_facts
        .extend(schema.fields().iter().map(|field| field_schema_fact(field)));
    facts.layout_facts = layout_facts(dataset, schema, row_count).await;
    facts.split_facts = split_facts(dataset).await;
    facts.coverage = Some(coverage_from_schema(schema, dataset.count_fragments()));
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

async fn layout_facts(dataset: &Dataset, schema: &Schema, row_count: u64) -> Vec<SourceLayoutFact> {
    let fragments = dataset.get_fragments();
    let mut facts = Vec::with_capacity(fragments.len() + 1);
    let version = dataset.version();
    let mut manifest = SourceLayoutFact::new("$.manifest", "lance-manifest");
    manifest.row_count = Some(row_count);
    manifest.child_count = dataset.count_fragments();
    manifest.child_names = fragments
        .iter()
        .map(|fragment| format!("fragment[{}]", fragment.id()))
        .collect();
    manifest.physical_refs = vec![
        format!("version={}", version.version),
        format!("schema_fields={}", schema.fields().len()),
        format!("fragments={}", dataset.count_fragments()),
        format!("metadata_keys={}", version.metadata.len()),
    ];
    manifest.metadata_byte_len = Some(format!("{version:?}").len());
    facts.push(manifest);

    for (index, fragment) in fragments.iter().enumerate() {
        let logical_rows = fragment
            .count_rows(None)
            .await
            .ok()
            .map(|count| count as u64);
        let physical_rows = fragment
            .physical_rows()
            .await
            .ok()
            .map(|count| count as u64);
        let validation = if fragment.validate().await.is_ok() {
            "ok"
        } else {
            "failed"
        };
        let mut layout = SourceLayoutFact::new(format!("$.fragments[{index}]"), "lance-fragment");
        layout.row_count = logical_rows.or(physical_rows);
        layout.child_count = fragment.num_data_files();
        layout.child_names = (0..fragment.num_data_files())
            .map(|file_index| format!("data_file[{file_index}]"))
            .collect();
        layout.physical_refs = vec![
            format!("fragment_id={}", fragment.id()),
            format!("data_files={}", fragment.num_data_files()),
            format!(
                "physical_rows={}",
                physical_rows
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            format!("validation={validation}"),
        ];
        if let Some(metadata_rows) = fragment.metadata().num_rows() {
            layout
                .physical_refs
                .push(format!("manifest_rows={metadata_rows}"));
        }
        layout.metadata_byte_len = Some(format!("{:?}", fragment.metadata()).len());
        facts.push(layout);
    }

    facts
}

async fn split_facts(dataset: &Dataset) -> Vec<SourceSplitFact> {
    let mut start_row = 0u64;
    let mut splits = Vec::with_capacity(dataset.count_fragments());
    for (index, fragment) in dataset.get_fragments().iter().enumerate() {
        let row_count = match fragment.count_rows(None).await {
            Ok(count) => count as u64,
            Err(_) => fragment
                .physical_rows()
                .await
                .map(|count| count as u64)
                .unwrap_or(0),
        };
        let end_row = start_row.saturating_add(row_count);
        splits.push(SourceSplitFact::new(index, start_row, end_row));
        start_row = end_row;
    }
    splits
}

fn coverage_from_schema(schema: &Schema, fragment_count: usize) -> SourceCoverage {
    let field_count = schema.fields().len();
    let has_nullable = schema.fields().iter().any(|field| field.is_nullable());
    let mut coverage = SourceCoverage::new(
        if field_count == 1 {
            logical_kind_for_field(&schema.fields()[0]).to_string()
        } else {
            "struct".to_string()
        },
        "lance-fragments",
        "arrow-record-batch",
    );
    coverage.nullability = Some(has_nullable);
    coverage.has_splits = fragment_count > 0;
    coverage.has_statistics = false;

    if field_count > 0 {
        coverage.support = SourceIngressStatus::Accepted;
        coverage.emission_kind = SourceEmissionKind::ArrowSemantic;
        coverage.emission_disposition = SourceEmissionDisposition::SemanticArrow;
        coverage.lowering_disposition = SourceLoweringDisposition::InterpreterOnly;
        coverage
            .notes
            .push("Lance scanner materializes this schema for LMA1 semantic emission".to_string());
    } else {
        coverage.support = SourceIngressStatus::Unsupported;
        coverage.notes.push(unsupported_note(schema));
    }

    coverage
}

fn unsupported_note(schema: &Schema) -> String {
    if schema.fields().iter().any(|field| field.is_nullable()) {
        "nullable Lance fields are unsupported for Phase 27 emission".to_string()
    } else if schema
        .fields()
        .iter()
        .any(|field| field_has_extension_metadata(field))
    {
        "extension Lance fields are unsupported for Phase 27 emission".to_string()
    } else if schema
        .fields()
        .iter()
        .any(|field| matches!(field.data_type(), DataType::Utf8 | DataType::LargeUtf8))
    {
        "string Lance fields are unsupported for Phase 27 emission".to_string()
    } else if schema
        .fields()
        .iter()
        .any(|field| matches!(field.data_type(), DataType::Struct(_) | DataType::List(_)))
    {
        "nested Lance fields are unsupported for Phase 27 emission".to_string()
    } else {
        "Lance schema is outside the non-null Int32/Int64/Float32/Float64 slice".to_string()
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
            "Lance source shape is supported for canonical emission, but artifact bytes are deferred to a later plan",
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
            "nullable Lance fields are outside the supported emission slice",
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
            "nested or dictionary Lance fields are outside the supported emission slice",
        );
    }

    SourceDiagnostic::new(
        SourceDiagnosticCode::UnsupportedConversion,
        "$.schema",
        "Lance schema is valid but cannot be converted to a Phase 27 Loom artifact",
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

async fn open_local_dataset(
    path: &Path,
    diagnostic_path: &'static str,
) -> Result<Dataset, SourceIngressReport> {
    let uri = path.to_str().ok_or_else(|| {
        rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                diagnostic_path,
                "local Lance dataset path is not valid UTF-8",
            ),
        )
    })?;

    if uri.contains("://") {
        return Err(rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::OpenFailed,
                diagnostic_path,
                "only local Lance dataset paths are supported by this adapter",
            ),
        ));
    }

    Dataset::open(uri).await.map_err(|error| {
        rejected_report(
            path,
            diagnostic_with_detail(
                SourceDiagnosticCode::OpenFailed,
                diagnostic_path,
                "local Lance dataset could not be opened",
                error.to_string(),
            ),
        )
    })
}

fn loom_artifact_from_batches(
    schema: SchemaRef,
    batches: &[RecordBatch],
) -> Result<Vec<u8>, SourceDiagnostic> {
    let semantic_batches = batches
        .iter()
        .map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedConversion,
                "$.payload",
                format!("failed to build Lance Arrow semantic batches: {err}"),
            )
        })?;
    let payload = ArrowSemanticPayload::try_new(schema, semantic_batches).map_err(|err| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.payload",
            format!("failed to build Lance Arrow semantic payload: {err}"),
        )
    })?;
    encode_arrow_semantic_payload(&payload).map_err(|err| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.payload",
            format!("failed to encode Lance LMA1 payload: {err}"),
        )
    })
}

#[allow(dead_code)]
fn legacy_lmc1_artifact_from_batches(batches: &[RecordBatch]) -> Result<Vec<u8>, SourceDiagnostic> {
    let first = batches.first().ok_or_else(|| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::OracleUnavailable,
            "$.oracle.scan",
            "Lance native oracle scan produced no batches",
        )
    })?;
    if first.num_columns() == 0 {
        return Err(SourceDiagnostic::new(
            SourceDiagnosticCode::SchemaUnavailable,
            "$.schema",
            "Lance native oracle scan produced an empty schema",
        ));
    }

    if first.num_columns() == 1 {
        let layout = layout_from_batches(first.schema().field(0), batches, 0)?;
        let payload = encode_layout_payload(&layout);
        return wrap_layout_payload(&payload).map_err(|err| {
            SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedConversion,
                "$.payload",
                format!("failed to wrap Lance LMP1 payload in LMC1: {err}"),
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
            format!("failed to encode Lance LMT1 payload: {err}"),
        )
    })?;
    wrap_table_payload(&payload).map_err(|err| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.payload",
            format!("failed to wrap Lance LMT1 payload in LMC1: {err}"),
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
            "nullable Lance fields cannot emit Phase 27 Loom artifacts",
        ));
    }
    if field_has_extension_metadata(field) {
        return Err(SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedSchema,
            format!("$.schema.{}", field.name()),
            "extension Lance fields cannot emit Phase 27 Loom artifacts",
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
                "Lance arrays with null values cannot emit Phase 27 Loom artifacts",
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
            "Lance schema is outside the Phase 27 primitive emission slice",
        )),
    }
}

fn total_row_count(batches: &[RecordBatch]) -> Result<usize, SourceDiagnostic> {
    batches.iter().try_fold(0usize, |sum, batch| {
        sum.checked_add(batch.num_rows()).ok_or_else(|| {
            SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedConversion,
                "$.oracle.rows",
                "Lance native oracle row count overflowed usize",
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

fn lance_oracle_evidence(
    batches: &[RecordBatch],
) -> Result<SourceOracleEvidence, SourceDiagnostic> {
    let row_count = total_row_count(batches)? as u64;
    let mut evidence =
        SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    if batches
        .iter()
        .flat_map(|batch| batch.columns())
        .any(|column| column.null_count() != 0)
    {
        evidence
            .notes
            .push("Lance native scan preserved source null values".to_string());
    }
    evidence.nulls_checked = true;
    evidence.notes.push(
        "Lance native scan is evidence only; Loom artifact verification/decode remains the acceptance path"
            .to_string(),
    );
    Ok(evidence)
}

fn source_verification_failed_report(facts: &SourceFacts, status: &str) -> SourceIngressReport {
    SourceIngressReport::unsupported(
        Some(facts.clone()),
        SourceDiagnostic::new(
            SourceDiagnosticCode::VerificationFailed,
            "$.verification",
            format!("emitted Lance LMA1 was not accepted by Loom artifact verifier: {status}"),
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
        .unwrap_or_else(|| "Lance native oracle scan failed".to_string());
    SourceIngressReport::unsupported(
        Some(facts.clone()),
        SourceDiagnostic::new(SourceDiagnosticCode::OracleUnavailable, "$.oracle", detail),
    )
}

fn rejected_report(path: &Path, diagnostic: SourceDiagnostic) -> SourceIngressReport {
    SourceIngressReport::rejected(
        SourceIdentity::new("lance", "external-source")
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
        .unwrap_or("Lance adapter error")
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
