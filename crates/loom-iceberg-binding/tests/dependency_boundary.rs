use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_text(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path.as_ref()).unwrap_or_else(|error| {
        panic!("read {}: {error}", path.as_ref().display());
    })
}

fn dependency_sections(text: &str) -> Vec<&str> {
    let mut in_dependency_section = false;
    let mut lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if matches!(
            trimmed,
            "[dependencies]" | "[dev-dependencies]" | "[build-dependencies]"
        ) {
            in_dependency_section = true;
            continue;
        }
        if in_dependency_section && trimmed.starts_with('[') {
            in_dependency_section = false;
        }
        if in_dependency_section && !trimmed.is_empty() && !trimmed.starts_with('#') {
            lines.push(trimmed);
        }
    }

    lines
}

fn direct_dep_line_has(text: &str, name: &str) -> bool {
    dependency_sections(text).iter().any(|line| {
        line.strip_prefix(name)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
}

fn direct_workspace_pin_has(text: &str, name: &str) -> bool {
    let mut in_workspace_deps = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.dependencies]" {
            in_workspace_deps = true;
            continue;
        }
        if in_workspace_deps && trimmed.starts_with('[') {
            return false;
        }
        if in_workspace_deps
            && trimmed
                .strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        {
            return true;
        }
    }
    false
}

fn forbidden_public_markers() -> Vec<String> {
    [
        ("loom_scan_", "iceberg"),
        ("loom_ingest_", "iceberg"),
        ("iceberg_", "catalog"),
        ("iceberg_", "rest"),
        ("ware", "house"),
        ("branch ", "mutation"),
        ("tag ", "mutation"),
        ("aws_", "access_key"),
        ("secret_", "access_key"),
        ("s3_", "credentials"),
        ("credential_", "mode"),
        ("storage_", "options"),
        ("cloud_", "credentials"),
        ("Star", "Rocks"),
        ("star", "rocks"),
        ("object_", "store"),
        ("object-", "store"),
    ]
    .into_iter()
    .map(|(left, right)| format!("{left}{right}"))
    .collect()
}

#[test]
fn no_default_iceberg_sdk_dependency_is_present() {
    let root = workspace_root();
    let sdk_name = format!("{}{}", "ice", "berg");
    let workspace_manifest = read_text(root.join("Cargo.toml"));
    assert!(
        !direct_workspace_pin_has(&workspace_manifest, &sdk_name),
        "workspace must not pin the official Iceberg SDK by default"
    );

    let mut direct_sdk_manifests = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates dir") {
        let manifest_path = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        let text = read_text(&manifest_path);
        if direct_dep_line_has(&text, &sdk_name) {
            direct_sdk_manifests.push(manifest_path);
        }
    }
    assert!(
        direct_sdk_manifests.is_empty(),
        "no crate should directly depend on the official Iceberg SDK by default: {direct_sdk_manifests:?}"
    );
}

#[test]
fn serde_json_is_limited_to_workspace_pin_and_adapter_dependency() {
    let root = workspace_root();
    let json_name = format!("serde_{}", "json");
    let workspace_manifest = read_text(root.join("Cargo.toml"));
    assert!(direct_workspace_pin_has(&workspace_manifest, &json_name));

    let mut direct_json_manifests = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates dir") {
        let manifest_path = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        let text = read_text(&manifest_path);
        if direct_dep_line_has(&text, &json_name) {
            direct_json_manifests.push(manifest_path);
        }
    }
    assert_eq!(
        direct_json_manifests,
        vec![root.join("crates/loom-iceberg-binding/Cargo.toml")]
    );
}

#[test]
fn source_neutral_ingress_surfaces_do_not_leak_iceberg_vocabulary() {
    let root = workspace_root();
    let source_files = [
        root.join("crates/loom-source-ingress/Cargo.toml"),
        root.join("crates/loom-source-ingress/src/lib.rs"),
        root.join("crates/loom-source-ingress/tests/source_ingress_contract.rs"),
    ];
    let forbidden = [
        format!("{}{}", "ice", "berg"),
        format!("{}{}", "Ice", "berg"),
    ];

    for file in source_files {
        let text = read_text(&file);
        for marker in &forbidden {
            assert!(
                !text.contains(marker),
                "generic source-ingress file leaked Iceberg marker {marker}: {}",
                file.display()
            );
        }
    }
}

#[test]
fn public_host_and_cli_surfaces_have_no_iceberg_or_credential_routes() {
    let root = workspace_root();
    let surfaces = [
        root.join("crates/loom-ffi/include/loom.h"),
        root.join("crates/loom-ffi/include/loom_runtime.h"),
        root.join("crates/loom-ffi/include/loom_duckdb_internal.h"),
        root.join("duckdb-ext/loom_extension.cpp"),
        root.join("crates/loom-cli/src/main.rs"),
    ];

    for file in surfaces {
        let text = read_text(&file);
        for marker in forbidden_public_markers() {
            assert!(
                !text.contains(&marker),
                "public/host surface contains forbidden marker {marker}: {}",
                file.display()
            );
        }
    }
}

#[test]
fn focused_gate_is_wired_after_lance_parquet_and_before_duckdb_smoke() {
    let root = workspace_root();
    assert!(root.join("scripts/iceberg-binding-test.sh").is_file());
    let main_gate = read_text(root.join("scripts/mvp0-verify.sh"));
    let source_pos = main_gate
        .find("scripts/source-ingress-contract-test.sh")
        .expect("source ingress gate");
    let lance_parquet_pos = main_gate
        .find("scripts/lance-parquet-ingress-test.sh")
        .expect("Lance/Parquet gate");
    let iceberg_pos = main_gate
        .find("scripts/iceberg-binding-test.sh")
        .expect("Iceberg binding gate");
    let duckdb_pos = main_gate
        .find("scripts/duckdb-smoke-test.sh")
        .expect("DuckDB smoke gate");
    assert!(
        source_pos < lance_parquet_pos && lance_parquet_pos < iceberg_pos && iceberg_pos < duckdb_pos,
        "Phase 28 gate must run after Phase 27 and before DuckDB smoke"
    );
}
