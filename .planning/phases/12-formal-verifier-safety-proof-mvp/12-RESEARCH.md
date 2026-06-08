# Phase 12 Research: Formal Verifier / Safety Proof MVP

**Date:** 2026-06-08
**Status:** Ready for plan
**Primary scope:** Safety proof surface for the implemented `LMC1`/`LMP1`/`LMT1` decode pipeline
**Explicitly deferred:** Future Loom IR proof, future L2 total-function language proof, MLIR/native lowering proof, real Vortex file ingress proof

## Executive Summary

Phase 12 should make Loom's implemented safety boundary reviewable and mechanically guarded. The right target is not a theorem prover integration. The right target is a proof-obligation matrix, written safety argument, focused no-panic/fail-closed tests, and a dedicated release gate wired into `scripts/mvp0-verify.sh`.

Recommended direction:

- Treat the proof target as the current implemented chain: `LMC1/LMP1/LMT1 bytes -> verifier/decode helpers -> Arrow output -> FFI/CLI/DuckDB surfaces`.
- Define a stable safety contract around typed diagnostics/errors, fail-closed behavior before Arrow output, bounded parsing/interpreter loops, and no `loom-core` unsafe code.
- Build a proof-obligation matrix that maps each safety claim to concrete code paths, tests, and release-gate commands.
- Add focused executable coverage for malformed bytes and descriptors that must return `Err`, verifier diagnostics, or FFI error codes rather than panicking.
- Add `scripts/safety-proof-test.sh` and call it from `scripts/mvp0-verify.sh`.
- Document what Phase 12 proves and what it does not prove in public docs to avoid overclaiming.

## Local Evidence

The existing codebase already has the essential primitives for a safety-proof MVP:

| Area | Existing evidence | Phase 12 use |
|---|---|---|
| Container boundary | `crates/loom-core/src/container_codec.rs` decodes `LMC1`, validates version/features/sections, and extracts wrapped payloads | Matrix obligations for section arithmetic, feature fail-closed behavior, and raw compatibility |
| Structural verifier | `crates/loom-core/src/verifier.rs` returns `VerificationReport` with code/path/message diagnostics | Contract docs and diagnostic stability coverage |
| Payload parsers | `layout_codec.rs` and `table_codec.rs` parse `LMP1`/`LMT1` with typed `Result` errors | Parser fail-closed obligations |
| Interpreter | `l1_model.rs` calls `verify_layout` before materializing Arrow output | Decode-before-Arrow obligation and bounded-loop argument |
| L2 kernels | `l2_kernel_registry.rs`, `fsst_params.rs`, and `alp_params.rs` use checked params and finite row counts | Kernel parameter and loop-bound obligations |
| FFI | `crates/loom-ffi/src/ffi.rs` wraps `loom_decode` in `catch_unwind` and maps failures to `LoomError` codes | FFI panic-containment obligation |
| CLI gates | `scripts/verifier-negative-test.sh` and `scripts/container-negative-test.sh` already exercise malformed inputs | Inputs for the safety proof gate |
| Release gate | `scripts/mvp0-verify.sh` centralizes test, dependency, fixture, negative, and DuckDB checks | Integration point for Phase 12 safety proof gate |

## Recommended Proof Contract

The Phase 12 contract should be precise and narrow:

1. Malformed attacker-controlled bytes do not cause `loom-core` decode entry points to panic.
2. Malformed bytes or descriptors fail closed through typed parse/decode errors or verifier diagnostics before Arrow output is produced.
3. `LMC1` unknown required features, unsupported versions, malformed section directories, duplicate payload sections, and invalid offsets are rejected before wrapped payload decode.
4. `LMP1`/`LMT1` raw compatibility remains, but raw payloads pass through the same verifier/decode safety boundary before Arrow materialization.
5. Parser and interpreter loops are bounded by finite values already read from payload bytes or decoded arrays: section count, column count, row count, run count, dictionary code count, buffer length, and kernel output count.
6. `loom-core` remains `#![forbid(unsafe_code)]`; FFI unsafety remains isolated in `loom-ffi` and guarded by `catch_unwind`.
7. Correctness is not claimed beyond existing oracle tests. Phase 12 proves safety and well-formed output construction for the implemented surface.

## Proof-Obligation Families

Use stable obligation IDs so docs, tests, scripts, and summaries can refer to the same claims.

| Obligation | Claim | Primary evidence |
|---|---|---|
| `OBL-12-01` | `loom-core` uses no unsafe code and keeps C ABI unsafety outside the core decoder | `#![forbid(unsafe_code)]`, FFI module boundary |
| `OBL-12-02` | `LMC1` container parsing rejects malformed headers, features, and section directories before payload decode | `container_codec` tests, container negative gate |
| `OBL-12-03` | Raw `LMP1`/`LMT1` payloads remain compatible but fail closed on parse errors | layout/table codec tests, malformed raw payload tests |
| `OBL-12-04` | Verifier diagnostics are typed, path-addressed, and exposed before decode output | verifier tests, CLI inspect negative gate |
| `OBL-12-05` | Decode helpers call verifier before Arrow output and return typed errors on verifier failure | `l1_model.rs`, FFI tests |
| `OBL-12-06` | Interpreter loops terminate because all loops are bounded by finite payload-derived counts or decoded array lengths | written loop-bound audit plus focused regression tests |
| `OBL-12-07` | L2 kernel params fail closed and kernel panics do not cross the public boundary | kernel param tests, registry `catch_unwind`, FFI `catch_unwind` |
| `OBL-12-08` | CLI and DuckDB ingress do not convert verifier/container failures into successful scans | negative scripts and DuckDB smoke gate |
| `OBL-12-09` | Release verification continuously checks docs, obligations, tests, and scripts together | `scripts/safety-proof-test.sh` in `mvp0-verify.sh` |

## Gaps Found During Research

| Gap | Impact | Recommended plan |
|---|---:|---|
| No single proof-obligation matrix exists | High | Add `12-PROOF-OBLIGATIONS.md` and map every claim to code/tests/gates |
| No written termination/loop-bound argument exists | High | Add `12-SAFETY-PROOF.md` with loop-bound table for container, payload, verifier, interpreter, and kernels |
| Existing negative gates are useful but not explicitly tied to obligations | Medium | Add safety proof script that composes existing gates and checks obligation IDs |
| No focused public no-panic contract test exists for curated malformed byte families | Medium | Add Rust tests around public decode/verifier surfaces using `catch_unwind` where needed |
| Public docs still describe formal proof as future/out-of-scope | Medium | Update docs after implementation to state the narrow Phase 12 proof surface and exclusions |

## Recommended Phase 12 Success Criteria

1. A proof-obligation matrix exists and maps every Phase 12 safety claim to source files, test names, and release-gate commands.
2. A written safety proof explains fail-closed behavior, no-panic expectations, unsafe isolation, and termination bounds for current parser/interpreter loops.
3. Focused Rust tests prove curated malformed `LMC1`/`LMP1`/`LMT1`/descriptor inputs return typed failures or diagnostics rather than panics.
4. `scripts/safety-proof-test.sh` runs the proof consistency checks, focused tests, existing negative gates, and static safety checks.
5. `scripts/mvp0-verify.sh` invokes `scripts/safety-proof-test.sh`.
6. Public/project docs clearly state that Phase 12 proves safety of the current implemented boundary only, not future native lowering or real Vortex file ingestion.

## Risks

| Risk | Impact | Mitigation |
|---|---:|---|
| Formal proof language overclaims what the code proves | High | Use explicit "implemented boundary only" scope and list exclusions in docs |
| Safety gate becomes slow or brittle | Medium | Compose focused tests and existing negative scripts; keep full workspace tests in `mvp0-verify.sh` |
| Diagnostic contract freezes too much too early | Medium | Stabilize code/path categories, not exact prose message wording unless already relied on by scripts |
| Panic scan produces false positives from tests or unreachable post-check unwraps | Medium | Prefer curated no-panic tests plus a documented allowlist over a naive repository-wide ban |
| Phase 12 drifts into future L2/MLIR proof design | High | Keep future proof work as Phase 13+ roadmap placeholders |

