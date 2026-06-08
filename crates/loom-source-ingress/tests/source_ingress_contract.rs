use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic,
    SourceDiagnosticCode, SourceDiagnosticFamily, SourceEmissionDisposition, SourceEmissionKind,
    SourceFacts, SourceIdentity, SourceIngressReport, SourceIngressStatus,
    SourceLayoutFact, SourceLoweringDisposition, SourceOracleEvidence, SourceOracleStrategy,
    SourceSchemaFact, SourceSegmentFact, SourceSplitFact,
};

fn manifest_text() -> String {
    std::fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR")))
        .expect("read manifest")
}

fn dependency_section(text: &str) -> Vec<&str> {
    let mut in_dependencies = false;
    let mut lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && trimmed.starts_with('[') {
            break;
        }
        if in_dependencies && !trimmed.is_empty() && !trimmed.starts_with('#') {
            lines.push(trimmed);
        }
    }

    lines
}

fn forbidden_source_markers() -> Vec<String> {
    [
        ("Vor", "tex"),
        ("vor", "tex"),
        ("fast", "lanes"),
        ("L", "ance"),
        ("Par", "quet"),
        ("Ice", "berg"),
        ("M", "CAP"),
        ("Z", "arr"),
        ("Le", "Robot"),
        ("object", "_store"),
        ("duck", "db"),
        ("mel", "ior"),
    ]
    .into_iter()
    .map(|(left, right)| format!("{left}{right}"))
    .collect()
}

#[test]
fn source_ingress_contract_public_types_exist() {
    let _ = SourceIngressStatus::Accepted;
    let _ = SourceIdentity::new("mock", "mock-format");
    let _ = SourceDiagnosticCode::UnsupportedConversion;
    let _ = SourceDiagnosticFamily::Support;
    let _ = SourceDiagnostic::new(
        SourceDiagnosticCode::UnsupportedConversion,
        "$.shape",
        "unsupported source shape",
    );
    let _ = SourceSchemaFact::new("$", "primitive");
    let _ = SourceLayoutFact::new("$", "raw");
    let _ = SourceSegmentFact::new(0, 0, 16);
    let _ = SourceSplitFact::new(0, 0, 4);
    let _ = SourceCoverage::new("primitive", "raw", "primitive");
    let _ = SourceFacts::new(SourceIdentity::new("mock", "mock-format"), 4);
    let _ = SourceEmissionKind::None;
    let _ = SourceEmissionDisposition::None;
    let _ = SourceLoweringDisposition::InterpreterOnly;
    let _ = SourceOracleStrategy::Unsupported;
    let _ = SourceOracleEvidence::unsupported("not supported by mock adapter");
    let _ = SourceArtifactVerificationSummary::not_applicable();
    let _ = SourceIngressReport::rejected(
        SourceIdentity::new("mock", "mock-format"),
        SourceDiagnostic::new(SourceDiagnosticCode::OpenFailed, "$", "open failed"),
    );
}

#[test]
fn crate_manifest_documents_and_enforces_dependency_hygiene() {
    let manifest = manifest_text();

    assert!(manifest.contains("no source SDK dependencies"));
    assert!(
        dependency_section(&manifest).is_empty(),
        "generic source contract crate must not carry runtime dependencies"
    );
}

#[test]
fn contract_sources_do_not_contain_source_specific_public_vocabulary() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let source = std::fs::read_to_string(format!("{manifest_dir}/src/lib.rs")).expect("read src");
    let tests =
        std::fs::read_to_string(format!("{manifest_dir}/tests/source_ingress_contract.rs"))
            .expect("read tests");

    for marker in forbidden_source_markers() {
        assert!(
            !source.contains(&marker),
            "source contract API leaked marker {marker}"
        );
        assert!(
            !tests.contains(&marker),
            "source contract tests leaked marker {marker}"
        );
    }
}

fn mock_identity() -> SourceIdentity {
    SourceIdentity::new("mock-buffer", "mock-format").with_format_version("1")
}

fn mock_facts(row_count: u64) -> SourceFacts {
    let mut facts = SourceFacts::new(mock_identity(), row_count);
    facts.root_schema = Some(SourceSchemaFact::new("$", "primitive"));
    facts.schema_facts.push(SourceSchemaFact::new("$", "primitive"));
    facts.layout_facts.push(SourceLayoutFact::new("$", "raw"));
    facts.segment_facts.push(SourceSegmentFact::new(0, 0, 16));
    facts.split_facts.push(SourceSplitFact::new(0, 0, row_count));
    facts.coverage = Some(SourceCoverage::new("primitive", "raw", "primitive"));
    facts
}

fn mock_diagnostic() -> SourceDiagnostic {
    SourceDiagnostic::new(
        SourceDiagnosticCode::UnsupportedConversion,
        "$.payload",
        "shape is valid but unsupported",
    )
}

#[test]
fn source_ingress_contract_stable_strings() {
    assert_eq!(SourceIngressStatus::Accepted.as_str(), "accepted");
    assert_eq!(SourceIngressStatus::Unsupported.as_str(), "unsupported");
    assert_eq!(SourceIngressStatus::Rejected.as_str(), "rejected");

    assert_eq!(SourceEmissionKind::None.as_str(), "none");
    assert_eq!(SourceEmissionKind::Lmp1.as_str(), "LMP1");
    assert_eq!(SourceEmissionKind::Lmt1.as_str(), "LMT1");

    assert_eq!(SourceEmissionDisposition::None.as_str(), "none");
    assert_eq!(
        SourceEmissionDisposition::CanonicalRaw.as_str(),
        "canonical-raw"
    );
    assert_eq!(
        SourceEmissionDisposition::CanonicalTable.as_str(),
        "canonical-table"
    );
    assert_eq!(
        SourceEmissionDisposition::StructuredLayout.as_str(),
        "structured-layout"
    );

    assert_eq!(
        SourceLoweringDisposition::InterpreterOnly.as_str(),
        "interpreter-only"
    );
    assert_eq!(
        SourceLoweringDisposition::ProductionLoweringSupported.as_str(),
        "production-lowering-supported"
    );
    assert_eq!(
        SourceLoweringDisposition::FailClosedDeferred.as_str(),
        "fail-closed/deferred"
    );

    assert_eq!(
        SourceOracleStrategy::DecodedRowFixture.as_str(),
        "decoded-row-fixture"
    );
}

#[test]
fn accepted_report_requires_facts_verifier_acceptance_and_oracle_evidence() {
    let artifact = SourceArtifactVerificationSummary::accepted(128, "artifact accepted");
    let oracle = SourceOracleEvidence::accepted(SourceOracleStrategy::DecodedRowFixture, 3);
    let report = SourceIngressReport::accepted(
        mock_facts(3),
        SourceEmissionKind::Lmp1,
        SourceEmissionDisposition::CanonicalRaw,
        SourceLoweringDisposition::ProductionLoweringSupported,
        artifact,
        oracle,
    )
    .expect("accepted report");

    assert_eq!(report.status, SourceIngressStatus::Accepted);
    assert!(report.facts.is_some());
    assert!(report.artifact_verification.accepted);
    assert!(report.artifact_verification.artifact_byte_len.is_some());
    assert!(report.oracle_evidence.as_ref().is_some_and(|e| e.accepted));

    let no_artifact = SourceIngressReport::accepted(
        mock_facts(3),
        SourceEmissionKind::None,
        SourceEmissionDisposition::None,
        SourceLoweringDisposition::FailClosedDeferred,
        SourceArtifactVerificationSummary::not_applicable(),
        SourceOracleEvidence::accepted(SourceOracleStrategy::DecodedRowFixture, 3),
    );
    assert!(no_artifact.is_err());

    let no_oracle = SourceIngressReport::accepted(
        mock_facts(3),
        SourceEmissionKind::Lmt1,
        SourceEmissionDisposition::CanonicalTable,
        SourceLoweringDisposition::ProductionLoweringSupported,
        SourceArtifactVerificationSummary::accepted(256, "artifact accepted"),
        SourceOracleEvidence::unsupported("oracle not checked"),
    );
    assert!(no_oracle.is_err());
}

#[test]
fn unsupported_valid_report_may_carry_facts_but_no_artifact_bytes() {
    let report = SourceIngressReport::unsupported(Some(mock_facts(3)), mock_diagnostic());

    assert_eq!(report.status, SourceIngressStatus::Unsupported);
    assert!(report.facts.is_some());
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
    assert!(!report.artifact_verification.accepted);
    assert!(report.artifact_verification.artifact_byte_len.is_none());
    assert!(report.oracle_evidence.is_none());
    assert!(!report.diagnostics.is_empty());
}

#[test]
fn rejected_malformed_report_exposes_diagnostics_and_no_trusted_facts() {
    let report = SourceIngressReport::rejected(mock_identity(), mock_diagnostic());

    assert_eq!(report.status, SourceIngressStatus::Rejected);
    assert!(report.facts.is_none());
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
    assert!(!report.artifact_verification.accepted);
    assert!(report.artifact_verification.artifact_byte_len.is_none());
    assert!(report.oracle_evidence.is_none());
    assert!(!report.diagnostics.is_empty());
}
