use loom_dual_query_surface::{
    accepted_fixture_bundle, plan_unsupported_query_feature, starrocks_descriptors,
    validate_starrocks_descriptor, QuerySurfaceStatus, UnsupportedQueryFeature,
};
use loom_iceberg_binding::{bind_iceberg_ref_from_paths, IcebergBindingStatus};

fn assert_no_accepted_descriptor<T>(
    result: Result<T, loom_dual_query_surface::DualQuerySurfaceDiagnostic>,
    expected: &str,
) {
    match result {
        Ok(_) => panic!("negative query-surface case unexpectedly returned accepted evidence"),
        Err(diagnostic) => {
            assert_ne!(diagnostic.code, "accepted");
            assert!(
                diagnostic.message.contains(expected),
                "expected diagnostic containing {expected:?}, got {:?}",
                diagnostic
            );
        }
    }
}

fn assert_no_accepted_binding<T>(
    result: Result<T, loom_iceberg_binding::IcebergBindingReport>,
    expected: &str,
) {
    match result {
        Ok(_) => panic!("negative binding case unexpectedly returned accepted bytes"),
        Err(report) => {
            assert_ne!(report.status, IcebergBindingStatus::Accepted);
            assert!(report.evidence.is_none());
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.contains(expected)),
                "expected diagnostic containing {expected:?}, got {:?}",
                report.diagnostics
            );
        }
    }
}

#[test]
fn descriptor_identity_and_result_drift_fail_closed() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let descriptor = starrocks_descriptors(&bundle.accepted)
        .expect("descriptors")
        .into_iter()
        .next()
        .expect("descriptor");

    let mut mutated = descriptor.clone();
    mutated.identity.table_uuid = "00000000-0000-0000-0000-000000000000".to_string();
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "identity",
    );

    let mut mutated = descriptor.clone();
    mutated.identity.schema_id += 1;
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "identity",
    );

    let mut mutated = descriptor.clone();
    mutated.identity.snapshot_id += 1;
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "identity",
    );

    let mut mutated = descriptor.clone();
    mutated.identity.artifact_sha256 =
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "identity",
    );

    let mut mutated = descriptor.clone();
    mutated.identity.row_count += 1;
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "identity",
    );

    let mut mutated = descriptor.clone();
    mutated.projection = vec!["id".to_string(), "extra".to_string()];
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "projection",
    );

    let mut mutated = descriptor.clone();
    mutated.expected_result_digest = "fnv1a64:0000000000000000".to_string();
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "expected result evidence",
    );

    let mut mutated = descriptor;
    mutated.status = QuerySurfaceStatus::Rejected;
    assert_no_accepted_descriptor(
        validate_starrocks_descriptor(&bundle.accepted, &mutated),
        "not in accepted state",
    );
}

#[test]
fn phase29_binding_drift_returns_no_query_surface_root() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");

    let mut sidecar = std::fs::read_to_string(&bundle.sidecar_path).expect("sidecar");
    sidecar = sidecar.replace(r#""schema_id": 7"#, r#""schema_id": 8"#);
    let schema_sidecar = bundle.root_dir.join("schema-mismatch-sidecar.json");
    std::fs::write(&schema_sidecar, sidecar).expect("write schema sidecar");
    assert_no_accepted_binding(
        bind_iceberg_ref_from_paths(
            &bundle.metadata_path,
            &schema_sidecar,
            &bundle.artifact_path,
        ),
        "identity",
    );

    let mut sidecar = std::fs::read_to_string(&bundle.sidecar_path).expect("sidecar");
    sidecar = sidecar.replace(
        &bundle.artifact_sha256,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    );
    let hash_sidecar = bundle.root_dir.join("hash-mismatch-sidecar.json");
    std::fs::write(&hash_sidecar, sidecar).expect("write hash sidecar");
    assert_no_accepted_binding(
        bind_iceberg_ref_from_paths(&bundle.metadata_path, &hash_sidecar, &bundle.artifact_path),
        "SHA-256",
    );

    let sidecar = std::fs::read_to_string(&bundle.sidecar_path)
        .expect("sidecar")
        .replace(
            r#",
  "source_oracle_evidence_path": "accepted-table-source-evidence.json""#,
            "",
        );
    let manifest_only = bundle.root_dir.join("manifest-only-sidecar.json");
    std::fs::write(&manifest_only, sidecar).expect("write manifest-only sidecar");
    assert_no_accepted_binding(
        bind_iceberg_ref_from_paths(&bundle.metadata_path, &manifest_only, &bundle.artifact_path),
        "source/oracle evidence",
    );

    let evidence = std::fs::read_to_string(&bundle.evidence_path)
        .expect("evidence")
        .replace(r#""row_count": 3"#, r#""row_count": 4"#);
    let stale_evidence = bundle.root_dir.join("stale-row-count-evidence.json");
    std::fs::write(&stale_evidence, evidence).expect("write stale evidence");
    let sidecar = std::fs::read_to_string(&bundle.sidecar_path)
        .expect("sidecar")
        .replace(
            "accepted-table-source-evidence.json",
            "stale-row-count-evidence.json",
        );
    let stale_sidecar = bundle.root_dir.join("stale-row-count-sidecar.json");
    std::fs::write(&stale_sidecar, sidecar).expect("write stale sidecar");
    assert_no_accepted_binding(
        bind_iceberg_ref_from_paths(&bundle.metadata_path, &stale_sidecar, &bundle.artifact_path),
        "row count",
    );

    let evidence = std::fs::read_to_string(&bundle.evidence_path)
        .expect("evidence")
        .replace(
            "82b7236a02334902a5e27c157bcc767f1451246e11959dc13f5c56e028da8d58",
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
    let forged_evidence = bundle.root_dir.join("forged-oracle-evidence.json");
    std::fs::write(&forged_evidence, evidence).expect("write forged evidence");
    let sidecar = std::fs::read_to_string(&bundle.sidecar_path)
        .expect("sidecar")
        .replace(
            "accepted-table-source-evidence.json",
            "forged-oracle-evidence.json",
        );
    let forged_sidecar = bundle.root_dir.join("forged-oracle-sidecar.json");
    std::fs::write(&forged_sidecar, sidecar).expect("write forged sidecar");
    assert_no_accepted_binding(
        bind_iceberg_ref_from_paths(
            &bundle.metadata_path,
            &forged_sidecar,
            &bundle.artifact_path,
        ),
        "values SHA-256",
    );
}

#[test]
fn unsupported_query_features_are_typed_and_non_accepting() {
    for feature in [
        UnsupportedQueryFeature::Join,
        UnsupportedQueryFeature::FreeformSql,
        UnsupportedQueryFeature::ExternalTableDdl,
        UnsupportedQueryFeature::RemoteCatalog,
        UnsupportedQueryFeature::Credentials,
        UnsupportedQueryFeature::NestedField,
        UnsupportedQueryFeature::NullableExpansion,
        UnsupportedQueryFeature::DistributedExecution,
        UnsupportedQueryFeature::PredicatePushdown,
    ] {
        let result = plan_unsupported_query_feature(feature);
        assert_no_accepted_descriptor(result, feature.as_str());
    }
}
