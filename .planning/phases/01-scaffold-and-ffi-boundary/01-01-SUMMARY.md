---
phase: 01-scaffold-and-ffi-boundary
plan: "01"
subsystem: workspace-scaffold
tags: [cargo-workspace, arrow-ffi, global-allocator, panic-abort, vortex-isolation]
dependency_graph:
  requires: []
  provides: [workspace-root, loom-core-crate, loom-ffi-crate, loom-fixtures-crate, arrow-version-pin, system-allocator, panic-abort]
  affects: [all-subsequent-plans]
tech_stack:
  added: [arrow=58.3.0, vortex-array=0.74.0, vortex-fastlanes=0.74.0, vortex-fsst=0.74.0, cbindgen=0.29.3]
  patterns: [workspace-inheritance, exact-version-pins, staticlib+rlib, forbid-unsafe-code]
key_files:
  created:
    - Cargo.toml
    - rust-toolchain.toml
    - .gitignore
    - crates/loom-core/Cargo.toml
    - crates/loom-core/src/lib.rs
    - crates/loom-ffi/Cargo.toml
    - crates/loom-ffi/src/lib.rs
    - crates/loom-fixtures/Cargo.toml
    - crates/loom-fixtures/src/lib.rs
    - Cargo.lock
  modified: []
decisions:
  - "Toolchain pinned to 1.92.0 (not 1.87.0 as STACK.md stated) to satisfy vortex-array 0.74.0 MSRV of 1.91.0"
  - "vortex-dict removed from workspace.dependencies — crate does not exist at 0.74.0 on crates.io; dictionary encoding is provided by vortex-array at 0.74.0"
  - "[patch.crates-io] section removed — cannot redirect crates.io packages back to crates.io; version unification achieved via =58.3.0 exact pins in workspace.dependencies"
metrics:
  duration: "~10 minutes"
  completed: "2026-06-07T10:09:49Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 10
  files_modified: 0
---

# Phase 1 Plan 1: Workspace Scaffold and Version Unification Summary

3-crate Cargo workspace with arrow-rs 58.3.0 unified across the entire dependency graph, panic=abort release profile, System global allocator in loom-ffi, and clean vortex isolation boundary (loom-core has zero vortex deps).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Workspace root, toolchain pin, version unification | 669013d | Cargo.toml, rust-toolchain.toml, .gitignore |
| 2 | Three member crates + System allocator | dfaafa8 | 7 crate files + Cargo.lock |

## Invariants Verified

The four acceptance criteria from the plan were verified after Task 2:

| Check | Command | Result |
|-------|---------|--------|
| Workspace builds (debug + release, zero warnings) | `cargo build --workspace --release` | Finished 0 warnings |
| Arrow version unification | Python lockfile scan for multi-version arrow-* | PASS: all at 58.3.0 |
| vortex-file absent from lockfile | `grep vortex-file Cargo.lock` | PASS: absent |
| loom-core has no vortex dep | `cargo tree -p loom-core \| grep vortex` | PASS: clean |
| panic=abort present | `grep 'panic = "abort"' Cargo.toml` | 1 line |
| System allocator present | `grep 'global_allocator' crates/loom-ffi/src/lib.rs` | 1 line |
| forbid(unsafe_code) in loom-core | `grep 'forbid(unsafe_code)' crates/loom-core/src/lib.rs` | 1 line |
| staticlib in loom-ffi | `grep 'staticlib' crates/loom-ffi/Cargo.toml` | present |

## Pinned Versions (for downstream phases)

| Item | Value |
|------|-------|
| Rust toolchain | 1.92.0 (channel in rust-toolchain.toml) |
| arrow / arrow-array / arrow-schema / arrow-data | =58.3.0 |
| vortex-array | =0.74.0 |
| vortex-fastlanes | =0.74.0 |
| vortex-fsst | =0.74.0 |
| cbindgen (declared, used in Plan 02) | =0.29.3 |

## Crate Inventory

| Crate | Path | Purpose | Unsafe | Vortex |
|-------|------|---------|--------|--------|
| loom-core | crates/loom-core | Pure-Rust decode library | `#![forbid(unsafe_code)]` | None (D-02) |
| loom-ffi | crates/loom-ffi | staticlib + rlib FFI shim, global allocator | Allowed (System allocator) | None |
| loom-fixtures | crates/loom-fixtures | Oracle decoder, fixture builders | Allowed | vortex-array/fastlanes/fsst (D-02) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] vortex-dict =0.74.0 does not exist on crates.io**
- **Found during:** Task 2, first cargo build attempt
- **Issue:** STACK.md listed `vortex-dict = "=0.74.0"` as a pinned dependency. The crate `vortex-dict` was last published at 0.54.0; in the 0.7x Vortex series, dictionary encoding was absorbed into `vortex-array`. There is no 0.74.0 release for `vortex-dict`.
- **Fix:** Removed `vortex-dict` from `[workspace.dependencies]` and from `loom-fixtures/Cargo.toml`. Dictionary encoding for Phase 3 fixture builders will use the `DictArray` type from `vortex-array` 0.74.0 directly.
- **Files modified:** Cargo.toml, crates/loom-fixtures/Cargo.toml
- **Commit:** dfaafa8
- **Impact on later phases:** Phase 3 and 4 plans that reference `vortex-dict` as a separate crate must use `vortex-array` instead for dict types.

**2. [Rule 3 - Blocking] Toolchain updated from 1.87.0 to 1.92.0**
- **Found during:** Task 2, after first cargo build attempt
- **Issue:** `vortex-array` 0.74.0 declares `rust-version = "1.91.0"` in its Cargo.toml. STACK.md listed 1.87.0 as the cbindgen MSRV, but did not account for vortex-array's own MSRV.
- **Fix:** Updated `rust-toolchain.toml` to `channel = "1.92.0"` (the next available installed stable that satisfies 1.91.0 MSRV). The system rustc is 1.96.0; 1.92.0 is installed via rustup and satisfies the constraint.
- **Files modified:** rust-toolchain.toml
- **Commit:** dfaafa8 (included in Task 2 commit since the fix was discovered during Task 2)

**3. [Rule 3 - Blocking] [patch.crates-io] section removed**
- **Found during:** Task 1 verification (cargo build failed immediately)
- **Issue:** A `[patch.crates-io]` section that redirects arrow-* back to crates.io (same source) is invalid — Cargo requires patches to point to a *different* source (local path or git). The intent was version unification, but this is achieved by the `=58.3.0` exact pins in `[workspace.dependencies]` combined with Cargo resolver 2.
- **Fix:** Removed `[patch.crates-io]` entirely. Version unification verified via Cargo.lock lockfile analysis: all 14 arrow-* sub-crates at exactly 58.3.0, zero version duplicates.
- **Files modified:** Cargo.toml
- **Commit:** dfaafa8

## Known Stubs

| File | Stub | Reason |
|------|------|--------|
| crates/loom-core/src/lib.rs | Empty `mod arrow_builder_output {}`, `mod l1_model {}`, `mod l2_kernel_registry {}` | Decode logic arrives in Phase 3; stubs establish module skeleton |
| crates/loom-fixtures/src/lib.rs | Empty lib | Fixture builders arrive in Phase 3 alongside the L1 decode loop |
| crates/loom-ffi/src/lib.rs | No `extern "C"` functions | FFI surface arrives in Plan 02; this plan only establishes the global allocator |

These stubs are intentional per plan scope. The plan goal (compiling workspace with correct invariants) is fully achieved.

## Threat Flags

No new security-relevant surface was introduced beyond what the plan's threat model covers. All T-01-xx mitigations are in place:
- T-01-01: `#[global_allocator] static GLOBAL: System = System;` verified
- T-01-02: Arrow version unification verified (all at 58.3.0)
- T-01-03: `panic = "abort"` in [profile.release] verified
- T-01-04: `vortex-file` absent from Cargo.lock verified

## Self-Check: PASSED

All 10 source files confirmed present on disk. Both commits (669013d, dfaafa8) confirmed in git history.
