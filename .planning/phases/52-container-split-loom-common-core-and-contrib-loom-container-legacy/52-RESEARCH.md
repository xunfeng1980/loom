# Phase 52: Container Split — loom-common Core + contrib/loom-container Legacy

**Researched:** 2026-06-11
**Domain:** Rust workspace crate splitting / module reorganization
**Confidence:** HIGH

## Summary

This phase splits the monolithic `crates/loom-container` crate (22 modules, ~13,500 lines) into two crates: `crates/loom-common` for production-core modules and `contrib/loom-container` for legacy container-format modules. The goal is a dependency architecture where DuckDB extension + native codegen depend only on `loom-common`, and the legacy `.loom` format lives in `contrib/`. Both `loom-common` and `contrib/loom-container` must remain workspaces members so `cargo` commands (check, test, clippy) cover both.

**Primary recommendation:** The proposed zero-logic-change split cannot be executed as specified. Eight modules have cross-category dependencies (Category A modules importing from Category B). Four Category B modules (`fsst_params`, `alp_params`, `arrow_builder_output`, `kloom_harness`) can be moved to common with zero logic changes — they are pure data/param/builder types with no Category A imports. The remaining blockers require extracting `ArtifactVerificationStatus`, `ArtifactVerificationReport`, and associated diagnostic types from `artifact_verifier.rs` into `loom-common`, and keeping `verifier.rs` and `artifact_verifier.rs` (with their container-codec-dependent functions) in `contrib/loom-container`.

The total structural change needed is one type-definition extraction from `artifact_verifier.rs` (lines 19-268, ~250 lines of pure type definitions with zero container-layer imports). All other changes are file moves and import path updates.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Arrow semantic artifact model (LMA1/LMC2) | loom-common | — | Source-compatibility substrate; engine-independent types |
| Arrow semantic codec (encode/decode) | loom-common | — | Byte-level encode/decode of Arrow semantic payloads (IPC-backed) |
| Arrow semantic verifier | loom-common | — | Verifies Arrow semantic artifacts without container dependency |
| Native Arrow semantic execution | loom-common | — | Engine-neutral native execution; used by production codegen path |
| Arrow buffer lowering | loom-common | — | Models primitive Arrow/raw-buffer builder plans |
| Native lowering support checks | loom-common | — | Verifier-gated support predicates; only uses loom-ir-core |
| Production native lowering | loom-common | — | Consumes artifact verification facts; needs extracted types only |
| Decode dialect (loom.decode MLIR) | loom-common | — | Deterministic post-verification contract surface |
| Host runtime ABI and execution policy | loom-common | — | Host-neutral vocabulary; needs ArtifactVerificationStatus only |
| Parameter types (FSST, ALP) | loom-common | — | Pure data types; no dependencies on container or A modules |
| Output builder (arrow_builder_output) | loom-common | — | Typed Arrow builder wrapper; imported by l1_model, native_arrow_semantic |
| K spec-oracle harness | loom-common | — | Offline/CI tooling; only imports loom-ir-core |
| L1 declarative layout model | loom-common | — | Core decode model; needs OutputBuilder, L2KernelRegistry, verify_layout |
| L2 kernel registry | loom-common | — | Kernel dispatch table; needs param types only |
| Artifact verification types | loom-common | — | Shared status/report/facts types; zero container imports |
| Container codec (LMC1/LMP1/LMT1) | contrib/loom-container | — | Legacy container wire format; Loom distribution artifact format |
| Layout codec | contrib/loom-container | — | LMP1 byte-level encode/decode |
| Table codec | contrib/loom-container | — | LMT1 table payload byte-level encode/decode |
| Descriptor (RON roundtrip) | contrib/loom-container | — | Human-readable descriptor; depends on layout_codec |
| Container verifier | contrib/loom-container | — | Verifies LMC1 containers; depends on container_codec, table_codec |
| Artifact verifier (functions) | contrib/loom-container | loom-common | Container-dispatch logic; depends on container_codec; types extracted to common |
| Verified lineage records | contrib/loom-container | — | Provenance records; imports artifact verification types |

## User Constraints (from CONTEXT.md)

### Locked Decisions

None explicitly listed in CONTEXT.md beyond the module categorization proposal.

### the agent's Discretion

Pure infrastructure refactoring. Module categories as specified in CONTEXT.md:
- **Category A (→ loom-common):** arrow_semantic, arrow_semantic_codec, arrow_semantic_verifier, native_arrow_semantic, arrow_buffer_lowering, native_lowering, production_native_lowering, decode_dialect, runtime_abi, artifact_verifier, l1_model, l2_kernel_registry, verifier
- **Category B (→ contrib/loom-container):** container_codec, layout_codec, table_codec, descriptor, verified_lineage, kloom_harness, fsst_params, alp_params, arrow_builder_output

Key principles:
- Zero logic changes — only file moves and import path updates
- `loom-core` switches from `loom-container` to `loom-common` dependency
- `contrib/loom-container` depends on `loom-common` for shared types
- `cargo tree` confirms zero `contrib/loom-container` in production deps of loom-ffi, loom-native-melior, loom-sidecar-ffi

### Deferred Ideas (OUT OF SCOPE)

None.

## Phase Requirements

This is an infrastructure refactoring phase — no functional requirements. The acceptance criteria are:
1. Production crates (loom-ffi, loom-native-melior, loom-sidecar-ffi) depend only on loom-common (not contrib/loom-container)
2. Existing tests pass with updated paths
3. `cargo tree` verifies zero contrib/loom-container in production dep chains

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust 2021 edition | — | Module system, path resolution | Already in use across workspace |
| Cargo resolver v2 | — | Workspace dependency resolution | Already configured in root Cargo.toml |
| arrow-rs | 58.3.0 | Arrow type system used by shared types | Already workspace dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| cbindgen | 0.29.3 | FFI header generation (loom-ffi, loom-sidecar-ffi) | Only in build deps of FFI crates |

**Installation:** No new packages required. This is a pure crate reorganization.

## Package Legitimacy Audit

> No external packages are being installed in this phase. This is a pure crate reorganization with zero new dependencies.

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────┐
│                  Workspace Root                       │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │  loom-ffi     │  │ loom-native- │  │loom-sidecar│ │
│  │ (DuckDB ext)  │  │   melior     │  │   -ffi     │ │
│  └──────┬───────┘  └──────┬───────┘  └─────┬──────┘ │
│         │                 │                │         │
│         ▼                 ▼                ▼         │
│  ┌──────────────────────────────────────────┐       │
│  │           loom-core (re-export shim)      │       │
│  │  ┌─────────────────┐ ┌─────────────────┐ │       │
│  │  │  loom-common     │ │  loom-ir-core   │ │       │
│  │  │  (production     │ │  (L2Core IR,    │ │       │
│  │  │   core types,    │ │   sidecar,      │ │       │
│  │  │   codecs, ABI)   │ │   full_verifier)│ │       │
│  │  └─────────────────┘ └─────────────────┘ │       │
│  └──────────────────────────────────────────┘       │
│                                                     │
│  ┌──────────────────────────────────────────┐       │
│  │       contrib/loom-container (legacy)     │       │
│  │  (container_codec, layout_codec,          │       │
│  │   table_codec, descriptor, verifier,      │       │
│  │   artifact_verifier functions,            │       │
│  │   verified_lineage)                       │       │
│  └──────────────┬───────────────────────────┘       │
│                 │                                    │
│                 ▼                                    │
│  ┌──────────────────────────────────────────┐       │
│  │  loom-self-ingress (IO boundary)          │       │
│  │  loom-cli (dev tool)                      │       │
│  │  loom-fixtures (test fixtures)            │       │
│  │  loom-vortex-ingress (uses l1_model)      │       │
│  └──────────────────────────────────────────┘       │
│                                                     │
│  KEY:                                               │
│  Arrow direction = "depends on"                     │
│  Production crates (top row) → loom-common only     │
│  Legacy consumers → contrib/loom-container          │
│  contrib/loom-container → loom-common (for types)   │
└─────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
crates/
├── loom-core/              # Re-export shim (loom-common + loom-ir-core)
├── loom-ir-core/           # L2Core IR, codec, sidecar, full_verifier (unchanged)
├── loom-common/            # NEW: production-core types, codecs, ABI (15 modules)
│   └── src/
│       ├── lib.rs          # Module declarations + public API
│       ├── arrow_semantic.rs
│       ├── arrow_semantic_codec.rs
│       ├── arrow_semantic_verifier.rs
│       ├── native_arrow_semantic.rs
│       ├── arrow_buffer_lowering.rs
│       ├── native_lowering.rs
│       ├── production_native_lowering.rs
│       ├── decode_dialect.rs
│       ├── runtime_abi.rs
│       ├── l1_model.rs
│       │   └── bitpack.rs
│       ├── l2_kernel_registry.rs
│       ├── fsst_params.rs          # ← moved from B
│       ├── alp_params.rs           # ← moved from B
│       ├── arrow_builder_output.rs # ← moved from B
│       ├── kloom_harness.rs        # ← moved from B
│       └── artifact_types.rs       # ← NEW: extracted from artifact_verifier.rs
├── loom-ffi/               # DuckDB FFI (depends on loom-core)
├── loom-fixtures/          # Test fixtures (depends on loom-core)
├── loom-cli/               # Dev CLI (depends on loom-core + contrib)
├── loom-native-melior/     # Native codegen (depends on loom-core)
└── loom-sidecar-ffi/       # Sidecar FFI (depends on loom-ir-core only)

contrib/
└── loom-container/         # Legacy container format (7 modules)
    └── src/
        ├── lib.rs          # Module declarations
        ├── container_codec.rs
        ├── layout_codec.rs
        ├── table_codec.rs
        ├── descriptor.rs
        ├── verifier.rs           # ← moved from A (depends on container_codec, table_codec)
        ├── artifact_verifier.rs  # ← moved from A (depends on container_codec; types in common)
        └── verified_lineage.rs   # ← moved from A (imports artifact verification types)
```

### Dependency Flow

```
Production crates need:
  loom-core → loom-common (all types, codecs, ABI)
           → loom-ir-core (L2Core IR, sidecar)

Legacy consumers need:
  loom-self-ingress → contrib/loom-container (container_codec)
  loom-cli          → contrib/loom-container (descriptor, layout_codec, table_codec)
  loom-fixtures     → contrib/loom-container (container_codec, layout_codec, table_codec)
  loom-vortex-ingress → loom-common (l1_model) + contrib/loom-container (container_codec)

contrib/loom-container needs:
  loom-common (for ArtifactVerificationStatus, ArtifactVerificationReport, fsst_params, alp_params, l1_model, ...)
  loom-ir-core (for LoomDecodeError, L2CoreProgram, etc.)
```

### Pattern: Module Move Without Logic Change

**What:** Move an entire `.rs` file from one crate to another, updating only `use crate::` → `use loom_common::` or vice versa.

**When to use:** When the module has no intra-crate imports from modules on the "wrong" side of the split.

**Example (fsst_params move to common):**
```rust
// Before (in crates/loom-container/src/fsst_params.rs):
// No intra-crate imports — only external arrow-rs crate types

// After (in crates/loom-common/src/fsst_params.rs):
// No intra-crate imports — same file, different location
// Consumers update: use loom_container::fsst_params → use loom_common::fsst_params
```

### Pattern: Type Extraction for Cross-Category Resolution

**What:** Extract type definitions (structs, enums, impls) that have zero container-layer imports into a new module in `loom-common`, keeping container-dependent functions in `contrib/loom-container`.

**When to use:** When a module straddles the boundary — some types are needed by production crates but functions in the same file import from container modules.

**Example (artifact_verifier extraction):**
```rust
// NEW: crates/loom-common/src/artifact_types.rs
// Contains ONLY types (lines 19-268 of original artifact_verifier.rs):
// - ArtifactVerificationStage (enum)
// - ArtifactVerificationStatus (enum)
// - ArtifactVerificationDiagnostic (struct)
// - ArtifactLoweringDiagnostic (struct)
// - ArtifactLoweringReadiness (struct)
// - ArtifactVerificationFacts (struct)
// - ArtifactVerificationOptions (struct)
// - ArtifactVerificationReport (struct)
// Zero imports from container_codec or any Category B module.

// contrib/loom-container/src/artifact_verifier.rs
// Re-exports types from loom-common, adds container-specific functions:
pub use loom_common::artifact_types::*;
// Then adds: verify_artifact(), verify_artifact_with_l2_core(), etc.
```

### Anti-Patterns to Avoid
- **Circular dependency:** `loom-common` must NEVER import from `contrib/loom-container` — this would defeat the purpose of the split and create a circular dep chain.
- **Partial module moves:** Do not move only part of a module's functions without extracting the shared types first — Rust's module system requires whole-module coherence.
- **Cross-crate `pub use` without path verification:** Every re-export must be verified via `cargo check -p loom-core` before committing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Module dependency resolution | Manual import path grep | `cargo check` + `cargo tree` | Cargo's resolver catches all transitive issues; grep only finds direct imports |
| Crate path configuration | Custom build.rs or env vars | `[workspace] members = ["contrib/loom-container"]` in root Cargo.toml | Standard Cargo workspace mechanism [VERIFIED: doc.rust-lang.org] |
| Type re-export tracking | Hand-maintained lists | `pub use loom_common::*;` in loom-core lib.rs | loom-core is already a re-export shim; same pattern applies |
| Testing split correctness | Manual import inspection | `cargo check -p loom-common -p contrib-loom-container -p loom-native-melior --no-default-features` | Full workspace check with multiple feature combos |

**Key insight:** Cargo's resolver is a more reliable dependency checker than manual grep. After the split, `cargo tree -i contrib-loom-container --invert` should show zero production crates in the inverted dependency tree.

## Complete Module Catalog and Dependency Analysis

### Full Cross-Category Dependency Matrix

Every `use crate::` import from every module was grepped and classified.

#### Category A modules → Category B imports (BLOCKERS)

| Category A Module | Imports from Category B | Severity | Resolution |
|-------------------|------------------------|----------|------------|
| `verifier.rs` | `alp_params`, `container_codec`, `fsst_params`, `table_codec` | BLOCKING | Module stays in contrib; container verifier functions are legacy |
| `artifact_verifier.rs` | `container_codec` (for LMC1 dispatch) | BLOCKING | Type definitions extracted to common; container-dispatch functions stay in contrib |
| `l1_model.rs` | `arrow_builder_output` (main), `fsst_params` (test-only) | RESOLVED | Move `arrow_builder_output` + `fsst_params` to common |
| `l2_kernel_registry.rs` | `alp_params`, `fsst_params` | RESOLVED | Move both param types to common |
| `native_arrow_semantic.rs` | `arrow_builder_output`, `kloom_harness` | RESOLVED | Move both to common |

#### Category B modules → Category A imports (EXPECTED — contrib depends on common)

| Category B Module | Imports from Category A |
|-------------------|------------------------|
| `container_codec.rs` | `l1_model`, `layout_codec`, `table_codec` |
| `layout_codec.rs` | `l1_model` |
| `table_codec.rs` | `l1_model`, `l2_kernel_registry`, `layout_codec`, `verifier` |
| `descriptor.rs` | `l1_model`, `layout_codec` |
| `verified_lineage.rs` | `artifact_verifier` (types), `native_arrow_semantic` |

#### Clean modules (no cross-category issues)

| Module | Original Category | Intra-crate imports | Action |
|--------|-------------------|---------------------|--------|
| `arrow_semantic.rs` | A | Only `loom_ir_core::error` | Move to common ✅ |
| `arrow_semantic_codec.rs` | A | Only `arrow_semantic` + `arrow_semantic_verifier` (both A) | Move to common ✅ |
| `arrow_semantic_verifier.rs` | A | Only `arrow_semantic` (A) | Move to common ✅ |
| `arrow_buffer_lowering.rs` | A | Only `decode_dialect` + `production_native_lowering` (both A) | Move to common ✅ |
| `native_lowering.rs` | A | ZERO intra-crate imports (only loom-ir-core) | Move to common ✅ |
| `decode_dialect.rs` | A | Only `production_native_lowering` (A) | Move to common ✅ |
| `fsst_params.rs` | B | ZERO intra-crate imports | Move to common ✅ |
| `alp_params.rs` | B | ZERO intra-crate imports | Move to common ✅ |
| `arrow_builder_output.rs` | B | ZERO intra-crate imports | Move to common ✅ |
| `kloom_harness.rs` | B | Only `loom_ir_core::l2_core` | Move to common ✅ |
| `container_codec.rs` | B | Only Category B modules | Stay in contrib ✅ |
| `layout_codec.rs` | B | Only l1_model (A) + fsst_params (B→common) | Stay in contrib ✅ |
| `table_codec.rs` | B | l1_model, l2_kernel_registry, verifier (A) + layout_codec (B) | Stay in contrib ✅ |
| `descriptor.rs` | B | l1_model (A) + layout_codec (B) | Stay in contrib ✅ |
| `verified_lineage.rs` | B | artifact_verifier + native_arrow_semantic (A) | Stay in contrib ✅ |

### Module Sizes

| Module | Lines | Complexity |
|--------|-------|------------|
| `native_arrow_semantic.rs` | 2,534 | Largest file; production native codegen logic |
| `l1_model.rs` | 1,445 | Core decode model; critical path |
| `verifier.rs` | 1,110 | Container + layout verification |
| `container_codec.rs` | 927 | Legacy LMC1 wire format |
| `runtime_abi.rs` | 804 | Host-native runtime model |
| `kloom_harness.rs` | 660 | K spec-oracle harness; test tooling |
| `arrow_builder_output.rs` | 646 | Arrow typed builder wrapper |
| `artifact_verifier.rs` | 585 | Unified verification pipeline (~250 lines types, ~335 lines functions) |

## Package Legitimacy Audit

**Skipped:** No external packages are being installed. This phase moves existing code between crates within the same workspace.

## Runtime State Inventory

**Skipped:** This is a greenfield crate reorganization, not a rename/refactor of existing runtime state. All file moves are within the Rust project source. No databases, service configs, OS registrations, secrets, or build artifacts carry the old crate names as mutable state — crate names are compile-time identifiers only.

## Common Pitfalls

### Pitfall 1: Stale Import Paths in Test Files

**What goes wrong:** `cargo check` passes for the library but `cargo test` fails because test files in `/tests/` directories still use `use loom_container::*` instead of `use loom_common::*` or `use contrib_loom_container::*`.

**Why it happens:** Test files are separate compilation units; they don't benefit from `lib.rs` re-exports unless the test crate depends on the re-exporting crate.

**How to avoid:** After all file moves, run `cargo test --workspace` (not just `cargo check`) and fix all compilation errors. Use a systematic search: `rg 'loom_container::' --include '*.rs'` across the entire workspace.

**Warning signs:** `cargo check -p loom-core` passes but `cargo test -p loom-fixtures` fails with "unresolved import".

### Pitfall 2: Phantom Dependency on Removed Crate

**What goes wrong:** A crate's `Cargo.toml` still lists `loom-container` as a dependency, but the crate no longer imports anything from it. This causes unnecessary rebuilds and confuses the dependency audit.

**Why it happens:** During refactoring, imports are updated but Cargo.toml entries are forgotten.

**How to avoid:** After all path updates, run `cargo tree -i loom-container --invert` to verify zero consumers, then remove the dependency from every Cargo.toml that no longer needs it.

**Warning signs:** `cargo tree | grep loom-container` shows the crate in unexpected locations.

### Pitfall 3: Workspace Members Path Mismatch

**What goes wrong:** `contrib/loom-container` is added to workspace `members` but Cargo can't find it because the glob pattern or explicit path doesn't match.

**Why it happens:** `contrib/` is a non-standard path. Standard Cargo workspace conventions use `crates/` or top-level directories.

**How to avoid:** Use an explicit path in the workspace members array: `"contrib/loom-container"`. Verify with `cargo metadata --format-version=1 | jq '.workspace_members'`. [VERIFIED: doc.rust-lang.org/cargo/reference/workspaces.html — `members` field accepts arbitrary paths]

**Warning signs:** `cargo check --workspace` doesn't include the contrib crate.

### Pitfall 4: Orphaned Re-Exports in loom-core

**What goes wrong:** `loom-core/src/lib.rs` re-exports modules from `loom_container` that have moved to `loom_common` or `contrib_loom_container`. Old consumers break because the re-export path changed.

**Why it happens:** `loom-core` is a re-export shim. Every module move requires updating the `pub use` path in `loom-core/src/lib.rs`.

**How to avoid:** After the split, audit `loom-core/src/lib.rs` line by line. Each `pub use loom_container::X` must be updated to `pub use loom_common::X` (for common modules) or `pub use contrib_loom_container::X` (for legacy modules). Run `cargo check -p loom-core` to verify.

**Warning signs:** Compilation errors in downstream crates that `use loom_core::old_module`.

### Pitfall 5: Unintended Public API Leakage

**What goes wrong:** `loom-common` accidentally exposes container-level types or functions through its public API, creating an implicit dependency that defeats the split.

**Why it happens:** `pub use` or `pub mod` in `loom-common/src/lib.rs` includes modules that shouldn't be there.

**How to avoid:** Audit `loom-common/src/lib.rs` after the split. Verify with `cargo tree -p loom-common` that no container types leak out. Any function that takes a `ContainerDescription` or `TableDescription` as a parameter must NOT be in `loom-common`.

**Warning signs:** `cargo check -p loom-common --no-default-features` compiles code that references `container_codec` or `table_codec`.

## Code Examples

### Example 1: Clean Module Move (fsst_params → loom-common)

```bash
# Step 1: Move file
mv crates/loom-container/src/fsst_params.rs crates/loom-common/src/fsst_params.rs

# Step 2: Update loom-common/src/lib.rs
echo "pub mod fsst_params;" >> crates/loom-common/src/lib.rs

# Step 3: Update loom-container/src/lib.rs
# Remove: pub mod fsst_params;

# Step 4: Update imports in consumers (l2_kernel_registry, verifier, etc.)
# Before: use crate::fsst_params::FsstParams;
# After:  use loom_common::fsst_params::FsstParams;
```

### Example 2: Updating Re-Export in loom-core

```rust
// crates/loom-core/src/lib.rs — BEFORE
pub use loom_container::fsst_params;
pub use loom_container::alp_params;
pub use loom_container::l1_model;
pub use loom_container::container_codec;

// crates/loom-core/src/lib.rs — AFTER
// Modules moved to loom-common:
pub use loom_common::fsst_params;
pub use loom_common::alp_params;
pub use loom_common::l1_model;
pub use loom_common::arrow_semantic;
pub use loom_common::runtime_abi;
// ... etc for all loom-common modules

// Modules moved to contrib/loom-container:
pub use contrib_loom_container::container_codec;
pub use contrib_loom_container::layout_codec;
pub use contrib_loom_container::table_codec;
pub use contrib_loom_container::descriptor;
pub use contrib_loom_container::verifier;
pub use contrib_loom_container::artifact_verifier;
pub use contrib_loom_container::verified_lineage;
```

### Example 3: Verifying Production Dep Chain

```bash
# After the split, verify zero contrib/loom-container in production deps:
cargo tree -p loom-ffi --invert | grep -c 'contrib-loom-container'  # Must be 0
cargo tree -p loom-native-melior --invert | grep -c 'contrib-loom-container'  # Must be 0
cargo tree -p loom-sidecar-ffi --invert | grep -c 'contrib-loom-container'  # Must be 0

# Verify loom-common has no dependency on contrib:
cargo tree -p loom-common | grep -c 'contrib-loom-container'  # Must be 0
```

### Example 4: ArtifactVerificationStatus Type Extraction

```rust
// NEW: crates/loom-common/src/artifact_types.rs
// (extracted from artifact_verifier.rs lines 42-56, zero container imports)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactVerificationStatus {
    Accepted,
    Rejected,
    Unsupported,
}

impl ArtifactVerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Unsupported => "unsupported",
        }
    }
}

// Also extract: ArtifactVerificationReport, ArtifactVerificationStage,
// ArtifactVerificationDiagnostic, ArtifactVerificationOptions,
// ArtifactVerificationFacts, ArtifactLoweringDiagnostic, ArtifactLoweringReadiness
// (lines 19-268 of artifact_verifier.rs)

// contrib/loom-container/src/artifact_verifier.rs — AFTER
pub use loom_common::artifact_types::*;

// Container-specific functions (verify_artifact, etc.) remain here
pub fn verify_artifact(bytes: &[u8], ...) -> ArtifactVerificationReport {
    // imports container_codec, l2_kernel_registry, etc.
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Monolithic loom-container crate | Split into loom-common + contrib/loom-container | This phase (52) | Production crates decoupled from legacy container format |
| `crates/` only workspace paths | Mixed `crates/` + `contrib/` paths | This phase | contrib/ isolates legacy code; workspaces support arbitrary paths |

**Deprecated/outdated:**
- `loom-container` (old monolithic crate): Replaced by the split; kept only as a transitional compatibility re-export if needed
- Direct `loom_container::container_codec` imports in production code: Should go through `loom-core` re-exports or be removed

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `kloom_harness` is "offline/CI only, outside production TCB" and safe to move to common | Dependency Analysis | If `kloom_harness` is load-bearing in production codegen, moving it to common adds unnecessary weight. Currently used only in native_arrow_semantic for kloom trace extraction during native/model validation. |
| A2 | `arrow_builder_output` has zero intra-crate imports | Dependency Analysis | Verified by grep: the file only imports from external arrow-rs crates. If future changes add crate-internal imports, the move would need updating. |
| A3 | `ArtifactVerificationStatus` and `ArtifactVerificationReport` type definitions (lines 19-268 of artifact_verifier.rs) have zero container-layer imports | Type Extraction | Verified by code inspection: these types only use `VerifiedArtifactFacts` from `loom_ir_core`. If this assumption is wrong, additional types would need extraction. |
| A4 | `verify_layout` function in verifier.rs does NOT use container_codec or table_codec in its function body | Dependency Analysis | Verified by code inspection at line 112: the function body only uses `LayoutDescription` and `L2KernelRegistry` (both Category A after params move). If this changes, l1_model→verifier becomes a cross-crate dep that needs resolution. |
| A5 | No external consumer depends on `loom-container` directly (they all go through `loom-core`) | Workspace Audit | Verified: only `loom-self-ingress` depends on loom-container directly; all others (loom-ffi, loom-native-melior, loom-fixtures, loom-cli) go through loom-core. `loom-sidecar-ffi` depends on loom-ir-core only. |

## Open Questions

1. **Should `verify_layout` move to loom-common or stay in contrib/loom-container?**
   - What we know: `l1_model.rs` calls `verify_layout`. The function itself only uses `LayoutDescription` and `L2KernelRegistry` (no container types). But it lives in `verifier.rs` alongside container-dependent functions (`verify_container`, `decode_layout_payload_maybe_container`, etc.).
   - What's unclear: Whether to split verifier.rs or move the whole module.
   - Recommendation: Split `verifier.rs` — extract `verify_layout`, `VerificationReport`, `VerificationCode`, `VerificationDiagnostic` (~250 lines, zero container imports) to a new `verify_layout.rs` in `loom-common`. Keep container-specific functions in `contrib/loom-container/verifier.rs`. This is one additional file extraction beyond the artifact_verifier extraction.

2. **Should runtime_abi and production_native_lowering go to loom-common or contrib/loom-container?**
   - What we know: Both import from `artifact_verifier` (A). After artifact types are extracted to common, `runtime_abi` needs only `ArtifactVerificationStatus` (now in common). `production_native_lowering` needs `ArtifactVerificationReport` + `ArtifactVerificationStatus` (both in common).
   - What's unclear: Whether the `CheckProductionLoweringSupport` function in production_native_lowering needs the full artifact verifier pipeline or just the types.
   - Recommendation: Both stay in loom-common. After type extraction, their only dependency from artifact_verifier is on the extracted types (now in common).

3. **Does native_lowering.rs need anything from the container layer?**
   - What we know: `native_lowering.rs` has ZERO intra-crate imports. It only imports from `loom_ir_core`. This is the cleanest module in the entire crate.
   - What's unclear: Nothing. This is confirmed clean.
   - Recommendation: Move to loom-common. Zero changes needed.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (cargo) | Build and test | ✓ | 1.92.0 (workspace MSRV) | — |
| `cargo tree` subcommand | Dependency audit | ✓ | Built-in | — |
| `rg` (ripgrep) | Import path search | ✓ | Available | `grep -r` |

**Missing dependencies:** None. All tools are available.

## Validation Architecture

**Skipped:** `workflow.nyquist_validation` is explicitly set to `false` in `.planning/config.json`. No test infrastructure changes are required for this infrastructure phase.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | — |
| V3 Session Management | No | — |
| V4 Access Control | No | — |
| V5 Input Validation | Yes | No change — existing verifier-gated boundaries remain in place; crate split does not introduce new input surfaces |
| V6 Cryptography | No | — |

### Known Threat Patterns for Rust Crate Reorganization

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Accidental public API expansion | Information Disclosure | Audit `pub` visibility on moved modules; `pub(crate)` where possible |
| Stale dependency on removed crate | Denial of Service | `cargo tree` audit before commit |
| Circular workspace dependency | Tampering | Cargo resolver rejects circular deps at compile time; no manual check needed |
| Phantom type leakage through re-exports | Information Disclosure | loom-common must not expose `ContainerDescription`, `TableDescription`, or any container-codec types |

## Sources

### Primary (HIGH confidence)
- [Codebase grep] — Complete cross-category dependency analysis of all 22 modules in `crates/loom-container/src/`; every `use crate::` import classified as A→A, A→B, B→A, or B→B
- [Codebase grep] — All workspace `Cargo.toml` files containing `loom-container` dependency
- [Codebase grep] — All `.rs` files outside `loom-container/` that import `loom_container::` or `loom_core::container_codec` etc.
- [doc.rust-lang.org] — Cargo workspace `members` field supports arbitrary paths including `contrib/` [VERIFIED: doc.rust-lang.org/cargo/reference/workspaces.html]

### Secondary (MEDIUM confidence)
- [CITED: doc.rust-lang.org/cargo/reference/workspaces.html] — Workspace member path rules confirmed; `default-members`, `exclude`, and glob patterns available

### Tertiary (LOW confidence)
- [ASSUMED] A1-A5 in Assumptions Log — code inspection only; must be re-verified with `cargo check` after file moves

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; existing Rust/Cargo toolchain; workspace path rules verified from official docs
- Architecture: HIGH — complete dependency graph computed from source code grep; all 22 modules classified with explicit import lists
- Pitfalls: MEDIUM — five pitfalls identified from code analysis but not all validated through actual execution (will be validated during plan execution)

**Research date:** 2026-06-11
**Valid until:** 2026-07-11 (30 days — stable infrastructure pattern; no fast-moving external dependencies)
