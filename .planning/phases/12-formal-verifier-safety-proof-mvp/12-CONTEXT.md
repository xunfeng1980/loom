# Phase 12: Formal Verifier / Safety Proof MVP - Context

**Gathered:** 2026-06-08
**Status:** Ready for research and planning

<domain>
## Phase Boundary

Phase 12 turns the current structural verifier and `LMC1` container boundary into a defensible safety-proof MVP. The phase must prove and gate the safety behavior of the implemented pipeline:

`LMC1/LMP1/LMT1 bytes -> verifier/decode helpers -> Arrow output -> FFI/CLI/DuckDB surfaces`

This is not the full formal verifier for the future Loom IR. It does not prove the final L2 total-function language, MLIR lowering, native codegen, content-hash artifact lookup, signatures, attestation, or real Vortex file ingestion.

</domain>

<decisions>
## Implementation Decisions

### Proof Target Boundary

- **D-12-01:** The proof target is the current implemented safety boundary, not the complete README future design.
- **D-12-02:** The boundary includes `LMC1` container shape, raw `LMP1`/`LMT1` compatibility, verifier routing, decode helpers, FFI error handling, CLI diagnostics, DuckDB smoke ingress, and release gates.
- **D-12-03:** Correctness remains out of scope. Phase 12 proves fail-closed safety and well-formed Arrow construction properties for the implemented surface, not that decoded values are semantically correct beyond existing oracle tests.

### Proof Artifact Shape

- **D-12-04:** Use a proof-obligation matrix plus executable gates plus written safety argument.
- **D-12-05:** Do not introduce a theorem prover or model checker in Phase 12 unless research finds a very small, low-risk crate/tool that fits the current codebase without derailing the milestone.
- **D-12-06:** The proof artifacts should be reviewer-readable and tied to concrete code paths, tests, and scripts.

### Verifier Contract

- **D-12-07:** Stabilize the existing verifier contract rather than rewriting the verifier API.
- **D-12-08:** The contract is: verifier/decode surfaces return typed diagnostics or typed decode errors, fail closed before Arrow output on malformed input, and never panic on attacker-controlled malformed payloads.
- **D-12-09:** Existing diagnostic code/path/message behavior should be documented as a contract and backed by coverage mapping.

### Totality and Termination MVP

- **D-12-10:** Termination proof scope is limited to the current interpreter and parser loops.
- **D-12-11:** The proof should explain why current loops are bounded by container section counts, payload lengths, row counts, column counts, buffer lengths, or finite decoded arrays.
- **D-12-12:** Do not attempt to prove the future L2 total-function language in this phase. That remains a later formal-verifier milestone after the language exists.

### Release Gate

- **D-12-13:** Add a dedicated safety-proof gate, expected as `scripts/safety-proof-test.sh` unless planning finds a better local naming pattern.
- **D-12-14:** Wire the safety-proof gate into `scripts/mvp0-verify.sh`.
- **D-12-15:** The gate must stay practical for local execution. It should compose focused tests, negative fixtures, contract checks, and documentation/proof-obligation consistency checks without creating a slow or brittle release process.

### the agent's Discretion

The agent may choose the exact proof matrix format, documentation filenames, and test/script decomposition, provided the result is concrete, reviewable, and integrated into the existing release gate.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Current Safety and Container Boundary

- `crates/loom-core/src/verifier.rs` — current structural verifier, typed diagnostics, `verify_layout`, `verify_table`, and `verify_container`.
- `crates/loom-core/src/container_codec.rs` — `LMC1` magic/version/features/section directory, checked decode, raw payload wrapping, and container-aware decode helpers.
- `crates/loom-core/src/layout_codec.rs` — raw `LMP1` decode surface and parse-time checks.
- `crates/loom-core/src/table_codec.rs` — raw `LMT1` table decode surface and table shape checks.
- `crates/loom-core/src/l1_model.rs` — interpreter decode paths, bounded loops, Arrow builder output, and error propagation.
- `crates/loom-ffi/src/ffi.rs` — FFI panic boundary and `DecodeFailed` mapping.
- `crates/loom-cli/src/main.rs` — CLI verifier visibility and container diagnostics.
- `duckdb-ext/loom_extension.cpp` — C++ DuckDB bind/scan boundary and shallow `LMC1` parsing for SQL ingress.

### Existing Gates

- `scripts/mvp0-verify.sh` — one-command release gate that Phase 12 must extend.
- `scripts/verifier-negative-test.sh` — curated malformed descriptor/table failures.
- `scripts/container-negative-test.sh` — curated malformed `LMC1` failures.
- `scripts/duckdb-smoke-test.sh` — SQL success gate over generated `LMC1` fixtures.

### Planning and Scope

- `.planning/PROJECT.md` — validated requirements, out-of-scope boundaries, and Phase 12 placeholder.
- `.planning/REQUIREMENTS.md` — completed SAFE/DIST requirements and current out-of-scope formal proof entry.
- `.planning/ROADMAP.md` — Phase 12 placeholder and dependencies.
- `.planning/phases/09-verifier-and-safety-boundary-mvp/09-CONTEXT.md` — prior safety-boundary decisions.
- `.planning/phases/11-distribution-container-v0/11-CONTEXT.md` — distribution container boundary decisions.
- `.planning/phases/11-distribution-container-v0/11-04-SUMMARY.md` — final Phase 11 verification and residual follow-ups.
- `README.md` sections "Current MVP0 Implementation", "The Safety Boundary", and "Distribution, Trust, and the Fast Path" — public scope language and future-design distinction.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `VerificationReport` and `VerificationDiagnostic` already provide code/path/message triples suitable for a formalized contract.
- `LoomDecodeError` already carries typed decode failures for parse/decode boundaries.
- `container-negative-test.sh` and `verifier-negative-test.sh` already create malformed input families that can become proof-obligation evidence.
- `mvp0-verify.sh` already centralizes workspace tests, dependency isolation, fixture hygiene, negative gates, and DuckDB SQL smoke.

### Established Patterns

- The project prefers executable gates over claims in documentation alone.
- `loom-core` must remain free of Vortex/FastLanes dependencies.
- FFI keeps one C ABI function and maps malformed input to error codes instead of exposing Rust panics.
- Current DuckDB path uses direct DataChunk population; ArrowArrayStream remains deferred.
- Raw `LMP1`/`LMT1` compatibility remains load-bearing after `LMC1`.

### Integration Points

- Add proof docs under a stable repo path chosen during planning, likely `.planning/phases/12-formal-verifier-safety-proof-mvp/` plus concise README references if needed.
- Add safety gate script under `scripts/` and invoke it from `scripts/mvp0-verify.sh`.
- Add focused Rust tests only where proof obligations reveal missing executable coverage.

</code_context>

<specifics>
## Specific Ideas

- Treat Phase 12 as "make the safety argument inspectable and mechanically guarded" rather than "finish formal methods for Loom."
- The core artifact should map claims to concrete code paths and tests. A reviewer should be able to ask: "What prevents malformed input X from reaching Arrow output?" and find a specific verifier/decode check plus a gate.

</specifics>

<deferred>
## Deferred Ideas

- Full theorem-prover/model-checker integration.
- Proof of the future L2 total-function language.
- Proof of MLIR/native lowering correctness.
- Real Vortex file/container ingress safety proof.
- Content-hash URI, signatures, attestation, remote fetch, encryption, and native fast-path security model.

</deferred>

---

*Phase: 12-Formal Verifier / Safety Proof MVP*
*Context gathered: 2026-06-08*
