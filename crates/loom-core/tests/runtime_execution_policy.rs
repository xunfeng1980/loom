use loom_core::artifact_types::{ArtifactVerificationStatus};
use loom_core::runtime_abi::{
    decide_runtime_execution, plan_split, ConcurrencyPolicy, RuntimeDecisionInput,
    RuntimeDiagnosticCode, RuntimeEmissionDisposition, RuntimeExecutionDecision,
    RuntimeFallbackPolicy, RuntimeLoweringDisposition, RuntimeReaderSupport, RuntimeSafetyPolicy,
    ScanShape, SplitDescriptor, UnsupportedPredicatePolicy,
};

fn native_ready_input() -> RuntimeDecisionInput {
    RuntimeDecisionInput {
        artifact_status: ArtifactVerificationStatus::Accepted,
        constraints_discharged: false,
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
fn collected_constraints_still_choose_native_when_lowering_supported() {
    // Phase A–C: runtime no longer gates on constraints_discharged.
    let mut input = native_ready_input();
    input.constraints_discharged = false;
    input.policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;

    let report = decide_runtime_execution(&input);
    assert_eq!(
        report.decision,
        RuntimeExecutionDecision::NativeCandidate
    );
    assert!(report.diagnostics.is_empty());
}

#[test]
fn interpreter_only_requires_explicit_fallback() {
    let mut input = native_ready_input();
    input.production_lowering_supported = false;
    input.lowering_disposition = RuntimeLoweringDisposition::InterpreterOnly;

    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        RuntimeDiagnosticCode::FallbackDisabled
    );
    assert_eq!(report.diagnostics[0].path, "$.policy.fallback");
    assert_eq!(
        report.diagnostics[0].message,
        "native lowering is unavailable and interpreter fallback is disabled"
    );

    input.policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    let report = decide_runtime_execution(&input);
    assert_eq!(
        report.decision,
        RuntimeExecutionDecision::InterpreterFallback
    );
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        RuntimeDiagnosticCode::LoweringUnsupported
    );
    assert_eq!(report.diagnostics[0].path, "$.lowering.disposition");
}

#[test]
fn unsupported_predicate_policy_can_scan_all_or_fail_closed() {
    let mut input = native_ready_input();
    input.predicate_supported = false;

    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
    assert_eq!(report.diagnostics[0].code.as_str(), "unsupported-predicate");
    assert_eq!(
        report.diagnostics[0].code,
        RuntimeDiagnosticCode::UnsupportedPredicate
    );
    assert_eq!(report.diagnostics[0].path, "$.predicate");

    input.policy.unsupported_predicate = UnsupportedPredicatePolicy::ScanAll;
    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::NativeCandidate);
    assert_eq!(report.diagnostics[0].code.as_str(), "unsupported-predicate");
    assert_eq!(report.diagnostics[0].path, "$.predicate");
}

#[test]
fn invalid_split_remains_runtime_policy_owned() {
    let mut input = native_ready_input();
    input.split_supported = false;

    let report = decide_runtime_execution(&input);
    assert_eq!(report.decision, RuntimeExecutionDecision::FailClosed);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        RuntimeDiagnosticCode::InvalidSplit
    );
    assert_eq!(report.diagnostics[0].path, "$.split");

    let err = plan_split(
        SplitDescriptor::RowRange { start: 4, end: 4 },
        ScanShape {
            column_count: 1,
            row_count: 8,
            splittable: true,
        },
        ConcurrencyPolicy::SingleWorker,
    )
    .expect_err("empty split should fail in runtime planning");
    assert_eq!(err.code, RuntimeDiagnosticCode::InvalidSplit);
    assert_eq!(err.path, "$.split");
}
