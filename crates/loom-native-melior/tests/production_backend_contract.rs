use arrow_schema::DataType;

use loom_core::production_native_lowering::{
    ProductionColumnShape, ProductionLoweringBackend, ProductionLoweringFacts,
    ProductionLoweringShape,
};
use loom_core::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity, RuntimeCacheKey,
    RuntimeCacheKeyInput, RuntimeDiagnostic, RuntimeDiagnosticCode, RuntimeExecutionDecision,
    RuntimeFallbackPolicy, RuntimePlan, RuntimeSafetyPolicy, SplitDescriptor,
};
use loom_native_melior::backend::{
    validate_backend_request, NativeBackendCancellation, NativeBackendDiagnosticCode,
    NativeBackendIdentity, NativeBackendRequestInput, NativeBackendStatus, NATIVE_BACKEND_NAME,
    PRODUCTION_BACKEND_PIPELINE_ID,
};

fn runtime_plan(decision: RuntimeExecutionDecision) -> RuntimePlan {
    RuntimePlan {
        abi_version: RuntimeAbiVersion::CURRENT,
        decision,
        projection: ProjectionSet::All,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan { row_count: 4 },
        diagnostics: Vec::new(),
    }
}

fn runtime_cache_key() -> RuntimeCacheKey {
    RuntimeCacheKey::build(&RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: "artifact-a".to_string(),
        facts_fingerprint: "facts-a".to_string(),
        verifier_identity: "bitwuzla-a".to_string(),
        production_lowering_fingerprint: "lowering-a".to_string(),
        backend_identity: RuntimeBackendIdentity {
            backend: NATIVE_BACKEND_NAME.to_string(),
            backend_version: "phase23-test".to_string(),
            toolchain: "llvm-22".to_string(),
            target_triple: "aarch64-apple-darwin".to_string(),
            cpu_features: vec!["neon".to_string()],
        },
        projection: ProjectionSet::All,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan { row_count: 4 },
        policy: RuntimeSafetyPolicy {
            fallback: RuntimeFallbackPolicy::FailClosedOnly,
            ..RuntimeSafetyPolicy::default()
        },
    })
}

fn lowering_facts() -> ProductionLoweringFacts {
    ProductionLoweringFacts {
        backend: ProductionLoweringBackend::LoomDecodeDialect,
        artifact_kind: "LMC1".to_string(),
        payload_kind: "LMT1 table".to_string(),
        constraints_discharged: false,
        shape: ProductionLoweringShape::PrimitiveTable {
            row_count: 4,
            columns: vec![ProductionColumnShape {
                builder_id: "id".to_string(),
                arrow_type: DataType::Int32,
                nullable: false,
            }],
        },
    }
}

fn request_input() -> NativeBackendRequestInput {
    NativeBackendRequestInput {
        runtime_plan: runtime_plan(RuntimeExecutionDecision::NativeCandidate),
        runtime_cache_key: Some(runtime_cache_key()),
        lowering_facts: Some(lowering_facts()),
        backend_identity: NativeBackendIdentity::preflight_only(),
        cancellation: NativeBackendCancellation::default(),
    }
}

#[test]
fn backend_identity_and_diagnostic_strings_are_stable() {
    let identity = NativeBackendIdentity::preflight_only();
    assert_eq!(identity.backend, NATIVE_BACKEND_NAME);
    assert_eq!(identity.pipeline_id, PRODUCTION_BACKEND_PIPELINE_ID);
    assert!(identity.as_key().contains("abi=0.1"));
    assert!(identity.as_key().contains("pipeline=phase23-preflight-v0"));
    assert_eq!(
        NativeBackendStatus::SkippedToolchain.as_str(),
        "skipped-toolchain"
    );
    assert_eq!(
        NativeBackendDiagnosticCode::RuntimePlanNotNativeCandidate.as_str(),
        "runtime-plan-not-native-candidate"
    );
    assert_eq!(
        NativeBackendDiagnosticCode::ToolchainFailed.as_str(),
        "toolchain-failed"
    );
}

#[test]
fn native_candidate_runtime_plan_builds_validated_request() {
    let input = request_input();
    let request = validate_backend_request(input).expect("native request should validate");
    let report = loom_native_melior::backend::NativeBackendReport::accepted(&request);

    assert!(report.is_ok());
    assert_eq!(report.status, NativeBackendStatus::Accepted);
    assert_eq!(
        report
            .runtime_cache_key
            .as_ref()
            .map(|key| key.stable_id.clone()),
        Some(request.runtime_cache_key.stable_id.clone())
    );
    assert_eq!(report.backend_identity.backend, NATIVE_BACKEND_NAME);
    assert!(report.artifact.is_some());
}

#[test]
fn interpreter_and_fail_closed_plans_reject_before_backend_work() {
    for decision in [
        RuntimeExecutionDecision::InterpreterFallback,
        RuntimeExecutionDecision::FailClosed,
        RuntimeExecutionDecision::DiagnosticOnly,
    ] {
        let mut input = request_input();
        input.runtime_plan = runtime_plan(decision);

        let report = validate_backend_request(input).expect_err("request must reject");
        assert_eq!(report.status, NativeBackendStatus::FailClosed);
        assert_eq!(
            report.diagnostics[0].code,
            NativeBackendDiagnosticCode::RuntimePlanNotNativeCandidate
        );
        assert!(report.artifact.is_none());
    }
}

#[test]
fn missing_cache_and_lowering_facts_fail_closed() {
    let mut input = request_input();
    input.runtime_cache_key = None;

    let report = validate_backend_request(input).expect_err("missing cache should reject");
    assert_eq!(report.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::MissingCacheKey
    );

    let mut input = request_input();
    input.lowering_facts = None;
    let report = validate_backend_request(input).expect_err("missing facts should reject");
    assert_eq!(report.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::MissingLoweringFacts
    );
}

#[test]
fn unsupported_lowering_facts_fail_closed() {
    let mut input = request_input();
    let mut facts = lowering_facts();
    // Use an actually unsupported shape (empty columns) to trigger the reject
    // path, since constraints_discharged no longer gates lowering facts.
    facts.shape = loom_core::production_native_lowering::ProductionLoweringShape::PrimitiveTable {
        row_count: 4,
        columns: vec![],
    };
    input.lowering_facts = Some(facts);

    let report = validate_backend_request(input).expect_err("unsupported facts should reject");
    assert_eq!(report.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::UnsupportedLoweringFacts
    );
}

#[test]
fn cancellation_is_distinct_from_runtime_and_toolchain_failure() {
    let mut input = request_input();
    input.cancellation = NativeBackendCancellation::cancelled("host interrupt");

    let report = validate_backend_request(input).expect_err("cancelled request should reject");
    assert_eq!(report.status, NativeBackendStatus::Cancelled);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::Cancelled
    );
    assert_eq!(report.diagnostics[0].message, "host interrupt");
}

#[test]
fn runtime_plan_diagnostics_reject_even_when_decision_is_native() {
    let mut input = request_input();
    input.runtime_plan.diagnostics.push(RuntimeDiagnostic::new(
        RuntimeDiagnosticCode::UnsupportedPredicate,
        "$.predicate",
        "predicate degraded",
    ));

    let report = validate_backend_request(input).expect_err("diagnostic plan should reject");
    assert_eq!(report.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::RuntimePlanHadDiagnostics
    );
}
