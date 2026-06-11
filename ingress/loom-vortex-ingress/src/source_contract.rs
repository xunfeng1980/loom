//! Source-neutral view of the existing Vortex reader contract.
//!
//! This module is an adapter only: it does not change the Vortex reader API.
//! Accepted artifact handoff is verifier-routed: source facts alone never
//! authorize `.loom` bytes.

use std::path::Path;

#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use arrow_array::RecordBatch;
#[cfg(test)]
use arrow_schema::Schema;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLayoutFact, SourceLoweringDisposition,
    SourceSchemaFact, SourceSegmentFact,
    SourceSplitFact,
};
#[cfg(test)]
use vortex_array::arrow::ArrowSessionExt;
#[cfg(test)]
use vortex_array::stream::ArrayStreamExt;
#[cfg(test)]
use vortex_array::VortexSessionExecute;
#[cfg(test)]
use vortex_io::runtime::BlockingRuntime;

use crate::{
    opened_buffer_or_report,
    reader_facts_from_vortex_buffer, reader_facts_from_vortex_path,
    sidecar_vortex,
    VortexEmissionDisposition, VortexEncodingCoverage, VortexFileFacts, VortexIngressDiagnostic,
    VortexIngressDiagnosticCode, VortexIngressReport, VortexIngressSourceKind, VortexIngressStatus,
    VortexLoweringDisposition, VortexReaderDTypeFact, VortexReaderDiagnostic,
    VortexReaderDiagnosticCode, VortexReaderEmissionKind, VortexReaderFacts, VortexReaderLayoutFact,
    VortexReaderSegmentFact, VortexReaderSplitFact, VortexReaderSupport,
};

/// Extract generic source facts from an in-memory Vortex buffer.
pub fn source_facts_from_vortex_buffer(bytes: &[u8]) -> Result<SourceFacts, SourceIngressReport> {
    reader_facts_from_vortex_buffer(bytes)
        .map(|facts| source_facts_from_vortex_reader_facts(&facts))
        .map_err(source_report_from_vortex_ingress_report)
}

/// Extract generic source facts from a local Vortex path.
pub fn source_facts_from_vortex_path(path: &Path) -> Result<SourceFacts, SourceIngressReport> {
    reader_facts_from_vortex_path(path)
        .map(|facts| source_facts_from_vortex_reader_facts(&facts))
        .map_err(source_report_from_vortex_ingress_report)
}

/// Extract sidecar bytes from a Vortex buffer (Phase 50).
///
/// Validates that the buffer can be opened as a Vortex file, then delegates to
/// [`sidecar_vortex::extract_sidecar_from_vortex_buffer`]. As of Vortex 0.74.0,
/// the footer does not expose a general-purpose metadata dictionary, so this
/// returns `Ok(None)` gracefully with a documented reason. This is a real
/// function, not a stub — it correctly handles the format limitation.
pub fn extract_sidecar_bytes_from_vortex_buffer(
    bytes: &[u8],
) -> Result<Option<Vec<u8>>, SourceIngressReport> {
    let _ = opened_buffer_or_report(bytes).map_err(source_report_from_vortex_ingress_report)?;
    match sidecar_vortex::extract_sidecar_from_vortex_buffer(bytes) {
        Ok(Some(overlay)) => Ok(Some(overlay.encode())),
        Ok(None) => Ok(None),
        Err(err) => Err(SourceIngressReport::rejected(
            SourceIdentity::new("vortex", "external-source"),
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
/// As of Vortex 0.74.0, the footer does not support writing arbitrary
/// user-defined metadata, so this is a documented no-op. When Vortex adds
/// custom metadata support, this function will write the content-hash
/// binding into the footer.
pub fn bind_content_hash_to_vortex_data(
    _ir_hash: &str,
    _host_data_range: (u64, u64),
) -> Result<(), SourceIngressReport> {
    Ok(())
}

/// Materialize a Vortex buffer through the Vortex Arrow executor.
#[cfg(test)]
pub fn vortex_arrow_oracle_batches_from_buffer(
    bytes: &[u8],
) -> Result<Vec<RecordBatch>, SourceIngressReport> {
    let file = opened_buffer_or_report(bytes).map_err(source_report_from_vortex_ingress_report)?;
    let facts = || crate::facts_from_file(&file, VortexIngressSourceKind::Buffer);
    let array = crate::RUNTIME
        .block_on(async {
            let stream = file
                .scan()
                .map_err(|err| format!("failed to create Vortex Arrow scan: {err}"))?
                .into_array_stream()
                .map_err(|err| format!("failed to create Vortex Arrow array stream: {err}"))?;
            stream
                .read_all()
                .await
                .map_err(|err| format!("failed to scan Vortex Arrow rows: {err}"))
        })
        .map_err(|message| {
            source_report_from_vortex_ingress_report(VortexIngressReport::unsupported(
                Some(facts()),
                VortexIngressDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                message,
            ))
        })?;
    let mut ctx = file.session().create_execution_ctx();
    let field = file
        .session()
        .arrow()
        .to_arrow_field("value", file.dtype())
        .map_err(|err| {
            source_report_from_vortex_ingress_report(VortexIngressReport::unsupported(
                Some(facts()),
                VortexIngressDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                format!("failed to derive Vortex Arrow field: {err}"),
            ))
        })?;
    let arrow_array = file
        .session()
        .arrow()
        .execute_arrow(array, Some(&field), &mut ctx)
        .map_err(|err| {
            source_report_from_vortex_ingress_report(VortexIngressReport::unsupported(
                Some(facts()),
                VortexIngressDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                format!("failed to materialize Vortex rows as Arrow: {err}"),
            ))
        })?;
    let batch = RecordBatch::try_new(Arc::new(Schema::new(vec![field])), vec![arrow_array])
        .map_err(|err| {
            source_report_from_vortex_ingress_report(VortexIngressReport::unsupported(
                Some(facts()),
                VortexIngressDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                format!("failed to build Vortex Arrow record batch: {err}"),
            ))
        })?;
    Ok(vec![batch])
}

/// Convert rich reader facts into the generic source fact contract.
pub fn source_facts_from_vortex_reader_facts(facts: &VortexReaderFacts) -> SourceFacts {
    let mut source = SourceFacts::new(identity_from_reader_facts(facts), facts.row_count);
    source.root_schema = Some(source_schema_from_vortex_dtype(&facts.root_dtype));
    source.schema_facts = facts
        .dtype_facts
        .iter()
        .map(source_schema_from_vortex_dtype)
        .collect();
    if !source
        .schema_facts
        .iter()
        .any(|fact| fact.path == facts.root_dtype.path)
    {
        source
            .schema_facts
            .insert(0, source_schema_from_vortex_dtype(&facts.root_dtype));
    }
    source.layout_facts = facts
        .layout_facts
        .iter()
        .map(source_layout_from_vortex_layout)
        .collect();
    source.segment_facts = facts
        .segment_facts
        .iter()
        .map(source_segment_from_vortex_segment)
        .collect();
    source.split_facts = facts
        .split_facts
        .iter()
        .map(source_split_from_vortex_split)
        .collect();
    source.coverage = Some(source_coverage_from_vortex_coverage(&facts.coverage));
    source
}

/// Convert a reader fact report into a source-neutral report.
pub fn source_report_from_vortex_reader_facts(facts: &VortexReaderFacts) -> SourceIngressReport {
    let source_facts = source_facts_from_vortex_reader_facts(facts);
    let coverage = source_facts
        .coverage
        .as_ref()
        .expect("Vortex reader facts always map coverage");
    let emission_kind = coverage.emission_kind;
    let emission_disposition = coverage.emission_disposition;
    let lowering_disposition = coverage.lowering_disposition;
    let mut diagnostics = facts
        .diagnostics
        .iter()
        .map(source_diagnostic_from_vortex_reader_diagnostic)
        .collect::<Vec<_>>();
    if facts.support == VortexReaderSupport::Unsupported && diagnostics.is_empty() {
        diagnostics.push(SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.payload",
            "valid source facts were extracted, but this source shape cannot emit a Loom artifact",
        ));
    }

    SourceIngressReport {
        status: source_status_from_vortex_reader_support(facts.support),
        identity: source_facts.identity.clone(),
        facts: Some(source_facts),
        diagnostics,
        emission_kind,
        emission_disposition,
        lowering_disposition,
        artifact_verification: SourceArtifactVerificationSummary::not_applicable(),
        oracle_evidence: None,
    }
}

/// Convert an existing ingress report into a source-neutral report.
pub fn source_report_from_vortex_ingress_report(
    report: VortexIngressReport,
) -> SourceIngressReport {
    let diagnostics = report
        .diagnostics
        .iter()
        .map(source_diagnostic_from_vortex_ingress_diagnostic)
        .collect::<Vec<_>>();
    let first_diagnostic = diagnostics.first().cloned().unwrap_or_else(|| {
        SourceDiagnostic::new(
            SourceDiagnosticCode::OpenFailed,
            "$",
            "source adapter did not provide a diagnostic",
        )
    });

    match report.status {
        VortexIngressStatus::Rejected => SourceIngressReport::rejected(
            identity_from_file_facts(report.facts.as_ref()),
            first_diagnostic,
        ),
        VortexIngressStatus::Unsupported => {
            let facts = report
                .facts
                .as_ref()
                .map(source_facts_from_vortex_file_facts);
            let mut source = SourceIngressReport::unsupported(facts, first_diagnostic);
            source.diagnostics = diagnostics;
            source
        }
        VortexIngressStatus::Accepted => {
            let facts = report
                .facts
                .as_ref()
                .map(source_facts_from_vortex_file_facts);
            let identity = facts
                .as_ref()
                .map(|facts| facts.identity.clone())
                .unwrap_or_else(|| identity_from_file_facts(None));
            SourceIngressReport {
                status: SourceIngressStatus::Accepted,
                identity,
                facts,
                diagnostics,
                emission_kind: SourceEmissionKind::None,
                emission_disposition: SourceEmissionDisposition::None,
                lowering_disposition: SourceLoweringDisposition::FailClosedDeferred,
                artifact_verification: SourceArtifactVerificationSummary::not_applicable(),
                oracle_evidence: None,
            }
        }
    }
}

/// Convert coverage dispositions without changing their stable vocabulary.
pub fn source_coverage_from_vortex_coverage(coverage: &VortexEncodingCoverage) -> SourceCoverage {
    SourceCoverage {
        schema_family: coverage.dtype_kind.clone(),
        nullability: coverage.nullable,
        layout_class: coverage.layout_class.clone(),
        array_encoding: coverage.array_encoding.clone(),
        has_splits: coverage.has_splits,
        has_statistics: coverage.has_statistics,
        support: source_status_from_vortex_reader_support(coverage.reader_support),
        emission_kind: source_emission_kind_from_vortex(coverage.emission_kind),
        emission_disposition: source_emission_disposition_from_vortex(
            coverage.emission_disposition,
        ),
        lowering_disposition: source_lowering_disposition_from_vortex(
            coverage.lowering_disposition,
        ),
        notes: coverage.notes.clone(),
    }
}

/// Convert a reader diagnostic into source-neutral diagnostic vocabulary.
pub fn source_diagnostic_from_vortex_reader_diagnostic(
    diagnostic: &VortexReaderDiagnostic,
) -> SourceDiagnostic {
    let code = match diagnostic.code {
        VortexReaderDiagnosticCode::OpenFailed => SourceDiagnosticCode::OpenFailed,
        VortexReaderDiagnosticCode::SplitUnavailable => SourceDiagnosticCode::SplitUnavailable,
        VortexReaderDiagnosticCode::TraversalFailed => SourceDiagnosticCode::LayoutUnavailable,
        VortexReaderDiagnosticCode::UnsupportedLayout => SourceDiagnosticCode::UnsupportedLayout,
        VortexReaderDiagnosticCode::UnsupportedDType => SourceDiagnosticCode::UnsupportedSchema,
        VortexReaderDiagnosticCode::UnsupportedConversion => {
            SourceDiagnosticCode::UnsupportedConversion
        }
        VortexReaderDiagnosticCode::VerificationRequired => {
            SourceDiagnosticCode::VerificationFailed
        }
    };
    SourceDiagnostic::new(code, diagnostic.path.clone(), diagnostic.message.clone())
        .with_source_detail(diagnostic.code.as_str())
}

/// Convert an ingress diagnostic into source-neutral diagnostic vocabulary.
pub fn source_diagnostic_from_vortex_ingress_diagnostic(
    diagnostic: &VortexIngressDiagnostic,
) -> SourceDiagnostic {
    let code = match diagnostic.code {
        VortexIngressDiagnosticCode::NotYetInspected => SourceDiagnosticCode::NotYetInspected,
        VortexIngressDiagnosticCode::OpenFailed => SourceDiagnosticCode::OpenFailed,
        VortexIngressDiagnosticCode::UnsupportedLayout => SourceDiagnosticCode::UnsupportedLayout,
        VortexIngressDiagnosticCode::UnsupportedDType => SourceDiagnosticCode::UnsupportedSchema,
        VortexIngressDiagnosticCode::UnsupportedConversion => {
            SourceDiagnosticCode::UnsupportedConversion
        }
    };
    SourceDiagnostic::new(code, diagnostic.path.clone(), diagnostic.message.clone())
        .with_source_detail(diagnostic.code.as_str())
}

fn source_facts_from_vortex_file_facts(facts: &VortexFileFacts) -> SourceFacts {
    let mut source = SourceFacts::new(identity_from_file_facts(Some(facts)), facts.row_count);
    let mut schema = SourceSchemaFact::new("$", dtype_kind_from_summary(&facts.dtype_summary));
    schema.arrow_summary = Some(facts.dtype_summary.clone());
    source.root_schema = Some(schema.clone());
    source.schema_facts.push(schema);

    let mut layout = SourceLayoutFact::new("$", facts.layout_summary.clone());
    layout.row_count = Some(facts.row_count);
    layout.physical_refs = (0..facts.segment_count)
        .map(|index| format!("segment[{index}]"))
        .collect();
    layout.metadata_byte_len = facts.footer_approx_byte_size;
    source.layout_facts.push(layout);

    source.segment_facts = facts
        .segment_ranges
        .iter()
        .enumerate()
        .map(|(index, (start, end))| {
            let mut fact = SourceSegmentFact::new(index, *start, *end);
            fact.alignment = facts.alignment_summary.get(index).cloned();
            fact
        })
        .collect();

    source
}

fn source_schema_from_vortex_dtype(dtype: &VortexReaderDTypeFact) -> SourceSchemaFact {
    let mut fact = SourceSchemaFact::new(dtype.path.clone(), dtype.kind.clone());
    fact.nullable = dtype.nullable;
    fact.field_count = dtype.field_count;
    fact.field_names = dtype.field_names.clone();
    fact.arrow_summary = Some(dtype.summary.clone());
    fact
}

fn source_layout_from_vortex_layout(layout: &VortexReaderLayoutFact) -> SourceLayoutFact {
    let mut fact = SourceLayoutFact::new(layout.path.clone(), layout.encoding_id.clone());
    fact.row_count = Some(layout.row_count);
    fact.child_count = layout.child_count;
    fact.child_names = layout.child_name.iter().cloned().collect();
    fact.physical_refs = layout
        .segment_ids
        .iter()
        .map(|id| format!("segment[{id}]"))
        .collect();
    fact.metadata_byte_len = Some(layout.metadata_byte_len);
    fact
}

fn source_segment_from_vortex_segment(segment: &VortexReaderSegmentFact) -> SourceSegmentFact {
    SourceSegmentFact {
        index: segment.index,
        start: segment.start,
        end: segment.end,
        length: segment.length,
        alignment: Some(segment.alignment.clone()),
        ordered_after_previous: segment.ordered_after_previous,
        overlaps_previous: segment.overlaps_previous,
    }
}

fn source_split_from_vortex_split(split: &VortexReaderSplitFact) -> SourceSplitFact {
    SourceSplitFact {
        index: split.index,
        start_row: split.start_row,
        end_row: split.end_row,
        row_count: split.row_count,
    }
}

fn identity_from_reader_facts(facts: &VortexReaderFacts) -> SourceIdentity {
    identity_from_parts(facts.source_kind, facts.vortex_file_version)
}

fn identity_from_file_facts(facts: Option<&VortexFileFacts>) -> SourceIdentity {
    facts.map_or_else(
        || SourceIdentity::new("unknown", "external-source"),
        |facts| identity_from_parts(facts.source_kind, facts.vortex_file_version),
    )
}

fn identity_from_parts(source_kind: VortexIngressSourceKind, version: u16) -> SourceIdentity {
    SourceIdentity::new(source_kind.as_str(), "external-source")
        .with_format_version(version.to_string())
}

fn source_status_from_vortex_reader_support(support: VortexReaderSupport) -> SourceIngressStatus {
    match support {
        VortexReaderSupport::Accepted => SourceIngressStatus::Accepted,
        VortexReaderSupport::Unsupported => SourceIngressStatus::Unsupported,
        VortexReaderSupport::Rejected => SourceIngressStatus::Rejected,
    }
}

fn source_emission_kind_from_vortex(kind: VortexReaderEmissionKind) -> SourceEmissionKind {
    match kind {
        VortexReaderEmissionKind::None => SourceEmissionKind::None,
        VortexReaderEmissionKind::Lmp1 => SourceEmissionKind::Lmp1,
        VortexReaderEmissionKind::Lmt1 => SourceEmissionKind::Lmt1,
    }
}

fn source_emission_disposition_from_vortex(
    disposition: VortexEmissionDisposition,
) -> SourceEmissionDisposition {
    match disposition {
        VortexEmissionDisposition::None => SourceEmissionDisposition::None,
        VortexEmissionDisposition::CanonicalRaw => SourceEmissionDisposition::CanonicalRaw,
        VortexEmissionDisposition::CanonicalTable => SourceEmissionDisposition::CanonicalTable,
        VortexEmissionDisposition::StructuredLayout => SourceEmissionDisposition::StructuredLayout,
    }
}

fn source_lowering_disposition_from_vortex(
    disposition: VortexLoweringDisposition,
) -> SourceLoweringDisposition {
    match disposition {
        VortexLoweringDisposition::InterpreterOnly => SourceLoweringDisposition::InterpreterOnly,
        VortexLoweringDisposition::ProductionLoweringSupported => {
            SourceLoweringDisposition::ProductionLoweringSupported
        }
        VortexLoweringDisposition::FailClosedDeferred => {
            SourceLoweringDisposition::FailClosedDeferred
        }
    }
}

fn dtype_kind_from_summary(summary: &str) -> &'static str {
    if summary.contains("Struct") {
        "struct"
    } else if summary.contains("Utf8") {
        "utf8"
    } else if summary.contains("Primitive") || summary.contains("I32") || summary.contains("I64") {
        "primitive"
    } else {
        "unknown"
    }
}
