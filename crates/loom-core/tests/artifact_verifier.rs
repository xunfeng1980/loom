use loom_core::artifact_verifier::{
    ArtifactLoweringDiagnostic, ArtifactLoweringReadiness, ArtifactVerificationDiagnostic,
    ArtifactVerificationFacts, ArtifactVerificationReport, ArtifactVerificationStage,
    ArtifactVerificationStatus, ConstraintDischargeStatus,
};

#[test]
fn accepted_report_exposes_facts() {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.container_version = Some(1);
    facts.required_features = vec!["single_column_lmp1".to_string()];
    facts.payload_kind = Some("LMP1 layout".to_string());
    facts.constraint_status = ConstraintDischargeStatus::NotRequired;

    let report = ArtifactVerificationReport::accepted(facts);

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    assert!(report.is_ok());
    let facts = report.facts().expect("accepted reports expose facts");
    assert_eq!(facts.artifact_kind, "LMC1");
    assert_eq!(facts.container_version, Some(1));
    assert_eq!(facts.constraint_status, ConstraintDischargeStatus::NotRequired);
}

#[test]
fn rejected_and_unsupported_reports_hide_facts() {
    let diagnostic = ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::Container,
        "container-shape",
        "$.container",
        "malformed container",
    );

    let rejected = ArtifactVerificationReport::rejected(vec![diagnostic.clone()]);
    assert_eq!(rejected.status(), ArtifactVerificationStatus::Rejected);
    assert!(!rejected.is_ok());
    assert!(rejected.facts().is_none());
    assert!(rejected.into_facts().is_none());

    let unsupported = ArtifactVerificationReport::unsupported(vec![diagnostic]);
    assert_eq!(unsupported.status(), ArtifactVerificationStatus::Unsupported);
    assert!(!unsupported.is_ok());
    assert!(unsupported.facts().is_none());
    assert!(unsupported.into_facts().is_none());
}

#[test]
fn diagnostic_preserves_stage_code_path_and_message() {
    let diagnostic = ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::L1Structural,
        "count-mismatch",
        "$.payload.row_count",
        "row count mismatch",
    );

    assert_eq!(diagnostic.stage, ArtifactVerificationStage::L1Structural);
    assert_eq!(diagnostic.stage.as_str(), "l1-structural");
    assert_eq!(diagnostic.code, "count-mismatch");
    assert_eq!(diagnostic.path, "$.payload.row_count");
    assert_eq!(diagnostic.message, "row count mismatch");
}

#[test]
fn enum_display_strings_are_stable() {
    assert_eq!(ArtifactVerificationStage::Container.as_str(), "container");
    assert_eq!(ArtifactVerificationStage::Manifest.as_str(), "manifest");
    assert_eq!(ArtifactVerificationStage::L2Core.as_str(), "l2core");
    assert_eq!(
        ArtifactVerificationStage::ConstraintDischarge.as_str(),
        "constraint-discharge"
    );
    assert_eq!(
        ArtifactVerificationStage::LoweringReadiness.as_str(),
        "lowering-readiness"
    );

    assert_eq!(ArtifactVerificationStatus::Accepted.as_str(), "accepted");
    assert_eq!(ArtifactVerificationStatus::Rejected.as_str(), "rejected");
    assert_eq!(
        ArtifactVerificationStatus::Unsupported.as_str(),
        "unsupported"
    );

    assert_eq!(
        ConstraintDischargeStatus::CollectedOnly.as_str(),
        "collected-only"
    );
    assert_eq!(ConstraintDischargeStatus::Discharged.as_str(), "discharged");
    assert_eq!(ConstraintDischargeStatus::Failed.as_str(), "failed");
    assert_eq!(ConstraintDischargeStatus::Unknown.as_str(), "unknown");
    assert_eq!(ConstraintDischargeStatus::Skipped.as_str(), "skipped");
}

#[test]
fn lowering_readiness_defaults_to_not_ready() {
    let default_readiness = ArtifactLoweringReadiness::default();
    assert!(!default_readiness.ready);
    assert!(default_readiness.backend.is_none());
    assert!(default_readiness.diagnostics.is_empty());

    let readiness = ArtifactLoweringReadiness::with_diagnostic(
        Some("textual-mlir"),
        ArtifactLoweringDiagnostic::new(
            "missing-l2core-facts",
            "$.facts.l2_core",
            "lowering requires L2Core facts",
        ),
    );
    assert!(!readiness.ready);
    assert_eq!(readiness.backend.as_deref(), Some("textual-mlir"));
    assert_eq!(readiness.diagnostics[0].code, "missing-l2core-facts");
}
