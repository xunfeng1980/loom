# Phase 36 Verified-Lineage Contract

**Status:** Complete for Phase 36 scope
**Requirements:** LINEAGE-01, LINEAGE-02
**Date:** 2026-06-09

## Scope

This contract defines what Loom may mean by "verified" at MVP1.5 exit.

For MVP1.5, **verified** means that a safety or Arrow well-formedness claim is
backed by one named evidence layer, or is explicitly assigned to the Trusted
Computing Base (TCB). It does not mean source-data correctness, upstream format
semantic correctness, performance, production readiness, or compiler/host
correctness.

The standing red line is unchanged:

```text
Loom guarantees safety + well-formedness, never correctness.
```

Phase 36 defines boundaries only. It adds no proofs, production code, execution
features, new gates, broader format support, native speed claims, or host
integration.

## Evidence Layers

Every in-scope safety claim maps to exactly one of these evidence layers.

| Evidence Layer | What It Can Support | What It Cannot Support |
|---|---|---|
| Rust verifier structural check | Executable artifact/container/schema/capability acceptance and fail-closed rejection in Rust. | Lean soundness, source correctness, native/compiler correctness. |
| Bitwuzla SMT discharge | Required local arithmetic/range/bad-state obligations encoded as deterministic `QF_BV` SMT-LIB and discharged as `unsat`. | Checked proof objects, full semantic proof, compiler correctness. |
| Lean soundness theorem | Future machine-checked safety theorem over the modeled executor and modeled L2Core semantics. | Rust interpreter behavior, native behavior, upstream source correctness. |
| differential validation | Future evidence that two executable paths agree on accept/reject behavior or builder-event traces across a fixture/fuzz matrix. | Proof by construction, correctness beyond the validated matrix. |
| explicit TCB trust assumption | A named component or seam trusted but not proven in this milestone. | A verified claim; TCB rows are assumptions, not evidence of proof. |

Evidence status words remain distinct and must not be collapsed:

| Status | Meaning |
|---|---|
| proven | Executable code or focused gate proves the claim for the stated scope. |
| bounded | True only for a named slice, fixture family, or adapter path. |
| fallback | The path is connected, but accepted execution routes through fallback. |
| scaffold | Contracts or reports exist, but load-bearing implementation/proof is absent. |
| skipped | A gate accepted an explicit skip/tool absence condition. |
| deferred | Intentionally incomplete and assigned to later work. |
| unsupported | Current code rejects or does not implement the capability. |
| incorrect | Wording would be false as written and must be corrected. |

## Claim Mapping

| Claim Family | Evidence Layer | Current Source | MVP1.5 Owner | Non-Claim |
|---|---|---|---|---|
| Artifact structural acceptance | Rust verifier structural check | `verify_artifact`, container/source verifier tests, artifact reports | Existing evidence plus Phase 37 parity checks | Source correctness |
| L2Core static acceptance | Rust verifier structural check | `verify_l2_core`, `full_verifier` tests | Phase 37 | Full modeled soundness |
| Arithmetic/range bad states | Bitwuzla SMT discharge | Phase 19 `QF_BV` bad-state queries | Existing evidence, future cross-checks if added | Checked proof objects |
| Modeled executor safety | Lean soundness theorem | Future operational semantics and theorem | Phase 38 | Rust/native correctness |
| Lean-to-Rust verifier parity | differential validation | Future Lean/Rust harness | Phase 37 | Soundness by itself |
| Real interpreter consistency with model | differential validation | Future builder-event trace comparison | Phase 39 | Native correctness |
| Native consistency with model | differential validation | Future native/model validation | Phase 40 | Compiler/toolchain correctness |
| Toolchain, ABI, and host assumptions | explicit TCB trust assumption | This contract's TCB section | TCB | Verified compiler or host behavior |

No row in this table claims correctness of the decoded source data beyond the
explicit oracle/equivalence scope of prior phases.

## TCB

The TCB is not evidence that a component is verified. It is the set of named
components trusted so Loom can make bounded safety and well-formedness claims
without proving the whole world.

| TCB Item | Assumed | Why Not Proven Here |
|---|---|---|
| Rust compiler/std | Rust code that passes the verifier and tests is compiled and run according to Rust language and standard-library semantics. | Proving compiler/std correctness is outside Loom's artifact verifier and formal model scope. |
| LLVM + MLIR toolchain | MLIR/LLVM validation, lowering, and JIT/toolchain behavior preserve the semantics consumed by bounded native evidence. | Compiler pipeline verification is a separate research problem and remains a permanent toolchain trust assumption unless a future phase narrows a specific sub-gap. |
| Rust<->C ABI | `extern "C"` calls, layout-compatible pointers, ownership handoff, panic containment, and release callbacks behave according to the declared ABI contracts. | Phase 36 does not prove cross-language ABI semantics or C/C++ host memory behavior. |
| DuckDB host process | DuckDB calls `loom_scan(path)`/extension entrypoints as documented and respects table-function lifecycle, vector ownership, and cancellation behavior. | DuckDB internals are host infrastructure, not part of Loom's Lean/Rust verifier model. |
| Arrow C Data Interface | Arrow arrays/schemas crossing the boundary obey the Arrow C Data Interface memory, schema, and release semantics. | Loom uses the standard interface as an interop contract; it does not prove the Arrow specification or every consumer implementation. |

The Rust+C++/MLIR/LLVM/toolchain gap stays in the TCB. A future phase may narrow
a named sub-gap with additional evidence, but Phase 36 does not silently close
any part of it.

## Obligation Matrix

| Trust Seam | Obligation | Owner | Status After Phase 36 |
|---|---|---|---|
| Lean<->Rust verifier | Prove or validate that the Lean static checker mirrors the executable Rust verifier's accepted/rejected programs and diagnostics. | Phase 37 | Assigned; not solved here. |
| static<->dynamic | Connect verifier acceptance to modeled execution safety through operational semantics and a soundness theorem. | Phase 38 | Assigned; not solved here. |
| modeled-executor<->real-executor | Validate that real Rust interpreter behavior matches the modeled/reference executor at builder-event trace granularity. | Phase 39 | Assigned; not solved here. |
| native<->model | Validate Phase 35 native Arrow semantic execution against the faithful model/reference path. | Phase 40 | Assigned; depends on Phase 35 and Phase 39. |
| compiler/host/ABI runtime | Trust Rust compiler/std, LLVM + MLIR, Rust<->C ABI, DuckDB host process, and Arrow C Data Interface. | TCB | Named assumption; not solved here. |

Every trust seam is either assigned to a later MVP1.5 phase or named as TCB.

## Non-Claims

Phase 36 does not claim:

- source-data correctness;
- upstream Vortex/Lance/Parquet/Iceberg semantic correctness beyond explicit
  oracle/equivalence evidence;
- performance or native speed;
- production readiness;
- checked solver proof objects or independently checkable solver certificates;
- full L2Core soundness today;
- Rust interpreter correctness today;
- native codegen correctness today;
- Rust compiler/std, LLVM + MLIR, DuckDB, Arrow, or ABI correctness;
- broader format, engine, distribution, signing, remote-fetch, or encryption
  support.

Phase 36 also does not redefine MVP1. MVP1 remains a bounded executable baseline;
MVP1.5 adds lineage for the safety story in later phases.

## Downstream Phase Handoff

| Phase | Must Consume From This Contract | Must Not Claim Until Done |
|---|---|---|
| Phase 37 | Evidence layer names, Lean<->Rust verifier seam, claim/status taxonomy. | Operational soundness or real-executor consistency. |
| Phase 38 | `static<->dynamic` seam and modeled-executor safety target. | Rust interpreter/native correctness. |
| Phase 39 | `modeled-executor<->real-executor` seam and differential validation layer. | Native/model equivalence or compiler correctness. |
| Phase 40 | Native/model validation obligation and permanent compiler/toolchain TCB. | Closing MLIR/LLVM/Rust+C++ TCB by implication. |
| Phase 41 | All evidence-layer names and TCB rows for the combined lineage record. | Any "verified" row without a backing layer or TCB assignment. |

Future plans must cite this file when they use the word "verified" in an
MVP1.5 context.
