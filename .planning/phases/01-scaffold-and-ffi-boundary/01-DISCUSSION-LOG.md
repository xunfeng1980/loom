# Phase 1: Scaffold and FFI Boundary - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-07
**Phase:** 01-scaffold-and-ffi-boundary
**Areas discussed:** Workspace layout

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Workspace layout | Single crate vs Cargo workspace (core/ffi/fixtures split) | ✓ |
| FFI contract shape | loom_decode signature + error reporting across the boundary | |
| Phase 1 depth | Pure stub vs minimal real Arrow roundtrip + release test | |
| Verification automation | GitHub Actions CI vs local Makefile/script for CORE checks | |

**User's choice:** Workspace layout only.
**Notes:** The other three areas were left to Claude's discretion (see below).

---

## Workspace layout

### Crate structure

| Option | Description | Selected |
|--------|-------------|----------|
| Multi-crate workspace | loom-core (pure-Rust, zero FFI) + loom-ffi (staticlib) + fixtures/reference crate; isolates unsafe FFI surface | ✓ |
| Single crate | One crate with modules + `[lib] crate-type=["staticlib"]` + bin target | |

**User's choice:** Multi-crate workspace.

### Vortex isolation

| Option | Description | Selected |
|--------|-------------|----------|
| Isolate Vortex to a reader/reference module | Core decode stays vortex-independent; vortex-* only in vortex_reader + oracle binary | ✓ |
| Use vortex types throughout core | loom-core depends on vortex-array types directly | |

**User's choice:** Isolate Vortex.
**Notes:** Keeps the verification oracle on a different code path than the thing it verifies — preserves the honesty of the "Loom decodes independently" proof.

### Toolchain pin

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, pin it | Commit rust-toolchain.toml pinning a specific stable version | ✓ |
| No, use ambient toolchain | Rely on installed stable Rust | |

**User's choice:** Pin the toolchain.

---

## Claude's Discretion

- **FFI contract shape** — `loom_decode` signature + error-reporting strategy. Planner/research to decide, grounded in `research/ARCHITECTURE.md`.
- **Phase 1 depth** — stub vs minimal real Arrow roundtrip. Recommendation recorded in CONTEXT.md: minimal real roundtrip + Rust-side release-path test (per `research/PITFALLS.md`).
- **Verification automation** — CI vs Makefile/script. ROADMAP says "verified in CI"; lean toward a CI workflow.

## Deferred Ideas

None — discussion stayed within phase scope.
