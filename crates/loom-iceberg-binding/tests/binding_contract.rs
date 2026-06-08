use loom_iceberg_binding::{
    iceberg_binding_facts_from_paths, source_ingress_report_from_iceberg_metadata_path,
    IcebergBindingEvidence, IcebergBindingFacts, IcebergBindingReport, IcebergBindingReportError,
    IcebergBindingStatus, IcebergTableRefIdentity,
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
        SourceIdentity::new("iceberg-binding-fixture", "external-source").with_format_version("2"),
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
    let report = IcebergBindingReport::accepted(Some(facts()), evidence(), true, true, true, true)
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

fn local_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/local")
        .join(name)
}

#[test]
fn local_metadata_and_sidecar_parse_to_descriptive_binding_facts() {
    let facts = iceberg_binding_facts_from_paths(
        &local_fixture("accepted-table-metadata.json"),
        &local_fixture("accepted-table-loom-binding.json"),
    )
    .expect("binding facts");

    assert_eq!(
        facts.identity.table_uuid,
        "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30"
    );
    assert_eq!(facts.identity.table_name, "demo.events");
    assert_eq!(facts.identity.snapshot_id, 314159);
    assert_eq!(facts.identity.schema_id, 7);
    assert_eq!(
        facts.identity.metadata_location,
        "tests/fixtures/local/metadata/v1.metadata.json"
    );
    assert_eq!(
        facts.identity.manifest_list_location.as_deref(),
        Some("tests/fixtures/local/metadata/snap-314159.avro")
    );
    assert_eq!(facts.identity.ref_name, "main");
    assert_eq!(facts.identity.ref_type, "branch");
    assert_eq!(
        facts.artifact_path,
        "tests/fixtures/local/artifacts/demo-events.lmc1.loom"
    );
    assert_eq!(
        facts.artifact_sha256,
        "4cfcf1c6e9233e2f2fc97a0162f5e9c60bb92f9e5f5c9572de700f98474421b7"
    );

    let report = source_ingress_report_from_iceberg_metadata_path(&local_fixture(
        "accepted-table-metadata.json",
    ));
    assert_eq!(report.status, SourceIngressStatus::Unsupported);
    assert!(report.facts.is_some());
    assert!(report.artifact_verification.artifact_byte_len.is_none());
    assert!(!report.artifact_verification.accepted);
    assert!(report.oracle_evidence.is_none());
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
}

#[test]
fn remote_or_catalog_metadata_is_unsupported_and_byte_free() {
    let report = source_ingress_report_from_iceberg_metadata_path(&local_fixture(
        "unsupported-remote-metadata.json",
    ));

    assert_eq!(report.status, SourceIngressStatus::Unsupported);
    assert_eq!(report.identity.source_kind, "iceberg-binding");
    assert_eq!(report.identity.format, "external-source");
    assert_eq!(report.identity.format_version.as_deref(), Some("2"));
    assert!(report.facts.is_some());
    assert_eq!(report.artifact_verification.artifact_byte_len, None);
    assert!(!report.artifact_verification.accepted);
    assert!(report.oracle_evidence.is_none());
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("remote or catalog")));
}

#[test]
fn missing_identity_is_rejected_with_diagnostics_only() {
    let report = source_ingress_report_from_iceberg_metadata_path(&local_fixture(
        "rejected-missing-identity.json",
    ));

    assert_eq!(report.status, SourceIngressStatus::Rejected);
    assert!(report.facts.is_none());
    assert!(report.oracle_evidence.is_none());
    assert_eq!(report.artifact_verification.artifact_byte_len, None);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("table UUID")));

    let error = iceberg_binding_facts_from_paths(
        &local_fixture("rejected-missing-identity.json"),
        &local_fixture("accepted-table-loom-binding.json"),
    )
    .expect_err("missing identity rejected");
    assert_eq!(error.status, IcebergBindingStatus::Rejected);
    assert!(error.facts.is_none());
    assert!(error.evidence.is_none());
}

#[test]
fn malformed_json_is_rejected_before_trusting_facts() {
    let temp = std::env::temp_dir().join(format!(
        "loom-iceberg-binding-malformed-{}.json",
        std::process::id()
    ));
    std::fs::write(&temp, "{ not valid iceberg metadata").expect("write malformed fixture");

    let report = source_ingress_report_from_iceberg_metadata_path(&temp);
    std::fs::remove_file(&temp).expect("remove malformed fixture");

    assert_eq!(report.status, SourceIngressStatus::Rejected);
    assert!(report.facts.is_none());
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("could not be parsed")));
}

#[test]
fn sidecar_accepted_claim_does_not_create_accepted_binding() {
    let facts = iceberg_binding_facts_from_paths(
        &local_fixture("accepted-table-metadata.json"),
        &local_fixture("accepted-table-loom-binding.json"),
    )
    .expect("descriptive facts");

    assert_eq!(
        facts.artifact_sha256,
        "4cfcf1c6e9233e2f2fc97a0162f5e9c60bb92f9e5f5c9572de700f98474421b7"
    );
    let report = source_ingress_report_from_iceberg_metadata_path(&local_fixture(
        "accepted-table-metadata.json",
    ));
    assert_ne!(report.status, SourceIngressStatus::Accepted);
    assert!(!report.artifact_verification.accepted);
    assert!(report.oracle_evidence.is_none());
}
