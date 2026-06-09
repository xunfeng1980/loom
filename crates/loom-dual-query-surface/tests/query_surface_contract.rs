use loom_dual_query_surface::{
    accepted_fixture_bundle, canonical_query_matrix, starrocks_descriptors,
    validate_starrocks_descriptor, QueryKind,
};
use loom_iceberg_binding::IcebergBindingStatus;

#[test]
fn accepted_fixture_bundle_is_bound_by_phase28_evidence() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    assert_eq!(
        bundle.accepted.report.status,
        IcebergBindingStatus::Accepted
    );
    assert!(!bundle.accepted.bytes.is_empty());
    assert!(bundle.artifact_path.is_file());
    let facts = bundle
        .accepted
        .report
        .facts
        .as_ref()
        .expect("accepted binding facts");
    assert_eq!(
        facts.identity.table_uuid,
        "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30"
    );
    assert_eq!(facts.identity.table_name, "demo.events");
    assert_eq!(facts.identity.schema_id, 7);
    assert_eq!(facts.identity.snapshot_id, 314159);
    assert_eq!(facts.artifact_sha256, bundle.artifact_sha256);
}

#[test]
fn canonical_query_matrix_matches_accepted_artifact_values() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let matrix = canonical_query_matrix(&bundle.accepted).expect("canonical matrix");
    let find = |kind| {
        matrix
            .iter()
            .find(|result| result.kind == kind)
            .unwrap_or_else(|| panic!("missing result {kind:?}"))
    };
    assert_eq!(find(QueryKind::OrderedRows).values, vec![-1, 7, 42]);
    assert_eq!(find(QueryKind::Projection).values, vec![-1, 7, 42]);
    assert_eq!(find(QueryKind::PredicateIdGteZero).values, vec![7, 42]);
    assert_eq!(find(QueryKind::Count).scalar, Some(3));
    assert_eq!(find(QueryKind::Sum).scalar, Some(48));
}

#[test]
fn starrocks_descriptors_preserve_binding_identity_and_query_matrix() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let descriptors = starrocks_descriptors(&bundle.accepted).expect("descriptors");
    assert_eq!(descriptors.len(), 5);
    for descriptor in &descriptors {
        validate_starrocks_descriptor(&bundle.accepted, descriptor).expect("valid descriptor");
        assert_eq!(descriptor.identity.table_name, "demo.events");
        assert_eq!(descriptor.projection, vec!["id"]);
        assert!(!descriptor.sql.contains("CREATE"));
        assert!(!descriptor.sql.contains("CATALOG"));
    }
}
