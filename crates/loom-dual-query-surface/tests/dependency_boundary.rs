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
        "jdbc",
        "odbc",
        "reqwest",
        "aws-sdk",
        "object_store",
        "docker",
    ] {
        assert!(
            !deps.contains(marker),
            "Phase 29 adapter must not add default runtime/client dependency marker {marker}"
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
    ];
    for file in surfaces {
        let text = read_text(&file);
        for marker in &forbidden {
            assert!(
                !text.contains(marker),
                "public host surface contains forbidden marker {marker}: {}",
                file.display()
            );
        }
    }
}
