//! Source-neutral view of the existing Vortex reader contract.
//!
//! This module is an adapter only: it does not change the Vortex reader API.
//! Accepted artifact handoff is verifier-routed: source facts alone never
//! authorize `.loom` bytes.

use std::path::Path;

use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLayoutFact, SourceLoweringDisposition,
    SourceOracleEvidence, SourceOracleStrategy, SourceSchemaFact, SourceSegmentFact,
    SourceSplitFact,
};

use crate::{
    emit_supported_lmc1_from_vortex_buffer, opened_buffer_or_report,
    reader_facts_from_vortex_buffer, reader_facts_from_vortex_path,
    scan_supported_single_column_layout, scan_supported_table, VortexEmissionDisposition,
    VortexEncodingCoverage, VortexFileFacts, VortexIngressDiagnostic, VortexIngressDiagnosticCode,
    VortexIngressReport, VortexIngressSourceKind, VortexIngressStatus, VortexLoweringDisposition,
    VortexReaderDTypeFact, VortexReaderDiagnostic, VortexReaderDiagnosticCode,
    VortexReaderEmissionKind, VortexReaderFacts, VortexReaderLayoutFact, VortexReaderSegmentFact,
    VortexReaderSplitFact, VortexReaderSupport,
};

/// Verifier-accepted source artifact handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIngressAcceptedArtifact {
    pub bytes: Vec<u8>,
    pub report: SourceIngressReport,
}

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

/// Build a generic source report from an in-memory Vortex buffer.
pub fn source_ingress_report_from_vortex_buffer(bytes: &[u8]) -> SourceIngressReport {
    match reader_facts_from_vortex_buffer(bytes) {
        Ok(facts) => source_report_from_vortex_reader_facts(&facts),
        Err(report) => source_report_from_vortex_ingress_report(report),
    }
}

/// Build a generic source report from a local Vortex path.
pub fn source_ingress_report_from_vortex_path(path: &Path) -> SourceIngressReport {
    match reader_facts_from_vortex_path(path) {
        Ok(facts) => source_report_from_vortex_reader_facts(&facts),
        Err(report) => source_report_from_vortex_ingress_report(report),
    }
}

/// Emit `LMC1` from a Vortex buffer only after Loom artifact verification accepts it.
pub fn emit_source_ingress_lmc1_from_vortex_buffer(
    bytes: &[u8],
) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let reader_facts =
        reader_facts_from_vortex_buffer(bytes).map_err(source_report_from_vortex_ingress_report)?;

    if reader_facts.support != VortexReaderSupport::Accepted {
        return Err(source_report_from_vortex_reader_facts(&reader_facts));
    }

    let artifact_bytes = emit_supported_lmc1_from_vortex_buffer(bytes)
        .map_err(source_report_from_vortex_ingress_report)?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        return Err(source_verification_failed_report(
            &reader_facts,
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
    let oracle_evidence = source_native_oracle_evidence_from_vortex_buffer(bytes, &reader_facts)
        .map_err(|diagnostic| source_oracle_failed_report(&reader_facts, diagnostic))?;
    let source_facts = source_facts_from_vortex_reader_facts(&reader_facts);
    let coverage = source_facts
        .coverage
        .as_ref()
        .expect("Vortex reader facts always map coverage");
    let emission_kind = coverage.emission_kind;
    let emission_disposition = coverage.emission_disposition;
    let lowering_disposition = coverage.lowering_disposition;
    let mut report = SourceIngressReport::accepted(
        source_facts,
        emission_kind,
        emission_disposition,
        lowering_disposition,
        artifact_summary,
        oracle_evidence,
    )
    .expect("accepted Vortex reader facts map to an accepted source report");
    report.diagnostics = reader_facts
        .diagnostics
        .iter()
        .map(source_diagnostic_from_vortex_reader_diagnostic)
        .collect();

    Ok(SourceIngressAcceptedArtifact {
        bytes: artifact_bytes,
        report,
    })
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

fn source_native_oracle_evidence_from_vortex_buffer(
    bytes: &[u8],
    facts: &VortexReaderFacts,
) -> Result<SourceOracleEvidence, SourceDiagnostic> {
    let file = opened_buffer_or_report(bytes).map_err(|report| {
        source_diagnostic_from_vortex_ingress_diagnostic(report.diagnostics.first().unwrap_or(
            &VortexIngressDiagnostic::new(
                VortexIngressDiagnosticCode::OpenFailed,
                "$.oracle",
                "source-native oracle could not open source bytes",
            ),
        ))
    })?;

    let row_count = match facts.emission_kind {
        VortexReaderEmissionKind::Lmp1 => scan_supported_single_column_layout(&file)
            .map(|layout| layout.row_count as u64)
            .map_err(|message| source_oracle_unavailable_diagnostic(message))?,
        VortexReaderEmissionKind::Lmt1 => scan_supported_table(&file)
            .map(|table| table.row_count as u64)
            .map_err(|message| source_oracle_unavailable_diagnostic(message))?,
        VortexReaderEmissionKind::None => {
            return Err(source_oracle_unavailable_diagnostic(
                "source-native oracle is only checked for emitted artifacts",
            ))
        }
    };

    let mut evidence =
        SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    evidence.nulls_checked = true;
    evidence.notes.push(
        "source-native scan is metadata only; Loom artifact verification/decode remains the acceptance path"
            .to_string(),
    );
    Ok(evidence)
}

fn source_verification_failed_report(
    facts: &VortexReaderFacts,
    status: &str,
) -> SourceIngressReport {
    SourceIngressReport::unsupported(
        Some(source_facts_from_vortex_reader_facts(facts)),
        SourceDiagnostic::new(
            SourceDiagnosticCode::VerificationFailed,
            "$.verification",
            format!("emitted LMC1 was not accepted by Loom artifact verifier: {status}"),
        ),
    )
}

fn source_oracle_failed_report(
    facts: &VortexReaderFacts,
    diagnostic: SourceDiagnostic,
) -> SourceIngressReport {
    SourceIngressReport::unsupported(
        Some(source_facts_from_vortex_reader_facts(facts)),
        diagnostic,
    )
}

fn source_oracle_unavailable_diagnostic(message: impl Into<String>) -> SourceDiagnostic {
    SourceDiagnostic::new(SourceDiagnosticCode::OracleUnavailable, "$.oracle", message)
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
