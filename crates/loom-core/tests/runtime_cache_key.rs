use loom_core::runtime_abi::{
    ConcurrencyPolicy, PredicateEnvelope, ProjectionColumn, ProjectionSet, RuntimeAbiVersion,
    RuntimeBackendIdentity, RuntimeCacheKey, RuntimeCacheKeyInput, RuntimeDiagnostic,
    RuntimeDiagnosticCode, RuntimeFallbackPolicy, RuntimeSafetyPolicy, SplitDescriptor,
    UnsupportedPredicatePolicy,
};

fn backend_identity() -> RuntimeBackendIdentity {
    RuntimeBackendIdentity {
        backend: "loom-decode-dialect".to_string(),
        backend_version: "phase22-test".to_string(),
        toolchain: "llvm-22.1.7".to_string(),
        target_triple: "aarch64-apple-darwin".to_string(),
        cpu_features: vec!["neon".to_string()],
    }
}

fn key_input() -> RuntimeCacheKeyInput {
    RuntimeCacheKeyInput {
        abi_version: RuntimeAbiVersion::CURRENT,
        artifact_digest: "artifact-a".to_string(),
        facts_fingerprint: "facts-a".to_string(),
        solver_identity: "bitwuzla-script-a".to_string(),
        production_lowering_fingerprint: "lowering-a".to_string(),
        backend_identity: backend_identity(),
        projection: ProjectionSet::All,
        predicate: PredicateEnvelope::None,
        split: SplitDescriptor::FullScan { row_count: 4 },
        policy: RuntimeSafetyPolicy::default(),
    }
}

#[test]
fn cache_key_is_deterministic() {
    let input = key_input();
    let first = RuntimeCacheKey::build(&input);
    let second = RuntimeCacheKey::build(&input);
    assert_eq!(first, second);
    assert!(first.stable_id.starts_with("loom-runtime-v0.1-"));
}

#[test]
fn cache_key_changes_when_artifact_or_facts_change() {
    let input = key_input();
    let baseline = RuntimeCacheKey::build(&input);

    let mut changed = input.clone();
    changed.artifact_digest = "artifact-b".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.facts_fingerprint = "facts-b".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));
}

#[test]
fn cache_key_changes_when_solver_backend_or_toolchain_change() {
    let input = key_input();
    let baseline = RuntimeCacheKey::build(&input);

    let mut changed = input.clone();
    changed.solver_identity = "bitwuzla-script-b".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.backend_identity.toolchain = "llvm-23.0.0".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));
}

#[test]
fn cache_key_changes_when_query_shape_or_policy_change() {
    let input = key_input();
    let baseline = RuntimeCacheKey::build(&input);

    let mut changed = input.clone();
    changed.projection = ProjectionSet::Columns(vec![ProjectionColumn {
        source_index: 0,
        output_index: 0,
    }]);
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.predicate = PredicateEnvelope::Unsupported {
        reason: "test".to_string(),
    };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.split = SplitDescriptor::RowRange { start: 0, end: 2 };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input;
    changed.policy = RuntimeSafetyPolicy {
        fallback: RuntimeFallbackPolicy::AllowInterpreter,
        unsupported_predicate: UnsupportedPredicatePolicy::ScanAll,
        concurrency: ConcurrencyPolicy::SerializeSharedScan,
    };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));
}

#[test]
fn runtime_diagnostic_codes_are_stable() {
    let diagnostic = RuntimeDiagnostic::new(
        RuntimeDiagnosticCode::CacheKeyMismatch,
        "$.cache.key",
        "cache key mismatch",
    );
    assert_eq!(diagnostic.code.as_str(), "cache-key-mismatch");
    assert_eq!(RuntimeDiagnosticCode::AbiMismatch.as_str(), "abi-mismatch");
    assert_eq!(
        RuntimeDiagnosticCode::ToolchainMismatch.as_str(),
        "toolchain-mismatch"
    );
}
