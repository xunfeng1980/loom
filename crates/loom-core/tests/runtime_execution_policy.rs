use loom_core::artifact_verifier::{ArtifactVerificationStatus, ConstraintDischargeStatus};
use loom_core::runtime_abi::{
    decide_runtime_execution, RuntimeDecisionInput, RuntimeEmissionDisposition,
    RuntimeExecutionDecision, RuntimeFallbackPolicy, RuntimeLoweringDisposition,
    RuntimeReaderSupport, RuntimeSafetyPolicy, UnsupportedPredicatePolicy,
};

fn native_ready_input() -> RuntimeDecisionInput {
    RuntimeDecisionInput {
        artifact_status: ArtifactVerificationStatus::Accepted,
        constraint_status: ConstraintDischargeStatus::Discharged,
        production_lowering_supported: true,
        reader_support: RuntimeReaderSupport::Accepted,
        emission_disposition: RuntimeEmissionDisposition::CanonicalRaw,
        lowering_disposition: RuntimeLoweringDisposition::ProductionLoweringSupported,
        projection_supported: true,
        predicate_supported: true,
        split_supported: true,
        concurrency_safe: true,
        policy: RuntimeSafetyPolicy::default(),
    }
}

#[test]
fn native_candidate_requires_all_trusted_preconditions() {
    let report = decide_runtime_execution(&native_ready_input());
    assert_eq!(report.decision, RuntimeExecutionDecision::NativeCandidate);
    assert!(report.diagnostics.is_empty());
    assert!(report.is_native_candidate());
}

#[test]
fn rejected_artifact_fails_closed() {
    let mut input = native_ready_input();
    input.artifact_status = ArtifactVerificationStatus::Rejected;

    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
    assert_eq!(report.diagnostics[0].code.as_str(), "verifier-rejected");
}

#[test]
fn collected_constraints_never_choose_native() {
    let mut input = native_ready_input();
    input.constraint_status = ConstraintDischargeStatus::CollectedOnly;
    input.policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;

    let report = decide_runtime_execution(&input);
    assert_eq!(
        report.decision,
        RuntimeExecutionDecision::InterpreterFallback
    );
    assert_eq!(report.diagnostics[0].code.as_str(), "constraint-rejected");
}

#[test]
fn interpreter_only_requires_explicit_fallback() {
    let mut input = native_ready_input();
    input.production_lowering_supported = false;
    input.lowering_disposition = RuntimeLoweringDisposition::InterpreterOnly;

    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);

    input.policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    let report = decide_runtime_execution(&input);
    assert_eq!(
        report.decision,
        RuntimeExecutionDecision::InterpreterFallback
    );
}

#[test]
fn unsupported_predicate_policy_can_scan_all_or_fail_closed() {
    let mut input = native_ready_input();
    input.predicate_supported = false;

    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
    assert_eq!(report.diagnostics[0].code.as_str(), "unsupported-predicate");

    input.policy.unsupported_predicate = UnsupportedPredicatePolicy::ScanAll;
    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::NativeCandidate);
    assert_eq!(report.diagnostics[0].code.as_str(), "unsupported-predicate");
}
