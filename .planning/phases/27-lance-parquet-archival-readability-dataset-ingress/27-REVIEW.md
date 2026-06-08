---
phase: 27-lance-parquet-archival-readability-dataset-ingress
reviewed: 2026-06-08T21:28:36Z
depth: standard
files_reviewed: 25
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - crates/loom-lance-ingress/Cargo.toml
  - crates/loom-lance-ingress/src/lib.rs
  - crates/loom-lance-ingress/src/source_contract.rs
  - crates/loom-lance-ingress/tests/dependency_boundary.rs
  - crates/loom-lance-ingress/tests/fixtures/legacy/MANIFEST.md
  - crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.loom
  - crates/loom-lance-ingress/tests/legacy_readability.rs
  - crates/loom-lance-ingress/tests/source_ingress_contract.rs
  - crates/loom-lance-ingress/tests/source_ingress_handoff.rs
  - crates/loom-parquet-ingress/Cargo.toml
  - crates/loom-parquet-ingress/src/lib.rs
  - crates/loom-parquet-ingress/src/source_contract.rs
  - crates/loom-parquet-ingress/tests/dependency_boundary.rs
  - crates/loom-parquet-ingress/tests/fixtures/legacy/MANIFEST.md
  - crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.loom
  - crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.parquet
  - crates/loom-parquet-ingress/tests/legacy_readability.rs
  - crates/loom-parquet-ingress/tests/source_ingress_contract.rs
  - crates/loom-parquet-ingress/tests/source_ingress_handoff.rs
  - crates/loom-source-ingress/src/lib.rs
  - crates/loom-source-ingress/tests/source_ingress_contract.rs
  - scripts/lance-parquet-ingress-test.sh
  - scripts/mvp0-verify.sh
findings:
  critical: 1
  warning: 2
  info: 0
  total: 3
status: issues_found
---

# Phase 27: Code Review Report

**Reviewed:** 2026-06-08T21:28:36Z
**Depth:** standard
**Files Reviewed:** 25
**Status:** issues_found

## Narrative Findings (AI reviewer)

### Summary

Reviewed the listed Rust adapters, shared source-ingress contract, legacy fixture manifests/artifacts, dependency-boundary tests, and verification scripts. Targeted tests pass for the current covered cases, but the review found one correctness blocker in Parquet schema admission and two guardrail weaknesses that should be fixed before relying on the phase gate.

Tests run during review:

- `cargo test -p loom-source-ingress -- --nocapture`
- `cargo test -p loom-parquet-ingress --test source_ingress_contract -- --nocapture`
- `cargo test -p loom-lance-ingress --test source_ingress_contract -- --nocapture`
- `cargo test -p loom-parquet-ingress --test source_ingress_handoff -- --nocapture`
- `cargo test -p loom-lance-ingress --test source_ingress_handoff -- --nocapture`
- `cargo test -p loom-parquet-ingress --test legacy_readability -- --nocapture`
- `cargo test -p loom-lance-ingress --test legacy_readability -- --nocapture`
- `cargo test -p loom-parquet-ingress --test dependency_boundary -- --nocapture`
- `cargo test -p loom-lance-ingress --test dependency_boundary -- --nocapture`

### Critical Issues

#### CR-01 [BLOCKER]: Parquet accepts Arrow extension fields as primitive Loom artifacts

**File:** `crates/loom-parquet-ingress/src/source_contract.rs:333`

**Issue:** The Parquet adapter accepts any non-null `Int32`/`Int64`/`Float32`/`Float64` field solely by physical Arrow type. Unlike the Lance adapter, it never checks `ARROW:extension:name` metadata when building schema facts or coverage. A Parquet file carrying an Arrow extension field over `Int32` can therefore be classified as `Accepted`, emitted as raw Loom bytes, and reported as production-lowering-supported even though extension semantics are outside the Phase 27 primitive slice. The tests cover this rejection for Lance but not for Parquet.

**Fix:**

```rust
fn field_has_extension_metadata(field: &Field) -> bool {
    field
        .metadata()
        .keys()
        .any(|key| key.eq_ignore_ascii_case("ARROW:extension:name"))
}

fn logical_kind_for_field(field: &Field) -> &'static str {
    if field_has_extension_metadata(field) {
        "extension"
    } else {
        logical_kind(field.data_type())
    }
}

let all_supported_primitives = field_count > 0
    && schema.fields().iter().all(|field| {
        !field.is_nullable()
            && !field_has_extension_metadata(field)
            && is_supported_primitive(field.data_type())
    });
```

Also update `field_schema_fact`, `unsupported_note`, `diagnostic_for_facts`, and `layout_from_batches` to reject extension metadata, then add a Parquet extension-field test mirroring the Lance case.

### Warnings

#### WR-01 [WARNING]: Parquet rejected-path detail sanitizer can leak secret-bearing or remote-looking error text

**File:** `crates/loom-parquet-ingress/src/source_contract.rs:720`

**Issue:** The Parquet sanitizer returns the first error line verbatim, while Lance drops details containing credentials, tokens, access keys, or URI-like strings and avoids attaching an empty detail. Parquet rejected reports use this function for open/read/oracle failures, so a parser or IO error containing a credential-bearing path, URL, or metadata-derived text can be exposed through `source_detail` even though rejected inputs must not expose trusted source facts.

**Fix:** Use the same redaction pattern as Lance and only attach non-empty details.

```rust
fn diagnostic_with_detail(
    code: SourceDiagnosticCode,
    path: impl Into<String>,
    message: impl Into<String>,
    detail: String,
) -> SourceDiagnostic {
    let sanitized = sanitized_detail(detail);
    if sanitized.is_empty() {
        SourceDiagnostic::new(code, path, message)
    } else {
        SourceDiagnostic::new(code, path, message).with_source_detail(sanitized)
    }
}

fn sanitized_detail(detail: String) -> String {
    let first_line = detail.lines().next().unwrap_or("Parquet adapter error").trim();
    let lowered = first_line.to_ascii_lowercase();
    if lowered.contains("credential")
        || lowered.contains("secret")
        || lowered.contains("token")
        || lowered.contains("access_key")
        || lowered.contains("://")
    {
        return String::new();
    }
    first_line.chars().take(240).collect()
}
```

#### WR-02 [WARNING]: Phase gate misses renamed source SDK dependencies on public surfaces

**File:** `scripts/lance-parquet-ingress-test.sh:116`

**Issue:** The direct dependency guard only matches dependency keys named exactly `lance` or `parquet` in `Cargo.toml` files. A public crate can add a renamed dependency such as `source_sdk = { package = "lance", ... }` without matching this check. The later cargo-tree guard only runs for `loom-core`, `loom-ffi`, and `loom-source-ingress`, so public surfaces like `loom-cli` can still gain a source SDK dependency while the closeout gate passes.

**Fix:** Scan package-renamed dependency declarations and extend dependency-tree checks to all public crates that must stay source-SDK-free.

```bash
refs="$(
    rg -n '^[[:space:]]*([A-Za-z0-9_-]+[[:space:]]*=.*package[[:space:]]*=[[:space:]]*"(lance|parquet)"|(lance|parquet)[[:space:]]*=)' \
        Cargo.toml crates/*/Cargo.toml || true
)"

check_cargo_tree_clean loom-core "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-ffi "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-source-ingress "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-cli "${source_dep_patterns[@]}"
```

---

_Reviewed: 2026-06-08T21:28:36Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
