use loom_dual_query_surface::{
    accepted_fixture_bundle, missing_starrocks_runtime_evidence, starrocks_descriptors,
    unsupported_starrocks_runtime_evidence, validate_starrocks_runtime_output, QueryKind,
    StarRocksRuntimeStatus, UnsupportedQueryFeature,
};

fn descriptor_for(kind: QueryKind) -> loom_dual_query_surface::StarRocksQueryDescriptor {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    starrocks_descriptors(&bundle.accepted)
        .expect("descriptors")
        .into_iter()
        .find(|descriptor| descriptor.query_kind == kind)
        .unwrap_or_else(|| panic!("missing descriptor {kind:?}"))
}

#[test]
fn accepted_runtime_rows_must_match_identity_descriptor_and_values() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let ordered = starrocks_descriptors(&bundle.accepted)
        .expect("descriptors")
        .into_iter()
        .find(|descriptor| descriptor.query_kind == QueryKind::OrderedRows)
        .expect("ordered rows descriptor");

    let evidence =
        validate_starrocks_runtime_output(&bundle.accepted, &ordered, vec![-1, 7, 42], None);
    assert_eq!(evidence.status, StarRocksRuntimeStatus::Accepted);
    assert_eq!(evidence.descriptor.identity.table_name, "demo.events");
    assert_eq!(
        evidence.observed_result_digest.as_deref(),
        Some(ordered.expected_result_digest.as_str())
    );
    assert!(evidence.diagnostics.is_empty());
}

#[test]
fn accepted_runtime_scalars_must_match_count_and_sum_descriptors() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let descriptors = starrocks_descriptors(&bundle.accepted).expect("descriptors");
    let count = descriptors
        .iter()
        .find(|descriptor| descriptor.query_kind == QueryKind::Count)
        .expect("count descriptor");
    let sum = descriptors
        .iter()
        .find(|descriptor| descriptor.query_kind == QueryKind::Sum)
        .expect("sum descriptor");

    assert_eq!(
        validate_starrocks_runtime_output(&bundle.accepted, count, Vec::new(), Some(3)).status,
        StarRocksRuntimeStatus::Accepted
    );
    assert_eq!(
        validate_starrocks_runtime_output(&bundle.accepted, sum, Vec::new(), Some(48)).status,
        StarRocksRuntimeStatus::Accepted
    );
}

#[test]
fn runtime_output_mismatch_is_not_accepted() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let descriptor = starrocks_descriptors(&bundle.accepted)
        .expect("descriptors")
        .into_iter()
        .find(|descriptor| descriptor.query_kind == QueryKind::PredicateIdGteZero)
        .expect("predicate descriptor");

    let evidence =
        validate_starrocks_runtime_output(&bundle.accepted, &descriptor, vec![-1, 7, 42], None);
    assert_eq!(evidence.status, StarRocksRuntimeStatus::Mismatch);
    assert!(evidence
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("does not match")));
}

#[test]
fn descriptor_drift_fails_before_runtime_output_can_be_accepted() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let mut descriptor = starrocks_descriptors(&bundle.accepted)
        .expect("descriptors")
        .into_iter()
        .next()
        .expect("descriptor");
    descriptor.identity.snapshot_id += 1;

    let evidence =
        validate_starrocks_runtime_output(&bundle.accepted, &descriptor, vec![-1, 7, 42], None);
    assert_eq!(evidence.status, StarRocksRuntimeStatus::Rejected);
    assert!(evidence
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("identity")));
    assert!(evidence.observed_result_digest.is_none());
}

#[test]
fn missing_runtime_and_unsupported_shapes_are_explicit_non_acceptance() {
    let descriptor = descriptor_for(QueryKind::OrderedRows);

    let missing =
        missing_starrocks_runtime_evidence(&descriptor, &["STARROCKS_MYSQL", "STARROCKS_HOST"]);
    assert_eq!(missing.status, StarRocksRuntimeStatus::MissingRuntime);
    assert!(missing.observed_result_digest.is_none());
    assert!(missing
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("missing required inputs")));

    let unsupported =
        unsupported_starrocks_runtime_evidence(&descriptor, UnsupportedQueryFeature::Join);
    assert_eq!(unsupported.status, StarRocksRuntimeStatus::Unsupported);
    assert!(unsupported
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("join")));
}
