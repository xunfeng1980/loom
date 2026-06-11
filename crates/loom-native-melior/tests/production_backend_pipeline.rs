use loom_ffi::l2_core::L2DataType;

use loom_ffi::production_native_lowering::{
    ProductionColumnShape, ProductionLoweringBackend, ProductionLoweringFacts,
    ProductionLoweringShape,
};
use loom_ffi::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity, RuntimeCacheKey,
    RuntimeCacheKeyInput, RuntimeExecutionDecision, RuntimeFallbackPolicy, RuntimePlan,
    RuntimeSafetyPolicy, SplitDescriptor,
};
use loom_native_melior::backend::{
    NativeBackendCancellation, NativeBackendDiagnosticCode, NativeBackendIdentity,
    NativeBackendRequestInput, NativeBackendStatus, NATIVE_BACKEND_NAME,
};
use loom_native_melior::pipeline::{
    prepare_production_backend_pipeline, validate_and_prepare_production_backend,
    validate_production_standard_mlir, validate_production_translation_to_llvm_ir,
    MlirValidationOptions, ProductionBackendPipelineOptions, ProductionMlirArtifact,
    LLVM_LOWERING_PIPELINE, PRODUCTION_LLVM_LOWERING_PIPELINE_ID,
    PRODUCTION_MLIR_VALIDATION_PIPELINE_ID,
};
use loom_native_melior::report::MeliorBackendDiagnosticCode;

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
        artifact_digest: "artifact-pipeline".to_string(),
        facts_fingerprint: "facts-pipeline".to_string(),
        verifier_identity: "bitwuzla-pipeline".to_string(),
        production_lowering_fingerprint: "lowering-pipeline".to_string(),
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
            columns: vec![
                ProductionColumnShape {
                    builder_id: "id".to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                },
                ProductionColumnShape {
                    builder_id: "score".to_string(),
                    arrow_type: L2DataType::Float64,
                    nullable: false,
                },
            ],
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
fn validated_request_flows_into_production_mlir_pipeline() {
    let report = validate_and_prepare_production_backend(
        request_input(),
        ProductionBackendPipelineOptions::default(),
    );

    assert!(
        matches!(
            report.status,
            NativeBackendStatus::Accepted | NativeBackendStatus::SkippedToolchain
        ),
        "unexpected report: {report:?}"
    );
    assert_eq!(
        report
            .runtime_cache_key
            .as_ref()
            .map(|key| key.stable_id.clone()),
        Some(runtime_cache_key().stable_id)
    );
    assert_eq!(
        report.backend_identity.pipeline_id,
        PRODUCTION_MLIR_VALIDATION_PIPELINE_ID
    );
    assert_eq!(
        report.backend_identity.llvm_lowering_pipeline.as_deref(),
        Some(LLVM_LOWERING_PIPELINE)
    );
    if report.status == NativeBackendStatus::Accepted {
        let artifact = report
            .artifact
            .expect("accepted report should expose artifact");
        assert_eq!(
            artifact.entry_symbol.as_deref(),
            Some("loom_decode_build_buffers")
        );
        assert_eq!(artifact.row_count, Some(4));
        assert_eq!(artifact.column_count, Some(2));
    } else {
        assert_eq!(
            report.diagnostics[0].code,
            NativeBackendDiagnosticCode::ToolchainSkipped
        );
    }
}

#[test]
fn invalid_runtime_request_rejects_before_mlir_validation() {
    let mut input = request_input();
    input.runtime_plan = runtime_plan(RuntimeExecutionDecision::InterpreterFallback);

    let report =
        validate_and_prepare_production_backend(input, ProductionBackendPipelineOptions::default());
    assert_eq!(report.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::RuntimePlanNotNativeCandidate
    );
    assert!(report.artifact.is_none());
}

#[test]
fn missing_cache_cancelled_and_unsupported_facts_fail_before_pipeline() {
    let mut input = request_input();
    input.runtime_cache_key = None;
    let report =
        validate_and_prepare_production_backend(input, ProductionBackendPipelineOptions::default());
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::MissingCacheKey
    );

    let mut input = request_input();
    input.cancellation = NativeBackendCancellation::cancelled("host cancelled");
    let report =
        validate_and_prepare_production_backend(input, ProductionBackendPipelineOptions::default());
    assert_eq!(report.status, NativeBackendStatus::Cancelled);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::Cancelled
    );

    let mut input = request_input();
    let mut facts = lowering_facts();
    // Use empty columns to trigger unsupported lowering facts,
    // since constraints_discharged no longer gates.
    facts.shape = loom_ffi::production_native_lowering::ProductionLoweringShape::PrimitiveTable {
        row_count: 4,
        columns: vec![],
    };
    input.lowering_facts = Some(facts);
    let report =
        validate_and_prepare_production_backend(input, ProductionBackendPipelineOptions::default());
    assert_eq!(report.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        report.diagnostics[0].code,
        NativeBackendDiagnosticCode::UnsupportedLoweringFacts
    );
}

#[test]
fn validated_request_can_be_prepared_directly() {
    let request =
        loom_native_melior::backend::validate_backend_request(request_input()).expect("request");
    let report =
        prepare_production_backend_pipeline(&request, ProductionBackendPipelineOptions::default());
    assert!(matches!(
        report.status,
        NativeBackendStatus::Accepted | NativeBackendStatus::SkippedToolchain
    ));
}

#[test]
fn invalid_mlir_artifact_returns_stable_pipeline_diagnostic() {
    let artifact = ProductionMlirArtifact {
        entry_symbol: "wrong_symbol".to_string(),
        mlir_text: "not valid mlir".to_string(),
        row_count: 4,
        column_count: 1,
        artifact_summary: "bad".to_string(),
    };
    let report = validate_production_standard_mlir(&artifact, MlirValidationOptions::default());
    assert!(!report.is_ok());
    assert_eq!(
        report.diagnostics[0].code,
        MeliorBackendDiagnosticCode::MlirVerificationFailed
    );
}

#[test]
fn strict_toolchain_outcome_is_reported() {
    let report = validate_and_prepare_production_backend(
        request_input(),
        ProductionBackendPipelineOptions {
            require_compatible_toolchain: true,
            validate_llvm_translation: true,
        },
    );

    if report.status == NativeBackendStatus::Accepted {
        assert!(report.backend_identity.toolchain_compatible);
        assert!(report.artifact.is_some());
    } else {
        assert_eq!(report.status, NativeBackendStatus::FailClosed);
        assert_eq!(
            report.diagnostics[0].code,
            NativeBackendDiagnosticCode::ToolchainFailed
        );
    }
}

#[test]
fn production_translation_to_llvm_ir_is_skip_aware() {
    let request =
        loom_native_melior::backend::validate_backend_request(request_input()).expect("request");
    let report = prepare_production_backend_pipeline(
        &request,
        ProductionBackendPipelineOptions {
            require_compatible_toolchain: false,
            validate_llvm_translation: true,
        },
    );

    assert!(matches!(
        report.status,
        NativeBackendStatus::Accepted | NativeBackendStatus::SkippedToolchain
    ));
    assert_eq!(
        report.backend_identity.llvm_lowering_pipeline.as_deref(),
        Some(LLVM_LOWERING_PIPELINE)
    );
    assert_eq!(
        report.backend_identity.pipeline_id,
        PRODUCTION_LLVM_LOWERING_PIPELINE_ID
    );

    let malformed = ProductionMlirArtifact {
        entry_symbol: "wrong_symbol".to_string(),
        mlir_text: "not valid mlir".to_string(),
        row_count: 4,
        column_count: 1,
        artifact_summary: "bad".to_string(),
    };
    let malformed_report =
        validate_production_translation_to_llvm_ir(&malformed, MlirValidationOptions::default());
    assert!(!malformed_report.is_ok());
}
