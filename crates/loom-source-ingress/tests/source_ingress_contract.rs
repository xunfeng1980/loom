use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceCoverage, SourceDiagnostic,
    SourceDiagnosticCode, SourceDiagnosticFamily, SourceEmissionDisposition, SourceEmissionKind,
    SourceFacts, SourceIdentity, SourceIngressReport, SourceIngressStatus,
    SourceLayoutFact, SourceLoweringDisposition, SourceOracleEvidence, SourceOracleStrategy,
    SourceSchemaFact, SourceSegmentFact, SourceSplitFact,
};

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
