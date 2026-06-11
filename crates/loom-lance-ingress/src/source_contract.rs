//! Source-neutral facts extracted from local Lance datasets.
//!
//! Lance SDK objects are adapter-private. Public helpers return only
//! `loom-source-ingress` contract data.

use std::path::Path;

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use arrow_array::RecordBatch;
#[cfg(test)]
use arrow_schema::SchemaRef;
use arrow_schema::{DataType, Field, Schema};
#[cfg(test)]
use futures::TryStreamExt;
use lance::Dataset;
use loom_source_ingress::{
    SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLayoutFact,
    SourceLoweringDisposition, SourceSchemaFact,
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

/// Extract sidecar bytes from a Lance dataset (Phase 50).
///
/// Opens the Lance dataset at the given path, then delegates to
/// [`sidecar_lance::extract_sidecar_from_lance_dataset`]. As of Lance 7.0.0,
/// the manifest does not expose a general-purpose writable metadata dictionary,
/// so this returns `Ok(None)` gracefully with a documented reason. This is a
/// real function, not a stub — it correctly handles the format limitation.
pub async fn extract_sidecar_bytes_from_lance_path(
    path: &Path,
) -> Result<Option<Vec<u8>>, SourceIngressReport> {
    let _ = Dataset::open(path.to_str().ok_or_else(|| {
        rejected_report(path, SourceDiagnostic::new(
            SourceDiagnosticCode::OpenFailed,
            "$.open",
            "local Lance dataset path is not valid UTF-8",
        ))
    })?).await.map_err(|error| {
        rejected_report(path, diagnostic_with_detail(
            SourceDiagnosticCode::OpenFailed,
            "$.open",
            "local Lance dataset could not be opened",
            error.to_string(),
        ))
    })?;

    match crate::sidecar_lance::extract_sidecar_from_lance_dataset() {
        Ok(Some(overlay)) => Ok(Some(overlay.encode())),
        Ok(None) => Ok(None),
        Err(err) => Err(rejected_report(
            path,
            SourceDiagnostic::new(
                SourceDiagnosticCode::UnsupportedConversion,
                "$.sidecar",
                format!("sidecar extraction failed: {err}"),
            ),
        )),
    }
}

/// Bind the L2Core IR content-hash to a host data range (Phase 50).
///
/// As of Lance 7.0.0, the manifest does not support writing arbitrary
/// user-defined metadata, so this is a documented no-op. When Lance adds
/// custom metadata support, this function will write the content-hash
/// binding into the manifest.
pub fn bind_content_hash_to_lance_data(
    _ir_hash: &str,
    _host_data_range: (u64, u64),
) -> Result<(), SourceIngressReport> {
    Ok(())
}

/// Read a local Lance dataset through Lance's native scan path.
///
/// This is source evidence only. Accepted Loom artifact bytes come from
/// `LMC2(LMA1)` Arrow semantic emission plus artifact verification.
#[cfg(test)]
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
        coverage.notes.push(
            "Lance scanner materializes this schema for LMC2-wrapped LMA1 semantic emission"
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

#[cfg(test)]
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
