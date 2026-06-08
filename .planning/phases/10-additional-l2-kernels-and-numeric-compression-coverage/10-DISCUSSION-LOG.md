# Phase 10: Additional L2 Kernels and Numeric Compression Coverage - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-08
**Phase:** 10-additional-l2-kernels-and-numeric-compression-coverage
**Areas discussed:** Kernel target selection, Kernel ABI and params shape, Oracle and fixture strategy, User-visible surface

---

## Kernel Target Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Delta-of-delta integers | Lower integration risk and close to existing integer paths. | |
| ALP float | Stronger proof of L2 as a real compute-kernel layer. | ✓ |
| Minimal synthetic numeric kernel | Fastest path but weak final-design proof. | |

**User's choice:** ALP float.
**Notes:** User asked which choice best serves Loom's final goal. The recommendation was ALP because it better represents computation that L1 cannot declare. User confirmed ALP.

| Option | Description | Selected |
|--------|-------------|----------|
| ALP-first with delta fallback | Try ALP first; use delta fallback only if ALP is blocked. | ✓ |
| ALP-only | Keep ALP even if it becomes a research-only phase. | |
| ALP spike first | Make the whole phase a feasibility spike. | |

**User's choice:** ALP-first with delta fallback.
**Notes:** Fallback is allowed, but not preferred.

| Option | Description | Selected |
|--------|-------------|----------|
| Float64 only | Lower scope, one output type. | |
| Float32 + Float64 | Broader numeric coverage. | ✓ |
| Follow Vortex fixture | Let research pick whichever is easiest to construct. | |

**User's choice:** Float32 + Float64.

| Option | Description | Selected |
|--------|-------------|----------|
| Time-boxed API risk | Fallback if ALP API/oracle is unstable. | |
| Only hard compile blocker | Continue ALP unless API/compile path is actually blocked. | ✓ |
| No fallback after planning | Treat ALP risk as a blocker. | |

**User's choice:** Only hard compile/API blocker.

---

## Kernel ABI and Params Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated `AlpParams` struct | Clear, verifier-readable, like `FsstParams`. | ✓ |
| Generic L2 params envelope | More future-proof but likely premature. | |
| Minimal ad hoc bytes | Fastest but weaker diagnostics and maintainability. | |

**User's choice:** Dedicated `AlpParams` struct.

| Option | Description | Selected |
|--------|-------------|----------|
| FSST=0, ALP=1, delta fallback=2 | Append-only IDs preserving existing fixtures. | ✓ |
| Reserve ranges by family | More spec-like, higher design overhead. | |
| Named registry internally, numeric ID on wire | More readable but changes registry model. | |

**User's choice:** FSST=0, ALP=1, delta fallback=2.

| Option | Description | Selected |
|--------|-------------|----------|
| Params carry Float32/Float64 | Verifier can check params vs layout dtype. | ✓ |
| Only `LayoutDescription` carries type | Smaller params but trait cannot infer dtype. | |
| Split kernel IDs by type | Simple but expands ID space. | |

**User's choice:** `AlpParams` carries output type.

| Option | Description | Selected |
|--------|-------------|----------|
| Keep current trait | `decode(params, count) -> ArrayData`; smallest change. | ✓ |
| Add expected dtype to trait | More explicit but touches every kernel/call site. | |
| Add metadata method only | Medium abstraction, possibly future work. | |

**User's choice:** Keep current `L2Kernel` trait.

---

## Oracle and Fixture Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Vortex-native oracle | Strongest project-consistent proof. | |
| Synthetic known-value fixtures | Faster but weaker real-encoding proof. | |
| Dual oracle | Vortex oracle plus synthetic edge cases. | ✓ |

**User's choice:** Dual oracle.

| Option | Description | Selected |
|--------|-------------|----------|
| Exact bit equality where possible + documented tolerance fallback | Strict by default, tolerance only when justified. | ✓ |
| Always epsilon compare | Stable but may hide decode errors. | |
| Decimal-string compare through CLI/DuckDB | Too indirect for core correctness. | |

**User's choice:** Exact bit equality where possible, documented fixed tolerance only if needed.

| Option | Description | Selected |
|--------|-------------|----------|
| Add SQL gate | ALP fixtures must work through DuckDB. | ✓ |
| Rust + CLI only | Smaller but weaker end-to-end proof. | |
| Optional, only if ALP succeeds | SQL not required for fallback. | |

**User's choice:** Add SQL gate.

| Option | Description | Selected |
|--------|-------------|----------|
| Small representative matrix | Normal decimals, negatives, zero, repeats, nulls. | ✓ |
| Edge-heavy matrix | Include NaN/Inf/subnormal and extremes. | |
| Normal finite only | Stable but less representative. | |

**User's choice:** Small representative matrix.

---

## User-Visible Surface

| Option | Description | Selected |
|--------|-------------|----------|
| Kernel name + output type + params summary | Reviewer-readable without noisy dumps. | ✓ |
| Keep current `params_bytes` only | Minimal but not informative. | |
| Verbose ALP params dump | Useful for debugging but noisy. | |

**User's choice:** Kernel name + output type + params summary.

| Option | Description | Selected |
|--------|-------------|----------|
| Concise Phase 10 README section | Public docs show the new L2 kernel and commands. | ✓ |
| Release notes only | Planning docs only. | |
| Full design update | More complete but too broad for implementation closeout. | |

**User's choice:** Concise README / README-zh Phase 10 section.

| Option | Description | Selected |
|--------|-------------|----------|
| Stable plain decimal | Human-readable finite float output; NULL unchanged. | ✓ |
| Debug-style exact representation | More exact but less readable. | |
| SQL-focused only | No CLI float decode support. | |

**User's choice:** Stable plain decimal.

| Option | Description | Selected |
|--------|-------------|----------|
| No ALP timing in Phase 10 | Avoid performance messaging. | ✓ |
| Illustrative timing only | Similar to Phase 7 timing output. | |
| Timing if already cheap | Opportunistic addition. | |

**User's choice:** No ALP timing in Phase 10.

---

## the agent's Discretion

- Exact Rust module layout, `AlpParams` field encoding, verifier diagnostic code names, and test file organization.

## Deferred Ideas

- Generic L2 params envelope.
- Named registry with numeric wire IDs.
- ALP timing output.
- NaN/Infinity/subnormal edge suite.
- MLIR/native lowering.
- Formal verifier.
