use loom_dual_query_surface::{accepted_fixture_bundle, duckdb_query_cases};

#[test]
fn duckdb_evidence_uses_existing_loom_scan_and_expected_values() {
    let bundle = accepted_fixture_bundle().expect("accepted fixture bundle");
    let cases = duckdb_query_cases(&bundle.artifact_path, &bundle.accepted).expect("duckdb cases");
    assert_eq!(cases.len(), 5);
    for case in &cases {
        assert!(case.sql.contains("loom_scan("), "{}", case.sql);
        assert!(!case.sql.contains("loom_scan_iceberg"), "{}", case.sql);
        assert!(!case.sql.contains("loom_scan_starrocks"), "{}", case.sql);
        assert_eq!(
            case.identity.table_uuid,
            "9f1a03d0-61f7-4f6d-a7a4-3d8b983cbe30"
        );
    }
    let by_name = |name: &str| {
        cases
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("missing case {name}"))
    };
    assert_eq!(by_name("ordered_rows").expected_csv, "-1\n7\n42");
    assert_eq!(by_name("predicate_id_gte_zero").expected_csv, "7\n42");
    assert_eq!(by_name("count").expected_csv, "3");
    assert_eq!(by_name("sum").expected_csv, "48");
}

#[test]
fn fixture_writer_produces_accepted_artifact_and_evidence() {
    let out_dir = std::env::temp_dir().join(format!(
        "loom-dual-query-emitter-test-{}",
        std::process::id()
    ));
    let bundle = loom_dual_query_surface::write_accepted_fixture_bundle(&out_dir)
        .expect("write accepted bundle");
    assert!(bundle.artifact_path.is_file());
    assert!(bundle.sidecar_path.is_file());
    let cases = duckdb_query_cases(&bundle.artifact_path, &bundle.accepted).expect("duckdb cases");
    assert!(cases.iter().any(|case| case.expected_csv == "48"));
    let _ = std::fs::remove_dir_all(&out_dir);
}
