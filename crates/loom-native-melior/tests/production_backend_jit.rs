use arrow_schema::DataType;

use loom_core::production_native_lowering::{
    ProductionColumnShape, ProductionLoweringBackend, ProductionLoweringFacts,
    ProductionLoweringShape,
};
use loom_core::runtime_abi::{
    PredicateEnvelope, ProjectionSet, RuntimeAbiVersion, RuntimeBackendIdentity, RuntimeCacheKey,
    RuntimeCacheKeyInput, RuntimeExecutionDecision, RuntimeFallbackPolicy, RuntimePlan,
    RuntimeSafetyPolicy, SplitDescriptor,
};
use loom_native_melior::backend::{
    validate_backend_request, NativeBackendCancellation, NativeBackendDiagnosticCode,
    NativeBackendIdentity, NativeBackendReport, NativeBackendRequestInput, NativeBackendStatus,
    NATIVE_BACKEND_NAME,
};
use loom_native_melior::jit::{
    compare_production_jit_output, execute_prepared_production_jit, ProductionJitOptions,
    PRODUCTION_JIT_ENTRY_SYMBOL,
};
use loom_native_melior::pipeline::{
    prepare_production_backend_pipeline, ProductionBackendPipelineOptions,
};

fn runtime_plan() -> RuntimePlan {
    RuntimePlan {
        abi_version: RuntimeAbiVersion::CURRENT,
        decision: RuntimeExecutionDecision::NativeCandidate,
        projection: ProjectionSet::All,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan { row_count: 4 },
        diagnostics: Vec::new(),
    }
}

fn runtime_cache_key() -> RuntimeCacheKey {
    RuntimeCacheKey::build(&RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: "artifact-jit".to_string(),
        facts_fingerprint: "facts-jit".to_string(),
        verifier_identity: "bitwuzla-jit".to_string(),
        production_lowering_fingerprint: "lowering-jit".to_string(),
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

fn lowering_facts(data_type: DataType) -> ProductionLoweringFacts {
    ProductionLoweringFacts {
        backend: ProductionLoweringBackend::LoomDecodeDialect,
        artifact_kind: "LMC1".to_string(),
        payload_kind: "LMT1 table".to_string(),
        constraints_discharged: false,
        shape: ProductionLoweringShape::PrimitiveTable {
            row_count: 4,
            columns: vec![ProductionColumnShape {
                builder_id: "id".to_string(),
                arrow_type: data_type,
                nullable: false,
            }],
        },
    }
}

fn nullable_lowering_facts(data_type: DataType) -> ProductionLoweringFacts {
    ProductionLoweringFacts {
        backend: ProductionLoweringBackend::LoomDecodeDialect,
        artifact_kind: "LMC1".to_string(),
        payload_kind: "LMT1 table".to_string(),
        constraints_discharged: false,
        shape: ProductionLoweringShape::PrimitiveTable {
            row_count: 4,
            columns: vec![ProductionColumnShape {
                builder_id: "nullable".to_string(),
                arrow_type: data_type,
                nullable: true,
            }],
        },
    }
}

fn request_input() -> NativeBackendRequestInput {
    NativeBackendRequestInput {
        runtime_plan: runtime_plan(),
        runtime_cache_key: Some(runtime_cache_key()),
        lowering_facts: Some(lowering_facts(DataType::Int32)),
        backend_identity: NativeBackendIdentity::preflight_only(),
        cancellation: NativeBackendCancellation::default(),
    }
}

fn accepted_backend_report() -> NativeBackendReport {
    let request = validate_backend_request(request_input()).expect("request should validate");
    let report =
        prepare_production_backend_pipeline(&request, ProductionBackendPipelineOptions::default());
    if report.status == NativeBackendStatus::Accepted {
        return report;
    }

    NativeBackendReport::accepted_pipeline(
        &request,
        request.backend_identity.clone(),
        PRODUCTION_JIT_ENTRY_SYMBOL,
        4,
        1,
        "test accepted pipeline artifact",
    )
}

fn i32_value_buffer(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn jit_options_with_i32(values: &[i32]) -> ProductionJitOptions {
    ProductionJitOptions {
        require_compatible_toolchain: true,
        input_value_buffers: vec![i32_value_buffer(values)],
    }
}

#[test]
fn production_jit_runs_only_from_accepted_backend_artifact() {
    let report = accepted_backend_report();
    let expected = i32_value_buffer(&[10, 20, 30, 40]);
    let output = execute_prepared_production_jit(
        &report,
        &NativeBackendCancellation::default(),
        jit_options_with_i32(&[10, 20, 30, 40]),
    );

    match output {
        Ok(output) => {
            assert_eq!(output.entry_symbol, PRODUCTION_JIT_ENTRY_SYMBOL);
            assert_eq!(output.row_count, 4);
            assert_eq!(output.column_count, 1);
            assert_eq!(output.value_buffers, vec![expected]);
        }
        Err(err) => {
            assert_eq!(err.status, NativeBackendStatus::FailClosed);
            assert!(matches!(
                err.diagnostics[0].code,
                NativeBackendDiagnosticCode::ToolchainFailed
                    | NativeBackendDiagnosticCode::JitUnavailable
                    | NativeBackendDiagnosticCode::JitSymbolMissing
            ));
        }
    }
}

#[test]
fn preflight_report_and_missing_symbol_do_not_execute() {
    let request = validate_backend_request(request_input()).expect("request should validate");
    let preflight = NativeBackendReport::accepted(&request);
    let err = execute_prepared_production_jit(
        &preflight,
        &NativeBackendCancellation::default(),
        ProductionJitOptions::default(),
    )
    .expect_err("preflight-only artifact has no JIT entry symbol");
    assert_eq!(err.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        err.diagnostics[0].code,
        NativeBackendDiagnosticCode::JitSymbolMissing
    );
    assert!(err.diagnostics[0]
        .message
        .contains(PRODUCTION_JIT_ENTRY_SYMBOL));
}

#[test]
fn cancellation_stops_before_jit_preparation() {
    let report = accepted_backend_report();
    let err = execute_prepared_production_jit(
        &report,
        &NativeBackendCancellation::cancelled("interrupt"),
        ProductionJitOptions::default(),
    )
    .expect_err("cancelled request should reject");
    assert_eq!(err.status, NativeBackendStatus::Cancelled);
    assert_eq!(
        err.diagnostics[0].code,
        NativeBackendDiagnosticCode::Cancelled
    );
    assert!(err.artifact.is_none());
}

#[test]
fn unsupported_primitive_shape_does_not_jit() {
    let mut input = request_input();
    input.lowering_facts = Some(lowering_facts(DataType::Utf8));
    let request = validate_backend_request(input).expect("preflight only checks runtime facts");
    let report = NativeBackendReport::accepted_pipeline(
        &request,
        request.backend_identity.clone(),
        PRODUCTION_JIT_ENTRY_SYMBOL,
        4,
        1,
        "unsupported utf8 artifact",
    );

    let err = execute_prepared_production_jit(
        &report,
        &NativeBackendCancellation::default(),
        ProductionJitOptions::default(),
    )
    .expect_err("unsupported primitive shape should reject");
    assert_eq!(err.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        err.diagnostics[0].code,
        NativeBackendDiagnosticCode::InvalidBackendArtifact
    );
}

#[test]
fn nullable_primitive_shape_does_not_jit() {
    let mut input = request_input();
    input.lowering_facts = Some(nullable_lowering_facts(DataType::Int32));
    let request = validate_backend_request(input).expect("preflight only checks runtime facts");
    let report = NativeBackendReport::accepted_pipeline(
        &request,
        request.backend_identity.clone(),
        PRODUCTION_JIT_ENTRY_SYMBOL,
        4,
        1,
        "nullable primitive artifact",
    );

    let err = execute_prepared_production_jit(
        &report,
        &NativeBackendCancellation::default(),
        ProductionJitOptions::default(),
    )
    .expect_err("nullable primitive shape should reject");
    assert_eq!(err.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        err.diagnostics[0].code,
        NativeBackendDiagnosticCode::InvalidBackendArtifact
    );
    assert_eq!(
        err.diagnostics[0].path,
        "$.backend_report.artifact.lowering_facts"
    );
    assert!(err.artifact.is_none());
}

#[test]
fn invalid_and_malformed_backend_artifacts_do_not_execute() {
    let request = validate_backend_request(request_input()).expect("request should validate");
    let missing_artifact = NativeBackendReport {
        status: NativeBackendStatus::Accepted,
        diagnostics: Vec::new(),
        runtime_plan: request.runtime_plan.clone(),
        runtime_cache_key: Some(request.runtime_cache_key.clone()),
        backend_identity: request.backend_identity.clone(),
        artifact: None,
    };
    let err = execute_prepared_production_jit(
        &missing_artifact,
        &NativeBackendCancellation::default(),
        ProductionJitOptions::default(),
    )
    .expect_err("accepted report without artifact should reject");
    assert_eq!(err.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        err.diagnostics[0].code,
        NativeBackendDiagnosticCode::InvalidBackendArtifact
    );
    assert!(err.artifact.is_none());

    let mut malformed = NativeBackendReport::accepted_pipeline(
        &request,
        request.backend_identity.clone(),
        PRODUCTION_JIT_ENTRY_SYMBOL,
        4,
        0,
        "malformed empty-column artifact",
    );
    malformed
        .artifact
        .as_mut()
        .expect("artifact")
        .lowering_facts
        .shape = ProductionLoweringShape::PrimitiveTable {
        row_count: 4,
        columns: Vec::new(),
    };
    let err = execute_prepared_production_jit(
        &malformed,
        &NativeBackendCancellation::default(),
        ProductionJitOptions::default(),
    )
    .expect_err("empty-column artifact should reject");
    assert_eq!(err.status, NativeBackendStatus::FailClosed);
    assert_eq!(
        err.diagnostics[0].code,
        NativeBackendDiagnosticCode::InvalidBackendArtifact
    );
    assert!(err.diagnostics[0]
        .message
        .contains("at least one supported primitive column"));
    assert!(err.artifact.is_none());
}

#[test]
fn production_jit_output_matches_reference_buffers_or_reports_mismatch() {
    let report = accepted_backend_report();
    let expected = i32_value_buffer(&[10, 20, 30, 40]);
    let output = execute_prepared_production_jit(
        &report,
        &NativeBackendCancellation::default(),
        jit_options_with_i32(&[10, 20, 30, 40]),
    );

    let Ok(output) = output else {
        return;
    };

    compare_production_jit_output(&report, &[expected], &output)
        .expect("JIT output should match reference");
    let mismatch = compare_production_jit_output(&report, &[vec![1u8; 16]], &output)
        .expect_err("mismatch should produce diagnostic");
    assert_eq!(
        mismatch.diagnostics[0].code,
        NativeBackendDiagnosticCode::NativeOutputMismatch
    );
}

#[test]
fn strict_missing_toolchain_is_fail_closed_unless_execution_succeeds() {
    let report = accepted_backend_report();
    let result = execute_prepared_production_jit(
        &report,
        &NativeBackendCancellation::default(),
        ProductionJitOptions {
            require_compatible_toolchain: true,
            input_value_buffers: vec![i32_value_buffer(&[10, 20, 30, 40])],
        },
    );

    if let Err(err) = result {
        assert_eq!(err.status, NativeBackendStatus::FailClosed);
        assert!(
            matches!(
                err.diagnostics[0].code,
                NativeBackendDiagnosticCode::ToolchainFailed
                    | NativeBackendDiagnosticCode::JitUnavailable
            ),
            "unexpected diagnostic: {:?}",
            err.diagnostics
        );
    }
}
