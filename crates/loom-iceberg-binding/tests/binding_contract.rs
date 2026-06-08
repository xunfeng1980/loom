use loom_iceberg_binding::{
    IcebergBindingEvidence, IcebergBindingFacts, IcebergBindingReport,
    IcebergBindingReportError, IcebergBindingStatus, IcebergTableRefIdentity,
};
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceFacts, SourceIdentity,
    SourceIngressReport, SourceIngressStatus, SourceLoweringDisposition, SourceOracleEvidence,
    SourceOracleStrategy, SourceSchemaFact,
};

fn identity() -> IcebergTableRefIdentity {
    IcebergTableRefIdentity {
        table_uuid: "123e4567-e89b-12d3-a456-426614174000".to_string(),
        table_name: "demo.events".to_string(),
        snapshot_id: 42,
        schema_id: 7,
        metadata_location: "file:///tmp/metadata/v1.metadata.json".to_string(),
        manifest_list_location: Some("file:///tmp/metadata/snap-42.avro".to_string()),
        ref_name: "main".to_string(),
        ref_type: "branch".to_string(),
    }
}

fn facts() -> IcebergBindingFacts {
    IcebergBindingFacts {
        identity: identity(),
        artifact_path: "fixtures/events.loom".to_string(),
        artifact_sha256: "abc123".to_string(),
    }
}

fn accepted_source_report() -> SourceIngressReport {
    let mut source_facts = SourceFacts::new(
        SourceIdentity::new("iceberg-binding-fixture", "external-source")
            .with_format_version("2"),
        3,
    );
    source_facts.root_schema = Some(SourceSchemaFact::new("$.schema", "primitive"));
    source_facts.coverage = Some(SourceCoverage::new(
        "primitive",
        "iceberg-ref",
        "sidecar-reference",
    ));

    SourceIngressReport::accepted(
        source_facts,
        SourceEmissionKind::Lmp1,
        SourceEmissionDisposition::CanonicalRaw,
        SourceLoweringDisposition::ProductionLoweringSupported,
        SourceArtifactVerificationSummary::accepted(128, "source artifact accepted"),
        SourceOracleEvidence::accepted(SourceOracleStrategy::DecodedRowFixture, 3),
    )
    .expect("accepted source report")
}

fn evidence() -> IcebergBindingEvidence {
    IcebergBindingEvidence {
        artifact_verification: SourceArtifactVerificationSummary::accepted(
            128,
            "LMC1 verifier accepted LMP1",
        ),
        source_report: accepted_source_report(),
        oracle_evidence: SourceOracleEvidence::accepted(SourceOracleStrategy::DecodedRowFixture, 3),
    }
}

#[test]
fn status_strings_are_stable() {
    assert_eq!(IcebergBindingStatus::Accepted.as_str(), "accepted");
    assert_eq!(IcebergBindingStatus::Unsupported.as_str(), "unsupported");
    assert_eq!(IcebergBindingStatus::Rejected.as_str(), "rejected");
}

#[test]
fn accepted_report_requires_facts_evidence_and_matches() {
    let report = IcebergBindingReport::accepted(
        Some(facts()),
        evidence(),
        true,
        true,
        true,
        true,
    )
    .expect("accepted binding report");

    assert_eq!(report.status, IcebergBindingStatus::Accepted);
    assert!(report.facts.is_some());
    assert!(report.diagnostics.is_empty());
    let evidence = report.evidence.as_ref().expect("accepted evidence");
    assert!(evidence.artifact_verification.accepted);
    assert_eq!(evidence.artifact_verification.artifact_byte_len, Some(128));
    assert_eq!(evidence.source_report.status, SourceIngressStatus::Accepted);
    assert!(evidence.oracle_evidence.accepted);

    assert_eq!(
        IcebergBindingReport::accepted(None, evidence.clone(), true, true, true, true),
        Err(IcebergBindingReportError::MissingFacts)
    );

    let mut no_verifier = evidence.clone();
    no_verifier.artifact_verification = SourceArtifactVerificationSummary::not_applicable();
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), no_verifier, true, true, true, true),
        Err(IcebergBindingReportError::ArtifactVerificationNotAccepted)
    );

    let mut no_bytes = evidence.clone();
    no_bytes.artifact_verification.artifact_byte_len = Some(0);
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), no_bytes, true, true, true, true),
        Err(IcebergBindingReportError::MissingArtifactBytes)
    );

    let mut no_oracle = evidence.clone();
    no_oracle.oracle_evidence = SourceOracleEvidence::unsupported("oracle not checked");
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), no_oracle, true, true, true, true),
        Err(IcebergBindingReportError::OracleEvidenceNotAccepted)
    );
}

#[test]
fn accepted_report_rejects_unaccepted_source_and_mismatch_flags() {
    let mut bad_source = evidence();
    bad_source.source_report = SourceIngressReport::unsupported(
        None,
        SourceDiagnostic::new(
            SourceDiagnosticCode::UnsupportedConversion,
            "$.binding",
            "source evidence not accepted",
        ),
    );
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), bad_source, true, true, true, true),
        Err(IcebergBindingReportError::SourceEvidenceNotAccepted)
    );

    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), evidence(), false, true, true, true),
        Err(IcebergBindingReportError::TableIdentityMismatch)
    );
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), evidence(), true, false, true, true),
        Err(IcebergBindingReportError::SnapshotMismatch)
    );
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), evidence(), true, true, false, true),
        Err(IcebergBindingReportError::SchemaMismatch)
    );
    assert_eq!(
        IcebergBindingReport::accepted(Some(facts()), evidence(), true, true, true, false),
        Err(IcebergBindingReportError::ArtifactHashMismatch)
    );
}

#[test]
fn unsupported_and_rejected_reports_do_not_carry_accepted_evidence() {
    let unsupported = IcebergBindingReport::unsupported(
        Some(facts()),
        "valid Iceberg table metadata without accepted Loom binding",
    );
    assert_eq!(unsupported.status, IcebergBindingStatus::Unsupported);
    assert!(unsupported.facts.is_some());
    assert!(unsupported.evidence.is_none());
    assert_eq!(unsupported.diagnostics.len(), 1);

    let rejected = IcebergBindingReport::rejected("malformed Iceberg metadata");
    assert_eq!(rejected.status, IcebergBindingStatus::Rejected);
    assert!(rejected.facts.is_none());
    assert!(rejected.evidence.is_none());
    assert_eq!(rejected.diagnostics, vec!["malformed Iceberg metadata"]);
}
