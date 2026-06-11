use loom_core::runtime_abi::{
    ConcurrencyPolicy, PredicateEnvelope, ProjectionColumn, ProjectionSet, RuntimeAbiVersion,
    RuntimeBackendIdentity, RuntimeCacheCompatibilityStatus, RuntimeCacheKey, RuntimeCacheKeyInput,
    RuntimeDiagnostic, RuntimeDiagnosticCode, RuntimeFallbackPolicy, RuntimeSafetyPolicy,
    SplitDescriptor, UnsupportedPredicatePolicy,
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
        verifier_identity: "loom-artifact-verifier-v1".to_string(),
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
    changed.abi_version = RuntimeAbiVersion { major: 0, minor: 2 };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

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
    changed.verifier_identity = "loom-artifact-verifier-v2".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.production_lowering_fingerprint = "lowering-b".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.backend_identity.backend = "other-backend".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.backend_identity.backend_version = "phase25-test".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.backend_identity.toolchain = "llvm-23.0.0".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.backend_identity.target_triple = "x86_64-unknown-linux-gnu".to_string();
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.backend_identity.cpu_features = vec!["sse4.2".to_string()];
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));
}

#[test]
fn cache_key_changes_when_query_shape_or_policy_change() {
    let input = key_input();
    let baseline = RuntimeCacheKey::build(&input);

    // Interpreter oracle and broad Vortex claims sit above this layer; these
    // assertions only prove cache identity observes every reuse input.
    let mut changed = input.clone();
    changed.projection = ProjectionSet::Columns(vec![ProjectionColumn {
        source_index: 0,
        output_index: 0,
    }]);
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));
    assert!(RuntimeCacheKey::build(&changed)
        .canonical_input
        .contains("projection=columns"));

    let mut changed = input.clone();
    changed.predicate = PredicateEnvelope::PrimitiveComparison {
        column_index: 0,
        op: loom_core::runtime_abi::PredicateOperator::GtEq,
        literal_i64: 7,
    };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));
    assert!(baseline.canonical_input.contains("predicate=none"));

    let mut changed = input.clone();
    changed.split = SplitDescriptor::RowRange { start: 0, end: 2 };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.policy.fallback = RuntimeFallbackPolicy::AllowInterpreter;
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.policy.unsupported_predicate = UnsupportedPredicatePolicy::ScanAll;
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input.clone();
    changed.policy.concurrency = ConcurrencyPolicy::SerializeSharedScan;
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let mut changed = input;
    changed.policy.concurrency = ConcurrencyPolicy::ParallelSplits {
        requested_workers: 4,
    };
    assert_ne!(baseline, RuntimeCacheKey::build(&changed));

    let parallel_baseline = RuntimeCacheKey::build(&changed);
    changed.policy.concurrency = ConcurrencyPolicy::ParallelSplits {
        requested_workers: 8,
    };
    assert_ne!(parallel_baseline, RuntimeCacheKey::build(&changed));
}

#[test]
fn cache_key_compatibility_reports_hit_miss_and_key_mismatch() {
    let expected = RuntimeCacheKey::build(&key_input());
    let hit = expected.compatibility_with(&expected);
    assert_eq!(hit.status, RuntimeCacheCompatibilityStatus::Hit);
    assert_eq!(hit.status.as_str(), "hit");
    assert!(hit.diagnostics.is_empty());

    let mut changed = key_input();
    changed.artifact_digest = "artifact-b".to_string();
    let miss = expected.compatibility_with(&RuntimeCacheKey::build(&changed));
    assert_eq!(miss.status, RuntimeCacheCompatibilityStatus::Miss);
    assert_eq!(miss.status.as_str(), "miss");
    assert!(miss.diagnostics.is_empty());

    let stable_id_miss = expected.compatibility_with(&RuntimeCacheKey {
        stable_id: format!("{}-miss", expected.stable_id),
        canonical_input: format!("{};tampered=true", expected.canonical_input),
    });
    assert_eq!(stable_id_miss.status, RuntimeCacheCompatibilityStatus::Miss);
    assert!(stable_id_miss.diagnostics.is_empty());

    let mismatched = RuntimeCacheKey {
        stable_id: expected.stable_id.clone(),
        canonical_input: format!("{};tampered=true", expected.canonical_input),
    };
    let mismatch = expected.compatibility_with(&mismatched);
    assert_eq!(
        mismatch.status,
        RuntimeCacheCompatibilityStatus::KeyMismatch
    );
    assert_eq!(mismatch.status.as_str(), "key-mismatch");
    assert_eq!(mismatch.diagnostics.len(), 1);
    assert_eq!(
        mismatch.diagnostics[0].code,
        RuntimeDiagnosticCode::CacheKeyMismatch
    );
    assert_eq!(mismatch.diagnostics[0].path, "$.cache.key");
    assert_eq!(
        mismatch.diagnostics[0].message,
        "runtime cache key stable id matched but canonical input differed"
    );
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
