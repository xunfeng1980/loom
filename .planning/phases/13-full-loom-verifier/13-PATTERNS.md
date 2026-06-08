# Phase 13 Pattern Map

**Status:** Planner pattern map
**Date:** 2026-06-08

## Existing Patterns To Reuse

### Rust verifier module

- Existing file: `crates/loom-core/src/verifier.rs`
- Pattern:
  - `VerificationCode` has stable `as_str()` values for diagnostics.
  - `VerificationDiagnostic` carries `code`, `path`, and `message`.
  - `VerificationReport` stores diagnostics and exposes `is_ok`, `diagnostics`, and `first_error`.
  - Public verifier entry points return reports rather than panicking.
- Phase 13 implication:
  - Add new full-verifier/L2Core diagnostics without breaking the existing MVP0 verifier API.
  - Prefer a separate module such as `full_verifier` or `l2_core_verifier` over overloading `verifier.rs`.

### No unsafe core boundary

- Existing file: `crates/loom-core/src/lib.rs`
- Pattern:
  - `#![forbid(unsafe_code)]` is a hard invariant.
  - New `loom-core` modules must stay safe Rust.
- Phase 13 implication:
  - The Rust executable verifier must live under this no-unsafe boundary.

### Safety contract tests

- Existing file: `crates/loom-core/tests/safety_contract.rs`
- Pattern:
  - Focused tests build small malformed inputs directly.
  - Tests use `catch_unwind` helper when proving no-panic behavior.
  - Test names include obligation IDs.
- Phase 13 implication:
  - Add focused tests such as `verifier_13_l2core.rs`.
  - Name tests after `VERIFIER-*` requirements where useful.

### Shell gate scripts

- Existing files:
  - `scripts/safety-proof-test.sh`
  - `scripts/verifier-negative-test.sh`
  - `scripts/container-negative-test.sh`
  - `scripts/mvp0-verify.sh`
- Pattern:
  - Use `set -euo pipefail`.
  - Resolve repo root through `git rev-parse --show-toplevel`.
  - Use `rg` checks for proof/document IDs.
  - Run focused Rust tests before broader gates.
- Phase 13 implication:
  - Add `scripts/full-verifier-test.sh`.
  - Wire it into `scripts/mvp0-verify.sh` only after the gate is stable.

### CLI inspect/decode behavior

- Existing file: `crates/loom-cli/src/main.rs`
- Pattern:
  - Small command router with `loom <inspect|decode> <payload-or-descriptor>`.
  - Failures return `Err(String)` and exit nonzero.
  - Verifier status is printed before successful decode/inspect details.
- Phase 13 implication:
  - If exposing L2Core, add a small `verify-l2core` command or document an internal-only verifier prototype.
  - Keep user-facing failures stable and grep-friendly.

### Planning proof artifacts

- Existing files:
  - `.planning/phases/12-formal-verifier-safety-proof-mvp/12-SAFETY-CONTRACT.md`
  - `.planning/phases/12-formal-verifier-safety-proof-mvp/12-PROOF-OBLIGATIONS.md`
  - `.planning/phases/12-formal-verifier-safety-proof-mvp/12-SAFETY-PROOF.md`
- Pattern:
  - Separate scope contract, proof-obligation matrix, and final proof narrative.
  - Explicit exclusions prevent overclaiming future work.
- Phase 13 implication:
  - Use `13-VERIFIER-SPEC.md`, `13-PROOF-OBLIGATIONS.md`, and final `13-VERIFIER-REPORT.md`.

## New Artifact Families Expected

- `crates/loom-core/src/l2_core.rs` or equivalent: tiny future language model.
- `crates/loom-core/src/full_verifier.rs` or equivalent: type/effect and abstract interpretation verifier.
- `crates/loom-core/tests/full_verifier.rs`: executable verifier tests.
- `formal/lean/LoomCore.lean`: Lean soundness scaffold.
- `specs/tla/LoomVerifierPipeline.tla` and `.cfg`: lifecycle invariant model.
- `scripts/full-verifier-test.sh`: Phase 13 gate.

