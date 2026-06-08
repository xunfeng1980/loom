use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn manifest(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).expect("read manifest")
}

fn dependency_sections(text: &str) -> Vec<&str> {
    let mut in_dependency_section = false;
    let mut lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if matches!(trimmed, "[dependencies]" | "[dev-dependencies]" | "[build-dependencies]") {
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

#[test]
fn lance_dependency_is_direct_only_in_lance_adapter_manifest() {
    let root = workspace_root();
    let lance_name = format!("{}{}", "lan", "ce");
    let parquet_name = format!("{}{}", "par", "quet");
    let workspace_manifest = manifest(root.join("Cargo.toml"));

    assert!(direct_workspace_pin_has(&workspace_manifest, &lance_name));
    assert!(direct_workspace_pin_has(&workspace_manifest, &parquet_name));

    let mut direct_lance_manifests = Vec::new();
    let mut direct_parquet_manifests = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates dir") {
        let manifest_path = entry.expect("crate entry").path().join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }
        let text = manifest(&manifest_path);
        if direct_dep_line_has(&text, &lance_name) {
            direct_lance_manifests.push(manifest_path.clone());
        }
        if direct_dep_line_has(&text, &parquet_name) {
            direct_parquet_manifests.push(manifest_path);
        }
    }

    assert_eq!(
        direct_lance_manifests,
        vec![root.join("crates/loom-lance-ingress/Cargo.toml")]
    );
    assert_eq!(
        direct_parquet_manifests,
        vec![root.join("crates/loom-parquet-ingress/Cargo.toml")]
    );
}

#[test]
fn generic_source_ingress_contract_has_no_source_sdk_vocabulary() {
    let root = workspace_root();
    let source_files = [
        root.join("crates/loom-source-ingress/Cargo.toml"),
        root.join("crates/loom-source-ingress/src/lib.rs"),
        root.join("crates/loom-source-ingress/tests/source_ingress_contract.rs"),
    ];
    let forbidden = [
        format!("{}{}", "lan", "ce"),
        format!("{}{}", "Lan", "ce"),
        format!("{}{}", "par", "quet"),
        format!("{}{}", "Par", "quet"),
    ];

    for file in source_files {
        let text = manifest(&file);
        for marker in &forbidden {
            assert!(
                !text.contains(marker),
                "generic source-ingress file leaked source SDK marker {marker}: {}",
                file.display()
            );
        }
    }
}
