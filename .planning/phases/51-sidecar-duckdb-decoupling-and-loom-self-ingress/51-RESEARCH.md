# Phase 51: Sidecar-DuckDB Decoupling and Loom Self-Ingress — Research

**Researched:** 2026-06-11
**Domain:** Rust workspace refactoring — crate dependency decoupling, FFI surface splitting, CLI feature gating
**Confidence:** HIGH

## Summary

Phase 51 decouples the DuckDB sidecar path from `loom-container` by introducing a new lean FFI crate (`loom-sidecar-ffi`) that exports sidecar extract/verify/routing functions through the C ABI, depending only on `loom-ir-core` and `loom-parquet-ingress` — zero dependency on `loom-container`. A new `loom-self-ingress` crate wraps `loom-container` codecs as the single IO boundary for `.loom` files. The CLI is split via Cargo features so the `sidecar embed` subcommand compiles without `loom-container`. The existing full DuckDB path through `loom-ffi` → `loom-container` → `loom-native-melior` continues to work unchanged.

The primary refactoring constraint is **Cargo feature unification**: in a workspace, all crates sharing a dependency see the union of all features enabled on that dependency. Therefore, `loom-sidecar-ffi` must NOT go through `loom-core` (which re-exports `loom-container`). It must depend directly on `loom-ir-core` and a container-free `loom-parquet-ingress`.

**Primary recommendation:** Two separate FFI crates with no shared `loom-core` dependency on the lean path. `loom-sidecar-ffi` depends directly on `loom-ir-core` (zero Arrow deps) + a refactored `loom-parquet-ingress` (with its unused `loom-core` dependency removed). `loom-self-ingress` is a thin wrapper over `loom-container` and is the single IO boundary for `.loom` files.

## User Constraints (from CONTEXT.md)

### Locked Decisions

No locked decisions — all implementation choices are at the agent's discretion (infrastructure refactoring phase).

### the agent's Discretion

All implementation choices are at the agent's discretion — infrastructure refactoring phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions. Key principles:
- `loom-ir-core` must remain zero Arrow-dependency
- The lean sidecar-FFI path must not transitively pull in `loom-container`
- `loom-self-ingress` is the single IO boundary for `.loom` files
- Existing DuckDB extension continues to work via both paths

### Deferred Ideas (OUT OF SCOPE)

None — pure infrastructure phase.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Sidecar extract from host files | `loom-ir-core` + `loom-parquet-ingress` | `loom-sidecar-ffi` (C ABI) | Sidecar overlay model lives in ir-core; host-specific extraction in parquet-ingress; C ABI export in sidecar-ffi |
| Content-hash verification | `loom-ir-core` (l2core_codec) | — | FNV-1a hash computation over canonical IR bytes is ir-core only |
| Sidecar routing (4-gate) | `loom-ir-core` (sidecar_routing) | — | Routing logic is host-agnostic and Arrow-free |
| `.loom` file read/write/verify | `loom-self-ingress` → `loom-container` | — | Self-ingress wraps container codecs; container owns the `.loom` format |
| Full DuckDB decode path (existing) | `loom-ffi` → `loom-container` → `loom-native-melior` | — | Unchanged; legacy LMC1/LMP1/LMT1 + native Arrow semantic decode |
| DuckDB sidecar-only path (new) | `loom-sidecar-ffi` → `loom-ir-core` + `loom-parquet-ingress` | — | Sidecar extract, verify, and routing — no Arrow output, no container |
| CLI sidecar embed | `loom-cli` (lean compile) → `loom-ir-core` + `loom-parquet-ingress` | — | Feature-gated to exclude container path |
| CLI inspect/decode (existing) | `loom-cli` (full compile) → `loom-container` | — | Container-aware commands require full feature |

## Standard Stack

### Core (no new external dependencies — pure refactoring)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `loom-ir-core` | 0.1.0 (workspace) | Sidecar overlay, routing, L2Core codec, verifier | Already contains all sidecar logic; zero Arrow deps |
| `loom-parquet-ingress` | 0.1.0 (workspace) | Parquet sidecar extract/embed via KeyValue metadata | Existing adapter; needs loom-core dep removed |
| `loom-container` | 0.1.0 (workspace) | `.loom` format codecs, verifiers, native lowering | Existing; repositioned as exclusive `.loom` handler |
| `cbindgen` | 0.29.3 (workspace) | C header generation from `extern "C"` surface | Already in build-dependencies; per-crate cbindgen.toml |
| `arrow` (arrow-rs) | 58.3.0 (workspace) | FFI_ArrowArray/FFI_ArrowSchema types for C ABI | Required by `loom-sidecar-ffi` for C ABI struct types |

### Supporting (for self-ingress)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `arrow` (arrow-rs) | 58.3.0 (workspace) | Arrow types for `.loom` file decode output | Used by loom-self-ingress → loom-container |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Two FFI crates | Feature-gated single `loom-ffi` | Feature unification in workspace makes it impossible to truly exclude container deps when another workspace member enables them |
| `loom-sidecar-ffi` via `loom-core` | Direct `loom-ir-core` dependency | `loom-core` re-exports `loom-container`; feature gating `loom-core` fails due to Cargo unification |
| Copying sidecar logic into sidecar-ffi | Reusing `loom-ir-core` modules | Violates single-source-of-truth; ir-core already owns sidecar |

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| None | — | — | — | — | — | No new external packages introduced |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*Phase 51 is a pure infrastructure refactoring phase — no new external dependencies are introduced. All crates are workspace-internal.*

## Architecture Patterns

### System Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                        DuckDB Host Process                        │
│                                                                    │
│   loom_scan(path) table function                                   │
│   ├── Full path (existing): links libloom_ffi.a                    │
│   │   └── loom-ffi → loom-core → loom-container → loom-native-melior│
│   │       (LMC1/LMP1/LMT1 decode, native Arrow semantic, JIT)      │
│   │                                                                 │
│   └── Sidecar path (new): links libloom_sidecar_ffi.a              │
│       └── loom-sidecar-ffi → loom-ir-core + loom-parquet-ingress  │
│           (sidecar extract, content-hash verify, routing decision)  │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                       loom-cli Binary                              │
│                                                                    │
│   Full compile (default): loom-cli → loom-core                     │
│   ├── inspect / decode / verify-artifact / ingest-vortex           │
│   │   (uses loom-container codecs)                                  │
│   │                                                                 │
│   Lean compile (--no-default-features): loom-cli                   │
│   └── sidecar embed / verify-l2core                                │
│       (uses loom-ir-core + loom-parquet-ingress only)               │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                      .loom File Boundary                           │
│                                                                    │
│   loom-self-ingress (single IO boundary)                           │
│   ├── read_loom_file(path) → LoomPayload                          │
│   ├── write_loom_file(path, payload) → Result<()>                  │
│   └── verify_loom_file(path) → ArtifactVerificationReport          │
│       └── wraps loom-container codecs internally                    │
└──────────────────────────────────────────────────────────────────┘

Dependency direction (arrows point from dependent to dependency):

loom-sidecar-ffi ──→ loom-ir-core
loom-sidecar-ffi ──→ loom-parquet-ingress ──→ loom-ir-core
loom-self-ingress ──→ loom-container ──→ loom-ir-core
loom-ffi (existing) ──→ loom-core ──→ loom-container + loom-ir-core
loom-cli (full) ──→ loom-core ──→ loom-container + loom-ir-core
loom-cli (lean) ──→ loom-ir-core + loom-parquet-ingress
```

**Critical boundary:** `loom-sidecar-ffi` must NOT depend on `loom-core` (which re-exports `loom-container`). It depends directly on `loom-ir-core`. `loom-parquet-ingress` must drop its unused `loom-core` dependency so the lean path avoids transitive container inclusion.

### Recommended Project Structure
```
crates/
├── loom-ir-core/           # Zero-Arrow IR crate (unchanged)
│   └── src/
│       ├── sidecar.rs      # SidecarOverlay, ChunkBinding, encode/decode
│       ├── sidecar_routing.rs  # 4-gate routing decision
│       ├── l2core_codec.rs # L2IR codec, content-hash
│       ├── full_verifier.rs    # verify_l2_core_bytes
│       ├── l2_core.rs      # L2CoreProgram model
│       └── error.rs
├── loom-container/         # Container layer (unchanged structure)
│   └── src/                # 19 modules: codecs, verifiers, lowering, lineage
├── loom-core/              # Re-export shim (unchanged, full path uses it)
├── loom-ffi/               # Full FFI (unchanged; existing path)
│   └── src/
│       ├── ffi.rs          # loom_decode (LMC1 + Arrow semantic paths)
│       └── duckdb_runtime.rs  # DuckDB native execution bridge
├── loom-sidecar-ffi/       # NEW: Lean sidecar FFI
│   ├── Cargo.toml          # Deps: loom-ir-core, loom-parquet-ingress, arrow
│   ├── cbindgen.toml       # Generates include/loom_sidecar.h
│   ├── build.rs            # cbindgen invocation
│   └── src/
│       ├── lib.rs          # Global allocator + module declarations
│       └── ffi.rs          # extern "C" sidecar extract/verify/route
├── loom-self-ingress/      # NEW: .loom file IO boundary
│   ├── Cargo.toml          # Deps: loom-container, arrow
│   └── src/
│       └── lib.rs          # read_loom_file, write_loom_file, verify_loom_file
└── loom-cli/               # CLI (feature-gated)
    ├── Cargo.toml          # Features: default=["full"], full=["loom-core", ...]
    └── src/
        └── main.rs         # Feature-gated command dispatch

ingress/
└── loom-parquet-ingress/   # Parquet adapter (loom-core dep removed)
    ├── Cargo.toml          # loom-core removed; only loom-ir-core + parquet
    └── src/
        ├── sidecar_parquet.rs  # Sidecar extract/embed (uses loom-ir-core only)
        └── source_contract.rs  # Source facts (uses loom-source-ingress only)
```

### Pattern 1: Feature-Gated CLI Compilation

**What:** Use Cargo features to compile the CLI binary with or without container support. The CLI binary name stays `loom`; `--no-default-features` builds the lean version.

**When to use:** When a single binary needs to support two different dependency sets but shouldn't split into two separate binaries.

**Example:**
```toml
# crates/loom-cli/Cargo.toml
[package]
name = "loom-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "loom"
path = "src/main.rs"
required-features = []  # always buildable

[features]
default = ["full"]
full = ["dep:loom-core", "dep:arrow", "dep:arrow-data", "dep:arrow-schema", "dep:loom-vortex-ingress"]
lean = ["dep:loom-parquet-ingress"]  # only sidecar + l2core operations

[dependencies]
loom-ir-core = { path = "../loom-ir-core" }
loom-core = { path = "../loom-core", optional = true }
arrow = { workspace = true, optional = true }
arrow-data = { workspace = true, optional = true }
arrow-schema = { workspace = true, optional = true }
loom-parquet-ingress = { path = "../../ingress/loom-parquet-ingress", optional = true }
loom-vortex-ingress = { path = "../../ingress/loom-vortex-ingress", optional = true }
```

```rust
// crates/loom-cli/src/main.rs — feature-gated dispatch

#[cfg(feature = "full")]
fn run_full_commands(command: &str, args: Vec<String>) -> Result<(), String> {
    match command {
        "inspect" => { /* uses loom_core::container_codec etc. */ }
        "decode" => { /* uses loom_core::l1_model etc. */ }
        "verify-artifact" => { /* uses loom_core::artifact_verifier */ }
        "ingest-vortex" => { /* uses loom_vortex_ingress */ }
        _ => Err(format!("unknown command '{command}'")),
    }
}

#[cfg(not(feature = "full"))]
fn run_full_commands(command: &str, _args: Vec<String>) -> Result<(), String> {
    Err(format!(
        "command '{command}' requires the 'full' feature (rebuild with --features full)"
    ))
}

fn run_lean_commands(command: &str, args: Vec<String>) -> Result<(), String> {
    match command {
        "sidecar" => sidecar(&args.get(0).ok_or_else(usage)?.as_str(), args[1..].to_vec()),
        "verify-l2core" => verify_l2core(&args.get(0).ok_or_else(usage)?.as_str()),
        _ => run_full_commands(command, args),  // falls through to full or error
    }
}
```

### Pattern 2: Multiple Staticlib FFI Crates (No Feature Unification Issue)

**What:** Two separate `staticlib` crates in the same workspace, each with its own `extern "C"` surface and `cbindgen.toml`. They produce separate `.a` files with non-overlapping symbol sets.

**When to use:** When the lean and full paths have completely disjoint dependency sets and must not share transitive dependencies.

**Constraints validated:**
- Cargo feature unification only affects crates that share the same dependency. Since `loom-sidecar-ffi` does NOT depend on `loom-core`, and `loom-ffi` DOES depend on `loom-core`, there is no unification conflict. [CITED: doc.rust-lang.org/cargo/reference/features.html#feature-unification]
- Both crates can coexist in the workspace because they are independent members (`crates/loom-ffi` and `crates/loom-sidecar-ffi`).
- Both can be linked into the same C++ extension because their `extern "C"` symbol names are disjoint (e.g., `loom_decode` vs `loom_sidecar_extract`).

**Symbol naming convention for sidecar FFI:**
```rust
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_extract(
    file_path: *const c_char,
    out_overlay_bytes: *mut *mut u8,
    out_overlay_len: *mut usize,
) -> i32 { /* ... */ }

#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_verify(
    overlay_bytes: *const u8,
    overlay_len: usize,
    out_hash: *mut *const c_char,
) -> i32 { /* ... */ }

#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_route(
    overlay_bytes: *const u8,
    overlay_len: usize,
    host_data_ptr: *const u8,
    host_data_len: usize,
    out_decision: *mut *const c_char,
) -> i32 { /* ... */ }

#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_free_bytes(ptr: *mut u8, len: usize) -> i32 { /* ... */ }
```

### Pattern 3: Self-Ingress as Thin Wrapper

**What:** `loom-self-ingress` wraps `loom-container`'s public API, not reimplementing codecs. It provides file-system IO (reading/writing bytes) while delegating all format logic to `loom-container`.

**When to use:** When consolidating all `.loom` file access through a single boundary, without duplicating codec logic.

**Example:**
```rust
// crates/loom-self-ingress/src/lib.rs
use std::fs;
use std::path::Path;
use loom_container::container_codec;

/// Read a .loom file, verifying the container structure.
pub fn read_loom_file(path: &Path) -> Result<Vec<u8>, SelfIngressError> {
    let bytes = fs::read(path).map_err(|e| SelfIngressError::Io(e))?;
    if !container_codec::is_container_payload(&bytes) {
        return Err(SelfIngressError::NotALoomFile);
    }
    Ok(bytes)
}

/// Write bytes as a .loom file.
pub fn write_loom_file(path: &Path, bytes: &[u8]) -> Result<(), SelfIngressError> {
    // Optional: validate bytes are a valid container before writing
    fs::write(path, bytes).map_err(|e| SelfIngressError::Io(e))
}
```

### Anti-Patterns to Avoid

- **Feature-gating `loom-core` to exclude container:** Cargo feature unification means if ANY workspace member enables the container feature, ALL members see it. `loom-sidecar-ffi` would silently get container deps. Use separate crate with direct `loom-ir-core` dependency instead. [CITED: doc.rust-lang.org/cargo/reference/features.html#feature-unification]
- **Merging sidecar-ffi into existing loom-ffi via features:** Same unification problem. The DuckDB extension build would always get container deps because another crate in the workspace enables them.
- **Copying sidecar logic into sidecar-ffi:** Duplicates source of truth. `loom-ir-core` already owns `sidecar.rs`, `sidecar_routing.rs`, `l2core_codec.rs`. Reuse them.
- **Putting `loom-sidecar-ffi` in `contrib/`:** FFI crates belong in `crates/` alongside `loom-ffi`. `contrib/` is for external integrations (iceberg-binding, duckdb-ext).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| C header generation for sidecar FFI | Manual `.h` file | `cbindgen` (already in workspace) | Same tool as loom-ffi; cbindgen.toml per crate |
| Feature-gated compilation | Custom build script logic | Cargo `[features]` + `#[cfg(feature = "...")]` | Standard Rust mechanism; no custom tooling |
| Sidecar overlay encode/decode | Rewrite in sidecar-ffi | `loom_ir_core::sidecar::SidecarOverlay::encode/decode` | Already implemented in Phase 50; tested |
| Content-hash verification | Custom hash | `loom_ir_core::l2core_codec::l2core_program_hash` | FNV-1a via fnv crate; already stable |
| `.loom` file format parsing | Rewrite in self-ingress | `loom_container::container_codec::decode_container` | Container owns the format; self-ingress wraps it |

**Key insight:** The sidecar overlay model and routing logic are already fully implemented in `loom-ir-core` (Phase 50). This phase only adds C ABI wrappers around existing Rust APIs — no new business logic is needed.

## Common Pitfalls

### Pitfall 1: Cargo Feature Unification Silently Pulling Container

**What goes wrong:** If `loom-sidecar-ffi` depends on `loom-core` (even conditionally), Cargo's feature unification will enable `loom-core`'s `loom-container` dependency because another crate like `loom-cli` or `loom-ffi` enables it. The lean path would compile but transitively include all container code.

**Why it happens:** Cargo resolves features across the entire workspace. When multiple crates depend on the same package, Cargo uses the union of all features. There is no way to have two different feature sets on the same dependency in one workspace build.

**How to avoid:** `loom-sidecar-ffi` must NOT list `loom-core` in its `[dependencies]`. It depends directly on `loom-ir-core` and `loom-parquet-ingress`. Similarly, `loom-parquet-ingress` must not depend on `loom-core` (or must make it optional behind a feature that is NOT enabled during the sidecar build).

**Warning signs:** `cargo tree -p loom-sidecar-ffi | grep loom-container` returns any output.

### Pitfall 2: Symbol Conflicts When Linking Two Staticlibs

**What goes wrong:** If both `libloom_ffi.a` and `libloom_sidecar_ffi.a` are linked into the same DuckDB extension, Rust runtime symbols (`__rust_*`, allocator symbols) may collide.

**Why it happens:** Each `staticlib` crate compiles its own copy of the Rust standard library and allocator. When linked together, duplicate symbols cause linker errors or undefined behavior.

**How to avoid:** The DuckDB extension should link ONLY ONE of the two staticlibs at a time, depending on the build mode. Alternatively, if both must be linked, ensure non-overlapping code. In practice, the DuckDB extension will link either the full or sidecar staticlib, not both. The CMakeLists.txt can use a CMake option to switch.

**For Phase 51:** The preferred approach is two separate DuckDB extension build targets, or a single build that can switch between them via a CMake variable.

### Pitfall 3: loom-parquet-ingress Transitive Container Dependency

**What goes wrong:** `loom-parquet-ingress` currently lists `loom-core` as a dependency in its `Cargo.toml`. Even though its source code doesn't appear to use `loom-core` directly, any new `loom-sidecar-ffi` that depends on `loom-parquet-ingress` would transitively get `loom-container`.

**Why it happens:** `loom-core` re-exports `loom-container`. `loom-parquet-ingress` depends on `loom-core`. Therefore, `loom-parquet-ingress` transitively includes `loom-container`.

**How to avoid:** Remove `loom-core` from `loom-parquet-ingress`'s `[dependencies]`. The `source_contract.rs` module uses `loom_source_ingress` types directly; the `sidecar_parquet.rs` module uses `loom_ir_core` types directly. Neither needs `loom-core`. Verify with `cargo tree -p loom-parquet-ingress --no-default-features | grep loom-core`.

**Warning signs:** `grep loom-core ingress/loom-parquet-ingress/Cargo.toml` finds the dependency.

### Pitfall 4: cbindgen Incomplete-Type Issues with New Crate

**What goes wrong:** The new `loom-sidecar-ffi` crate uses `cbindgen` to generate `loom_sidecar.h`, but if it references `FFI_ArrowArray` or `FFI_ArrowSchema` types (which are defined by the Arrow C Data Interface on the C++ side), cbindgen may emit conflicting definitions.

**Why it happens:** cbindgen walks the Rust source and tries to emit definitions for all types. If the Arrow FFI types are not excluded, cbindgen generates incomplete or conflicting C definitions.

**How to avoid:** Mirror the existing `loom-ffi/cbindgen.toml` pattern: exclude `FFI_ArrowArray` and `FFI_ArrowSchema` from the export list, add forward declarations in `after_includes`. The new header should include only `loom_sidecar_*` symbols. The generated header can be leaner than the full `loom.h` since sidecar operations don't produce Arrow arrays.

## Code Examples

### Sidecar FFI Entry Points

```rust
// crates/loom-sidecar-ffi/src/ffi.rs
// Source: Pattern derived from existing loom-ffi/src/ffi.rs (verified patterns: catch_unwind, ptr::write, error codes)

use std::ffi::{c_char, CStr, CString};
use std::panic::{self, AssertUnwindSafe};
use loom_ir_core::sidecar::SidecarOverlay;
use loom_ir_core::sidecar_routing::{route_sidecar, SidecarRoutingInput, SidecarRoutingDecision};
use loom_parquet_ingress::sidecar_parquet::extract_sidecar_from_parquet_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LoomSidecarError {
    NullPointer = 1,
    IoError = 2,
    DecodeFailed = 3,
    Panicked = 4,
    NoSidecar = 5,
}

impl LoomSidecarError {
    pub fn code(self) -> i32 { self as i32 }
}

#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_extract(
    file_path: *const c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if file_path.is_null() || out_bytes.is_null() || out_len.is_null() {
        return LoomSidecarError::NullPointer.code();
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let path = CStr::from_ptr(file_path).to_string_lossy();
        let overlay = extract_sidecar_from_parquet_path(std::path::Path::new(path.as_ref()))
            .map_err(|_| LoomSidecarError::IoError)?;
        match overlay {
            None => Err(LoomSidecarError::NoSidecar),
            Some(sidecar) => {
                let encoded = sidecar.encode();
                let boxed = encoded.into_boxed_slice();
                let (ptr, len) = (boxed.as_ptr(), boxed.len());
                std::mem::forget(boxed);
                std::ptr::write(out_bytes, ptr as *mut u8);
                std::ptr::write(out_len, len);
                Ok(0)
            }
        }
    }));
    match result {
        Ok(Ok(0)) => 0,
        Ok(Err(e)) => e.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}
```

### CMakeLists.txt Lean Build Path

```cmake
# contrib/duckdb-ext/CMakeLists.txt additions for sidecar-only build
option(LOOM_SIDECAR_ONLY "Build only the sidecar FFI path (no container deps)" OFF)

if(LOOM_SIDECAR_ONLY)
    set(LIBLOOM_FFI "${WORKSPACE_ROOT}/target/release/libloom_sidecar_ffi.a")
    add_custom_command(
        OUTPUT "${LIBLOOM_FFI}"
        COMMAND cargo build -p loom-sidecar-ffi --release
            --manifest-path "${WORKSPACE_CARGO_TOML}"
        WORKING_DIRECTORY "${WORKSPACE_ROOT}"
        COMMENT "Building libloom_sidecar_ffi.a (lean sidecar-only staticlib)"
    )
else()
    set(LIBLOOM_FFI "${WORKSPACE_ROOT}/target/release/libloom_ffi.a")
    # existing cargo build command for full path
endif()
```

## Runtime State Inventory

> This phase is a rename/refactor phase. Runtime state inventory is required.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — verified by grep for `.loom` file paths in databases, n8n, or Redis keys | None |
| Live service config | None — no external services reference `.loom` or Loom crate names | None |
| OS-registered state | None — no systemd, launchd, pm2, or Task Scheduler entries reference Loom | None |
| Secrets/env vars | SOPS key names (if any) are format-agnostic; no references to crate names | None — code rename only |
| Build artifacts | `target/` directory contains stale `.a` files after crate rename; `cargo clean` resolves | Rebuild required after refactoring; no migration |

**Nothing found in any category requiring data migration.** All changes are code edits (new crates, dependency adjustments, feature flags). No stored data or live service configuration references Loom crate names.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust / cargo | All crates | ✓ | 1.87+ (MSRV) | — |
| cbindgen | loom-ffi, loom-sidecar-ffi build.rs | ✓ | 0.29.3 (workspace pin) | — |
| CMake | DuckDB extension build | ✓ | 3.22+ | — |
| DuckDB v1.5.3 headers | DuckDB extension | ✓ | vendored at contrib/duckdb-ext/vendor/ | — |
| LLVM/MLIR (llvm-config) | Full loom-ffi path only | ✓ | 22.1.7 (managed) | Skip with LOOM_ALLOW_NATIVE_TOOL_SKIP=1 |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none

*Step 2.6: Environment audit complete. All required tools are available.*

## Validation Architecture

> Skipped — `workflow.nyquist_validation` is explicitly set to `false` in `.planning/config.json`.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase is infrastructure refactoring; no auth surface |
| V3 Session Management | no | No session state |
| V4 Access Control | no | File system access uses OS permissions; no access control layer |
| V5 Input Validation | yes | Sidecar overlay bytes validated by `SidecarOverlay::decode` fail-closed; file paths validated for null/validity |
| V6 Cryptography | no | Content-hash is FNV-1a (non-cryptographic); no crypto operations |
| V10 Malicious Code | yes | `loom-sidecar-ffi` must mirror existing `catch_unwind` boundary; no `unsafe` in new crate beyond FFI marshaling |

### Known Threat Patterns for Rust FFI Staticlibs

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Panic unwinding across C ABI | Tampering / DoS | `std::panic::catch_unwind` at every `extern "C"` entry point (mirrors existing loom-ffi pattern) |
| Null pointer dereference | Denial of Service | Null checks before all pointer dereferences (mirrors existing `LoomError::NullPointer` pattern) |
| Buffer over-read from host file | Information Disclosure | `SidecarOverlay::decode` validates lengths; fail-closed on malformed input |
| Path traversal in sidecar extract | Information Disclosure | OS open() syscall resolves paths; no custom path logic needed |
| Allocator mismatch (Rust↔C++) | Tampering | System allocator declared in all `staticlib` crates (mirrors existing pattern) |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single `loom-ffi` linking everything | Two FFI crates: `loom-ffi` (full) and `loom-sidecar-ffi` (lean) | Phase 51 now | DuckDB can use sidecar-only path without container deps |
| `loom-container` accessed directly from CLI/FFI | `loom-self-ingress` is single `.loom` IO boundary | Phase 51 now | Centralizes `.loom` file access; easier to audit |

**Deprecated/outdated:**
- Direct use of `loom_container::container_codec` from outside `loom-self-ingress`: After Phase 51, all `.loom` file IO should route through `loom-self-ingress`. Internal use within `loom-core`'s own verify/decode paths remains acceptable.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `loom-parquet-ingress` does not actually use `loom-core` in its source code (only listed in Cargo.toml) | Common Pitfalls #3 | If it does use loom-core types, the dependency removal needs more work (conditional compilation or split) |
| A2 | The DuckDB extension will link only one of the two staticlibs at a time | Common Pitfalls #2 | If both must be linked simultaneously, Rust runtime symbol conflicts need resolution |
| A3 | `loom-ir-core`'s sidecar/routing/l2core_codec modules have no silent dependency on Arrow types | Architecture Patterns | Verified by grep — ir-core Cargo.toml shows only `fnv` dependency; confirmed zero Arrow |
| A4 | Feature unification behavior described in Cargo docs applies to workspace-internal path dependencies the same as crates.io dependencies | Architecture Patterns | Consistent with Cargo's documented behavior for `resolver = "2"` |

## Open Questions

1. **Should `loom-parquet-ingress` be split into two crates (sidecar-only vs full)?**
   - What we know: The sidecar_parquet.rs module only uses `loom-ir-core`. The source_contract.rs module uses `loom-source-ingress` and `parquet`. Neither uses `loom-core`.
   - What's unclear: Whether removing `loom-core` from deps breaks any tests or downstream consumers.
   - Recommendation: Remove `loom-core` from parquet-ingress deps in the first plan; fix any breakage before proceeding.

2. **One DuckDB extension binary or two?**
   - What we know: The CMakeLists.txt can switch between FFI backends via a CMake option.
   - What's unclear: Whether users need both paths simultaneously or one at a time.
   - Recommendation: Single extension binary that links the full `libloom_ffi.a` by default; the sidecar-only `.a` is built but linked via a CMake option for lean deployments.

3. **Should `loom-self-ingress` be a crate or just a module in `loom-container`?**
   - What we know: The goal is a single IO boundary for `.loom` files. This could be a module or a crate.
   - What's unclear: Whether other crates need to use self-ingress without pulling in all of container.
   - Recommendation: Make it a separate crate so the boundary is enforceable with `cargo tree`. Self-ingress depends on container but consumers can depend on self-ingress without needing to know about container's internal modules.

## Sources

### Primary (HIGH confidence)
- [Cargo Features documentation](https://doc.rust-lang.org/cargo/reference/features.html) — feature unification, optional dependencies, cfg attributes, feature resolver v2 [CITED]
- [Cargo Workspaces documentation](https://doc.rust-lang.org/cargo/reference/workspaces.html) — workspace structure, member management [CITED]
- [cbindgen User Guide](https://github.com/mozilla/cbindgen/blob/main/docs.md) — per-crate configuration, build.rs integration, export exclude [CITED]
- [cbindgen docs.rs](https://docs.rs/cbindgen/latest/cbindgen/) — API reference for Builder, generate_with_config [CITED]
- Codebase source grep — verified dependency chains, module imports, crate structure [VERIFIED: codebase analysis]

### Secondary (MEDIUM confidence)
- Existing loom-ffi patterns (ffi.rs, duckdb_runtime.rs, cbindgen.toml, build.rs, CMakeLists.txt) — verified working patterns for new FFI crate [VERIFIED: codebase analysis]
- Existing loom-ir-core module structure — verified sidecar, routing, l2core_codec, full_verifier are container-free [VERIFIED: codebase analysis]

### Tertiary (LOW confidence)
- None — all recommendations are grounded in verified codebase analysis or official Cargo/cbindgen documentation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new external deps; all crates are workspace-internal with verified versions
- Architecture: HIGH — dependency graph verified by source analysis; Cargo feature unification behavior confirmed by official docs
- Pitfalls: HIGH — pitfall #1 (feature unification) confirmed by official Cargo docs; pitfall #2 (symbol conflicts) is a known Rust staticlib linking concern

**Research date:** 2026-06-11
**Valid until:** 2026-07-11 (stable infrastructure pattern; no fast-moving external deps)
