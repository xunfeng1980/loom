use loom_core::runtime_abi::{
    ProjectionColumn, ProjectionSet, RuntimeAbiVersion, RuntimeEmissionDisposition,
    RuntimeExecutionDecision, RuntimeFallbackPolicy, RuntimeHandleKind, RuntimeSafetyPolicy,
    SplitDescriptor,
};

#[test]
fn runtime_abi_strings_are_stable() {
    assert_eq!(RuntimeAbiVersion::CURRENT.as_key(), "0.1");
    assert_eq!(RuntimeHandleKind::Plan.as_str(), "plan");
    assert_eq!(RuntimeHandleKind::Scan.as_str(), "scan");
    assert_eq!(
        RuntimeExecutionDecision::NativeCandidate.as_str(),
        "native-candidate"
    );
    assert_eq!(
        RuntimeExecutionDecision::InterpreterFallback.as_str(),
        "interpreter-fallback"
    );
    assert_eq!(
        RuntimeFallbackPolicy::FailClosedOnly.as_str(),
        "fail-closed-only"
    );
    assert_eq!(
        RuntimeEmissionDisposition::SemanticArrow.as_str(),
        "semantic-arrow"
    );
}

#[test]
fn runtime_model_represents_projection_and_split_inputs() {
    let projection = ProjectionSet::Columns(vec![
        ProjectionColumn {
            source_index: 2,
            output_index: 0,
        },
        ProjectionColumn {
            source_index: 0,
            output_index: 1,
        },
    ]);
    assert_eq!(projection.as_key(), "columns:2>0,0>1");

    let split = SplitDescriptor::RowRange { start: 10, end: 20 };
    assert_eq!(split.as_key(), "range:10:20");
    assert!(!split.is_empty());
}

#[test]
fn default_runtime_policy_is_fail_closed_single_worker() {
    let policy = RuntimeSafetyPolicy::default();
    assert_eq!(policy.fallback, RuntimeFallbackPolicy::FailClosedOnly);
    assert!(!policy.fallback.allows_interpreter());
}
