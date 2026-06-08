# Phase 9: Verifier and Safety Boundary MVP - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-08
**Phase:** 09-verifier-and-safety-boundary-mvp
**Areas discussed:** Verifier boundary, diagnostics, CLI visibility, negative fixtures, FFI/DuckDB boundary, pending todo handling

---

## Verifier Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Structural verifier | Cheap pre-decode validation, with deeper checks delegated to authoritative decode paths | ✓ |
| Deep semantic verifier | Broader semantic validation before decode, risking duplicate logic | |
| Formal verifier | Totality/termination proof direction | |

**User's choice:** Follow recommended option.
**Notes:** Phase 9 is first-pass structural verification only.

---

## Diagnostics

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse `LoomDecodeError` only | Minimal new surface, but weak path/code information | |
| Lightweight verifier diagnostics | Stable code, human message, and recursive path | ✓ |
| Rich diagnostic framework | More complete but larger than Phase 9 needs | |

**User's choice:** Follow recommended option.
**Notes:** Failures are errors in Phase 9; warning severity can wait.

---

## CLI Visibility

| Option | Description | Selected |
|--------|-------------|----------|
| `loom inspect` default status | Show `verification: pass|fail` in existing reviewer command | ✓ |
| Dedicated command only | Add separate verifier command, less visible to reviewers | |
| JSON output | Machine-readable output, deferred for now | |

**User's choice:** Follow recommended option.
**Notes:** JSON output is deferred.

---

## Negative Fixtures

| Option | Description | Selected |
|--------|-------------|----------|
| Curated negative fixtures | Hand-selected malformed cases mapped to requirements | ✓ |
| Fuzzing | Broader malformed generation, larger scope | |
| Minimal smoke only | Too weak for safety-boundary proof | |

**User's choice:** Follow recommended option.
**Notes:** Include truncated payloads, count mismatches, run-end issues, unknown kernels, unsupported combinations, and table mismatches.

---

## FFI and DuckDB Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Rust/FFI ingress validation | Decode helpers and FFI path verify before Arrow export where practical | ✓ |
| CLI/test only | Leaves DuckDB relying only on decode failure | |
| New DuckDB verifier API | More surface area than Phase 9 needs | |

**User's choice:** Follow recommended option.
**Notes:** DuckDB benefits through existing `loom_decode`; no new C++ verifier API required.

---

## Pending Todo Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Fold into Phase 9 | Audit and close/update stale FOR-over-non-BitPack warning | ✓ |
| Defer | Keep warning pending for later | |
| Ignore | Risk misleading future planning | |

**User's choice:** Follow recommended option.
**Notes:** The todo appears stale after Phase 4, but Phase 9 should verify and close/update it explicitly.

---

## the agent's Discretion

- Exact module names, test file layout, and diagnostic naming.
- Whether existing decode-time checks are invoked directly or documented as authoritative for a given invariant.

## Deferred Ideas

- Formal totality/termination verifier.
- Non-termination safety demo.
- Fuzzing/property-based malformed input generation.
- Machine-readable verifier output such as JSON.
