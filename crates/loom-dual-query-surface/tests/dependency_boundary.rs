use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_text(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path.as_ref())
        .unwrap_or_else(|error| panic!("read {}: {error}", path.as_ref().display()))
}

fn non_comment_code(text: &str) -> String {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*/")
            {
                None
            } else {
                Some(trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

#[test]
fn no_default_starrocks_runtime_or_client_dependency_is_present() {
    let root = workspace_root();
    let manifest = read_text(root.join("crates/loom-dual-query-surface/Cargo.toml"));
    let deps = dependency_sections(&manifest)
        .join("\n")
        .to_ascii_lowercase();
    for marker in [
        "starrocks",
        "mysql",
        "mysql_async",
        "mysqlclient",
        "jdbc",
        "odbc",
        "reqwest",
        "ureq",
        "hyper",
        "tonic",
        "aws-sdk",
        "aws_config",
        "object_store",
        "object-store",
        "docker",
        "bollard",
        "kube",
    ] {
        assert!(
            !deps.contains(marker),
            "Phase 30 adapter must not add default runtime/client/catalog dependency marker {marker}"
        );
    }
}

#[test]
fn public_host_surfaces_keep_existing_duckdb_sql_only() {
    let root = workspace_root();
    let surfaces = [
        root.join("crates/loom-ffi/include/loom.h"),
        root.join("crates/loom-ffi/include/loom_runtime.h"),
        root.join("crates/loom-ffi/include/loom_duckdb_internal.h"),
        root.join("duckdb-ext/loom_extension.cpp"),
        root.join("crates/loom-cli/src/main.rs"),
    ];
    let forbidden = [
        format!("loom_scan_{}", "iceberg"),
        format!("loom_scan_{}", "starrocks"),
        format!("loom_{}_query", "starrocks"),
        format!("{}_catalog", "starrocks"),
        format!("{}_credential", "starrocks"),
        format!("{}_external_table", "starrocks"),
        format!("{}_runtime_smoke", "starrocks"),
        format!("{}_jdbc", "starrocks"),
        format!("{}_odbc", "starrocks"),
        "CREATE EXTERNAL TABLE".to_string(),
        "aws_access_key".to_string(),
        "secret_access_key".to_string(),
    ];
    for file in surfaces {
        let text = non_comment_code(&read_text(&file));
        for marker in &forbidden {
            assert!(
                !text.contains(marker),
                "public host surface contains forbidden marker {marker}: {}",
                file.display()
            );
        }
    }
}

#[test]
fn neutral_crates_do_not_absorb_phase30_query_surface_vocabulary() {
    let root = workspace_root();
    let neutral_files = [
        root.join("crates/loom-core/src/lib.rs"),
        root.join("crates/loom-ffi/src/lib.rs"),
        root.join("crates/loom-source-ingress/src/lib.rs"),
        root.join("crates/loom-iceberg-binding/src/lib.rs"),
    ];
    let forbidden = [
        format!("{}{}", "Star", "Rocks"),
        format!("{}{}", "star", "rocks"),
        "dual-query-surface".to_string(),
        "query_surface".to_string(),
        "external table".to_string(),
        "distributed execution".to_string(),
        "predicate pushdown".to_string(),
    ];
    for file in neutral_files {
        let text = non_comment_code(&read_text(&file));
        for marker in &forbidden {
            assert!(
                !text.contains(marker),
                "neutral crate leaked Phase 30 marker {marker}: {}",
                file.display()
            );
        }
    }
}
