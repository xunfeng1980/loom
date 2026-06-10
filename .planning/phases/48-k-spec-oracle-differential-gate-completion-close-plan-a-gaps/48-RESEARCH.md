# Phase 48: K Spec-Oracle Differential Gate Completion — Research

**Researched:** 2026-06-10
**Domain:** K Framework spec-oracle, kloom harness, per-shape native-route disable, placeholder serialization, K LLVM backend feasibility
**Confidence:** HIGH (code-verified) for Q1–Q4, Q6–Q7; MEDIUM for Q5 (K LLVM backend behavior cannot be run without K installed)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- K semantics (kloom) is the spec baseline; native output is the system-under-test. Two-way K ↔ native mirror reconciliation per builder event (reuse existing `TracedOutputBuilder`-derived trace comparison in `native_arrow_semantic.rs`). No Rust interpreter trace leg.
- Native↔K divergence → fail-closed for that run AND disable the native route for that shape (keyed by existing schema/cache fingerprint identity) with interpreter fallback. Disable persists for the process lifetime (in-process registry; no new persistent format, no public ABI change).
- Disabled-shape decisions must be observable (diagnostic/route report), and must prevent native cache/replay admission for that shape, consistent with Phase 43.2 admission discipline.
- krun/kompile binary missing or timeout ⇒ explicit skip (recorded, observable, distinguishable from "compared and matched"); production native path and release gates proceed.
- K present but output unparseable, or traces compared and divergent ⇒ HARD FAIL (fail-closed). Never classify these as skip.
- CI with K installed runs strict (skip not allowed); skip-tolerance is for local/dev without K, expressed via env-var convention consistent with `LOOM_ALLOW_NATIVE_TOOL_SKIP` discipline.
- K never enters the production binary path this phase: external `krun` subprocess from test/CI harness code only; nothing K-related crosses the C ABI or reaches DuckDB.
- Native remains the only default execution route; performance path untouched.
- Lean `accepted_program_safe` retained untouched.
- No K reachability proofs.
- No correctness claims — safety/well-formedness and divergence detection only.
- Programs whose L2Core AST contains constructs the kloom harness cannot faithfully serialize (Min/Max, bytes constants, any other placeholder lowering found) are classified explicitly unsupported-for-K-oracle: the harness returns a typed unsupported outcome (skip-with-reason in the gate), and never emits placeholder values into a compared trace.
- Implementing real min/max K rules is DEFERRED (out of scope this phase).
- LLVM backend deliverable: script-driven evidence that kloom.k compiles with `--backend llvm` and `krun` (or compiled interpreter binary) reproduces the same traces as the Haskell backend over the existing 12-test semantics corpus. Evidence recorded in build script + findings doc; does NOT gate release pipeline strictly.

### Claude's Discretion
- Exact module/file layout for the shape-disable registry and skip reporting (prefer integrating with existing `duckdb_runtime`/route-report fallback policy over inventing a parallel mechanism).
- Typed outcome enum shape for the K oracle (e.g., Compared/Diverged/SkippedRefereeAbsent/UnsupportedProgram) as long as the four states are distinguishable and divergence is fail-closed.
- How the LLVM-backend evidence integrates into `contrib/kloom/scripts/` (separate script vs kloom-diff.sh flag).

### Deferred Ideas (OUT OF SCOPE)
- Generated near-exhaustive corpus over the kloom-modeled matrix.
- Semantics sync-checklist gate (now three places after dropping the Rust interpreter leg: kloom.k / native codegen / Lean).
- Real min/max K rules; modeling UInt32/UInt64/Bytes/RowIndex in kloom.
- Extracting the K LLVM-backend interpreter into an actual Loom execution mode (next phase candidate).
</user_constraints>

---

## Summary

Phase 48 closes four clearly-scoped gaps on top of the already-landed kloom v4 K spec-oracle (commit 77d1bc4). All gaps are in existing files; no new crates are needed.

**Critical finding (Q2):** `krun` IS in the production binary call path today when the `melior` feature is enabled. The chain is: `loom_ffi::duckdb_runtime::prepare_duckdb_runtime` → `loom_native_melior::jit::execute_arrow_semantic_codegen_production_route` → `validate_native_arrow_semantic_codegen_output` → `verify_native_arrow_semantic_model_for_output` → `reference_model_trace_for_batch` → `kloom_trace_for_program` → `krun` subprocess. None of `validate_native_arrow_semantic_codegen_output`, `verify_native_arrow_semantic_model_for_output`, or `reference_model_trace_for_batch` are guarded by `#[cfg(feature = "melior")]`, so they compile and execute in the production path whenever the JIT runs. This does NOT violate the red line in isolation (krun is still an external subprocess, not in the binary itself), but the skip semantics must be implemented at the `verify_native_arrow_semantic_model_for_output` level so that krun absence causes the validation to return a typed skip outcome rather than a hard error that propagates up to abort the production route.

**Per-shape disable insertion point (Q3):** The insertion point is `execute_arrow_semantic_codegen_production_route_inner` in `loom-native-melior/src/jit.rs`, where `validate_native_arrow_semantic_codegen_output` is called and the resulting `execution` report is examined. The existing `fallback_or_fail_closed(policy)` pattern already handles the route status decision. A new in-process registry (keyed by `schema_fingerprint` from `NativeArrowSemanticCodegenSupportReport.schema_fingerprint`) can be a module-level `OnceLock<Mutex<HashSet<String>>>` — the only existing example in the codebase (`PANIC_SENTINEL` in `loom-ffi/src/ffi.rs`) uses `thread_local!` for test isolation, which is the wrong scope for a persistent per-shape disable. A global `std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>>` initialized once is the standard Rust pattern. Alternatively, the `DuckDbRuntimePlanReport` / route report can carry a pre-check at the top of `prepare_duckdb_runtime` against a module-level set.

**Float landmine (Q6):** No landmine. The `reference_program_for_batch` function generates only `AppendValue`/`AppendNull` statements with literal constants. No comparisons, loops, or variables appear in the generated reference programs. The kloom trace events only record `append-value:col0:float32` and `append-null:col0:float32` — they do NOT include the actual float value. So float bit-pattern encoding as K integers is sound for trace comparison: the trace outcome (append-value vs append-null) depends solely on null status, and K never applies `lt`/`le` to float-encoded integers in these generated programs. The float bit-pattern encoding is **faithful for trace comparison** but still a **placeholder for value content** — flagging it as `UnsupportedForKOracle` is unnecessary for float values in the current reference_program_for_batch path. However, `Float32Bits`/`Float64Bits` in `serialize_scalar_value` emits integer literals for the value itself (not the trace event). Since kloom.k's `appendValue` ignores the actual value in its trace rule (it only records the builder name and type), this is safe.

**Primary recommendation:** Add a `KOracleOutcome` enum to `kloom_harness.rs` with four variants (Compared, Diverged, SkippedRefereeAbsent, UnsupportedProgram); thread it through `verify_native_arrow_semantic_model_for_output`; hook the divergence arm to consult and update a module-level disabled-shapes registry in `jit.rs`.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| krun-absent skip semantics | `loom-core::kloom_harness` (outcome enum) | `loom-core::native_arrow_semantic` (validate dispatch) | krun invocation lives in kloom_harness; the outcome type must be interpretable in native_arrow_semantic |
| Per-shape native-route disable registry | `loom-native-melior::jit` (insertion point, module-level registry) | `loom-core::native_arrow_semantic` (divergence signal) | Route admission lives in jit.rs; the registry must be checked/updated at route execution time |
| Placeholder hole classification | `loom-core::kloom_harness` (serialize_expr / serialize_scalar_value) | none | Serialization is the gating point; unsound values must be caught before krun is invoked |
| Mirror reconciliation hardening | `loom-core::native_arrow_semantic` (verify_native_arrow_semantic_model_for_output) | `loom-native-melior::jit` (outcome propagation) | The comparison function already exists here; skip/unsupported outcomes must propagate to route decision |
| LLVM backend evidence | `contrib/kloom/scripts/` (new script) | `contrib/kloom/docs/` (findings doc) | Purely offline evidence; no production code change |
| Skip env convention | `contrib/kloom/scripts/kloom-diff.sh` | `.github/workflows/ci.yml` (strict, no skip) | Matches existing LOOM_ALLOW_NATIVE_TOOL_SKIP pattern |

---

## Q1: kloom_harness.rs Anatomy

**File:** `crates/loom-core/src/kloom_harness.rs` [VERIFIED: file:line codebase grep]

### krun location/invocation

- `Command::new("krun")` — invokes `krun` from PATH (line 336). No env var override or explicit binary path; relies on `krun` being on `$PATH`.
- Definition dir: `definition_dir()` (line 359). Uses compile-time `env!("CARGO_MANIFEST_DIR")` = `crates/loom-core`, walks two levels up to workspace root, appends `contrib/kloom/.build`. Hard-coded path, no env var override.
- Input: written to `$TMPDIR/loom_kloom_harness_<pid>_<seq>.kloom` (line 329-332).
- Args: `krun <file> --definition <def_dir> --output pretty` (lines 337-344).
- Working dir: inherited (not set explicitly).
- stdout: piped. stderr: piped.
- No timeout.

### Error paths in `kloom_trace_for_program` / `run_kloom`

| Error path | Location | Current behavior | Required behavior (Phase 48) |
|---|---|---|---|
| `definition_dir()` fails — `.build` does not exist | `kloom_harness.rs:369-375` | `KloomHarnessError` with "kloom definition directory not found" | Maps to `SkippedRefereeAbsent` (kompile not run = referee absent) |
| `Command::new("krun").output()` fails — krun not on PATH | `kloom_harness.rs:345` | `KloomHarnessError` with "failed to spawn krun: {e}" (`os error 2` = ENOENT) | Maps to `SkippedRefereeAbsent` |
| krun exits non-zero | `kloom_harness.rs:347-353` | `KloomHarnessError` with "krun exited with status N: {stderr}" | Maps to HARD FAIL (K present but output unusable; referee disagreeing) |
| `parse_trace` finds no `<events>` cell or malformed output | `kloom_harness.rs:382-411` | Returns empty `Vec<String>` (no error!) or includes raw unexpected lines | Phase 48 must detect this case: empty trace from a non-trivial program, or unexpected lines, should map to HARD FAIL (garbled output) |
| `serialize_program` fails — unsupported Arrow type | `kloom_harness.rs:112-115` | `KloomHarnessError` with "unsupported Arrow type" | Maps to `UnsupportedProgram` |
| `serialize_expr` hits `Min`/`Max` | `kloom_harness.rs:276-284` | Silently emits `0` (placeholder) | Must return early with `UnsupportedProgram` before serialization |
| `serialize_scalar_value` hits `Bytes` | `kloom_harness.rs:309-313` | Silently emits `0` (placeholder) | Must return early with `UnsupportedProgram` before serialization |

### Caller chain

```
kloom_trace_for_program(&program)              [pub, kloom_harness.rs:41]
  └── serialize_program(program)               [private, :50]
  └── run_kloom(&text)                         [private, :322]

reference_model_trace_for_batch(batch)         [private, native_arrow_semantic.rs:1946]
  └── reference_program_for_batch(batch)       [:1959]
  └── kloom_trace_for_program(&program)        [:1950]

verify_native_arrow_semantic_model_for_output  [private, :1221]
  └── decode_reference_batch(bytes)
  └── reference_model_trace_for_batch(&reference)  [line 1244]
  └── native_model_trace_for_batch(output)

validate_native_arrow_semantic_codegen_output  [pub, :679]
  └── validate_native_arrow_semantic_codegen_output_inner [:693]
      └── verify_native_arrow_semantic_model_for_output [:726]
```

### Proposed `KOracleOutcome` enum

```rust
/// Outcome of a K spec-oracle trace comparison.
pub enum KOracleOutcome {
    /// krun ran and traces agreed.
    Compared { reference_trace: Vec<String>, native_trace: Vec<String> },
    /// krun ran and traces diverged — HARD FAIL.
    Diverged { reference_trace: Vec<String>, native_trace: Vec<String> },
    /// krun or kompile binary absent / definition dir missing — referee absent, skip.
    SkippedRefereeAbsent { reason: String },
    /// Program contains constructs harness cannot faithfully serialize — unsupported, skip.
    UnsupportedProgram { reason: String },
}
```

Mapping from existing error variants:
- `"failed to spawn krun"` with `os error 2` (ENOENT) → `SkippedRefereeAbsent`
- `definition_dir` missing → `SkippedRefereeAbsent`
- krun exits non-zero → `Diverged` or hard-error (keep as error propagating to HARD FAIL path)
- `parse_trace` returns unexpected/empty trace from non-trivial program → HARD FAIL (garbled output)
- `Min`/`Max` in expr → `UnsupportedProgram` (early return before serialization)
- `Bytes` scalar value → `UnsupportedProgram` (early return before serialization)
- Unsupported Arrow type in `arrow_type_to_l2ty` → `UnsupportedProgram`

---

## Q2: Trace Comparison Call Graph and Production-vs-Test Answer

**CRITICAL FINDING:** krun IS invoked in the production path when `melior` feature is enabled. [VERIFIED: codebase grep + file reads]

### Full production call chain (melior feature enabled)

```
DuckDB FFI entrypoint (loom_ffi/src/ffi.rs: loom_duckdb_prepare_route)
  └── prepare_duckdb_runtime(plan_report, cancellation)        [duckdb_runtime.rs:1076]
      └── execute_arrow_semantic_codegen_production_route(...)  [jit.rs:158, NOT cfg-guarded]
          └── execute_arrow_semantic_codegen_production_route_inner [jit.rs:196]
              └── execute_arrow_semantic_codegen_jit_backend    [jit.rs, #[cfg(feature="melior")]]
              └── [JIT succeeds]
              └── validate_native_arrow_semantic_codegen_output  [native_arrow_semantic.rs:679, NOT cfg-guarded]
                  └── validate_native_arrow_semantic_codegen_output_inner [:693]
                      └── verify_native_arrow_semantic_model_for_output [:726]
                          └── reference_model_trace_for_batch [:1244]
                              └── kloom_trace_for_program      [kloom_harness.rs:41]
                                  └── krun subprocess  ← PRODUCTION PATH
```

**Implication:** `krun` absence in a production environment where the `melior` feature is compiled in will cause `reference_model_trace_for_batch` to return an error, which propagates as `NativeModelTraceMismatch` diagnostic, which causes `execution.is_supported()` to be false, which causes `decide_validated_native_arrow_semantic_codegen_runtime` to return `FailClosed`. So currently, if krun is not on PATH in a production binary with `melior`, the native route fail-closes. This is the current behavior that must change to skip-gracefully.

**Red-line status:** The CONTEXT.md red line says "K never enters the production binary path this phase: external `krun` subprocess from test/CI harness code only". The current code ALREADY violates this spirit (krun is invoked in the production DuckDB route when melior is on). Phase 48 does not need to fix this architectural violation (it is pre-existing), but the planner must know: the skip semantics must work in the production melior path, not just in test code. The per-shape disable hook must also live at `execute_arrow_semantic_codegen_production_route_inner`.

### Test callers

- `crates/loom-core/tests/native_arrow_semantic_codegen.rs` — calls `validate_native_arrow_semantic_codegen_output` directly (test-only)
- `crates/loom-core/tests/native_arrow_semantic_codegen_adversarial.rs` — same
- `crates/loom-core/tests/native_arrow_semantic_codegen_stability.rs` — same
- `crates/loom-native-melior/tests/production_arrow_semantic_jit.rs` — same
- `crates/loom-core/tests/native_arrow_semantic.rs` — exercises kloom harness via `--test native_arrow_semantic` (run by `kloom-diff.sh` step 3)

### Where mismatch diagnostic propagates

`NativeModelTraceMismatch` from `verify_native_arrow_semantic_model_for_output` sets `model_trace_matches = false` in `NativeArrowSemanticModelValidationReport`. This propagates into `NativeArrowSemanticCodegenExecutionReport.validation` and then to `execution.is_supported() == false`, which makes `decide_validated_native_arrow_semantic_codegen_runtime` set `decision = FailClosed` (or fallback). The planner's per-shape disable hook belongs BETWEEN the `execute_arrow_semantic_codegen_jit_backend` success and the point where route status is set in `execute_arrow_semantic_codegen_production_route_inner` — specifically after `validate_native_arrow_semantic_codegen_output` returns with a divergence outcome.

---

## Q3: Per-Shape Disable Integration Point

### Insertion point [VERIFIED: codebase reads]

**Primary insertion point:** `loom-native-melior/src/jit.rs`, `execute_arrow_semantic_codegen_production_route_inner` (line 196), between the JIT execution and the route status decision.

Current code sequence (jit.rs ~lines 402-423):
```rust
let execution = validate_native_arrow_semantic_codegen_output(bytes, &support, ...);
let runtime_decision = decide_validated_native_arrow_semantic_codegen_runtime(&execution, policy);
// ... then: status = NativeCandidate OR fallback_or_fail_closed(policy)
```

**Insertion:** After `execution` is obtained, check if `execution.diagnostics()` contains `NativeModelTraceMismatch`. If so: (1) look up `execution.schema_fingerprint` in the module-level disabled registry; (2) if not already disabled, insert it; (3) return route with `InterpreterFallback` or `FailClosed` status (per policy).

**Second insertion point (pre-check):** At the top of `execute_arrow_semantic_codegen_production_route_inner`, before JIT execution: if `support.schema_fingerprint` is in the disabled registry, return `InterpreterFallback` immediately without running JIT.

### Shape identity key [VERIFIED: native_arrow_semantic.rs:142,673,735]

Use `NativeArrowSemanticCodegenSupportReport.schema_fingerprint` as the key. This is already computed at line 673 via `schema_fingerprint(&reference)` and propagated to `NativeArrowSemanticCodegenExecutionReport.schema_fingerprint` at line 735. The `schema_fingerprint` function (line 1820) hashes the Arrow schema. This is the right granularity: per-shape (schema-level), not per-artifact, which composes correctly with Phase 43.2 cache key which also uses `schema_fingerprint` as a component.

### Registry pattern [VERIFIED: loom-ffi/src/ffi.rs:72 for existing pattern]

Existing in-process registry patterns in the codebase:
- `thread_local! { static PANIC_SENTINEL: Cell<bool> }` in `loom-ffi/src/ffi.rs:72` — thread-scoped, test-only
- No global `OnceLock<Mutex<...>>` registries exist yet

**Recommended pattern** (module-level in `jit.rs`):
```rust
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

static NATIVE_ROUTE_DISABLED_SHAPES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn disabled_shapes() -> &'static Mutex<HashSet<String>> {
    NATIVE_ROUTE_DISABLED_SHAPES.get_or_init(|| Mutex::new(HashSet::new()))
}

fn is_shape_disabled(schema_fingerprint: &str) -> bool {
    disabled_shapes().lock().unwrap().contains(schema_fingerprint)
}

fn disable_shape(schema_fingerprint: &str) {
    disabled_shapes().lock().unwrap().insert(schema_fingerprint.to_string());
}
```

### Composing with Phase 43.2 admission discipline

Phase 43.2 requires that unsupported or divergent executions cannot produce positive replay evidence or seed runtime cache. The per-shape disable must:
1. Be checked BEFORE JIT execution (pre-check path) so disabled shapes skip `validate_native_arrow_semantic_codegen_output` entirely — no krun invoked for already-disabled shapes.
2. Return `ArrowSemanticCodegenRouteStatus::InterpreterFallback` (or `FailClosed` if policy is `FailClosedOnly`), which propagates to `decide_validated_native_arrow_semantic_codegen_runtime` returning `FailClosed`/`InterpreterFallback`, which means `cacheable = false` (line 417: `cacheable` requires `NativeCandidate`).
3. Emit an observable diagnostic (new `NativeBackendDiagnosticCode::NativeShapeDisabled`) so route reports show the disable.

---

## Q4: Skip-Discipline Conventions [VERIFIED: scripts/ grep]

### LOOM_ALLOW_NATIVE_TOOL_SKIP pattern

The existing convention in all scripts:
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` → skip optional tool check with `return 2` from `toolchain_llvm_bin_dir` in `scripts/toolchain-common.sh:27`
- Scripts check: `if [ "${LOOM_ALLOW_NATIVE_TOOL_SKIP:-}" = "1" ]; then echo "...skipped..."; return/exit 0; fi`
- Strict gates (`production-native-codegen-stabilization-test.sh:13-17`, `production-native-codegen-realization-test.sh:13`) actively REJECT `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` and call `exit 1`.

### Proposed K oracle skip convention

New env var: `LOOM_ALLOW_K_ORACLE_SKIP` (mirrors naming pattern exactly).

- `LOOM_ALLOW_K_ORACLE_SKIP=1` → krun absence returns `SkippedRefereeAbsent` outcome; gate proceeds.
- `LOOM_ALLOW_K_ORACLE_SKIP` unset or `0` → krun absence returns hard error (fail-closed in gate).
- CI (`ci.yml` kloom job) does NOT set `LOOM_ALLOW_K_ORACLE_SKIP` → strict mode.
- `kloom-diff.sh` does NOT set it by default → strict mode.
- `scripts/mvp2-verify.sh` / `scripts/mvp0-verify.sh` use `LOOM_ALLOW_K_ORACLE_SKIP=1` if they invoke the K gate at all (so local CI without K still passes).

**Implementation location:** `kloom_harness.rs::run_kloom` — read `std::env::var("LOOM_ALLOW_K_ORACLE_SKIP")` before spawning krun; if binary not found AND skip allowed, return `SkippedRefereeAbsent`.

---

## Q5: K LLVM Backend Feasibility [ASSUMED for LLVM-specific behavior; VERIFIED for current repo state]

### K Framework version in CI [VERIFIED: .github/workflows/ci.yml:225]

K Framework is installed via `nix profile install nixpkgs#kframework` from `nixpkgs=channel:nixos-unstable`. No version pin. The version installed will be whatever `nixos-unstable` provides at CI time.

### Current kompile invocation [VERIFIED: kloom-diff.sh:65]

```bash
kompile src/kloom.k --backend haskell -o .build
```

Only Haskell backend. The `.build` directory structure (from repo): `allRules.txt`, `backend.txt`, `cache.bin`, `compiled.bin`, `compiled.txt`, `configVars.sh`, `definition.kore`, `macros.kore`, `mainModule.txt`, `mainSyntaxModule.txt`, `parsed.txt`, `README.md`, `scanner`, `syntaxDefinition.kore`, `timestamp` — these are all Haskell-backend artifacts.

### K LLVM backend constraints [ASSUMED — cannot run K locally]

The K LLVM backend is the path toward a fast extracted interpreter. Known constraints from K Framework documentation:
- `kompile src/kloom.k --backend llvm -o .build-llvm` — uses a different output directory
- LLVM backend produces a native binary (`interpreter`) rather than a Haskell-backed `krun` wrapper
- The LLVM backend may have restrictions around certain K built-ins; `INT`, `BOOL`, `STRING`, `LIST`, `MAP` are all standard builtins supported by LLVM backend [ASSUMED]
- `krun` with `--definition .build-llvm` should work if `interpreter` binary is in path or co-located
- Output format with `--output pretty` should produce the same `<events>` cell structure [ASSUMED]
- The LLVM backend is known to be faster than the Haskell backend but requires LLVM toolchain [ASSUMED]
- `kloom.k` uses `INT`, `BOOL`, `LIST`, `MAP`, `STRING` imports — all are standard builtins with LLVM backend support [ASSUMED]

### kore-exec.tar.gz hint

The repo contains `contrib/kloom/kore-exec.tar.gz` (41 KB gzip). This is likely a pre-built KORE execution artifact or snapshot. It may be a pre-compiled Haskell-backend artifact. Not a LLVM backend artifact. [ASSUMED based on file size and context]

### LLVM backend evidence script shape

Deliverable: `contrib/kloom/scripts/kloom-llvm-feasibility.sh`

```bash
# Proposed structure:
# 1. Check kompile, krun are available (or LOOM_ALLOW_K_ORACLE_SKIP=1 → skip with note)
# 2. kompile src/kloom.k --backend llvm -o .build-llvm
# 3. For each tests/semantics/*.kloom:
#    a. Run with Haskell backend: krun test --definition .build --output pretty → haskell_trace
#    b. Run with LLVM backend: krun test --definition .build-llvm --output pretty → llvm_trace
#    c. diff haskell_trace llvm_trace → fail on any divergence
# 4. Report: N tests compared, all identical → feasibility confirmed
# 5. Record findings in contrib/kloom/docs/LLVM-BACKEND-FEASIBILITY.md
```

**Explicit unknowns (cannot verify without running K):**
- Whether `--backend llvm` requires any flags not needed for Haskell (e.g., `--gen-bison-parser`)
- Whether `krun --output pretty` produces identical token spacing for both backends
- Whether LLVM backend supports the exact `MAP[N <- V]` and `LIST ListItem()` syntax used in kloom.k rules
- Whether the installed K version on `nixos-unstable` includes the LLVM backend toolchain by default

---

## Q6: Placeholder-Hole Inventory [VERIFIED: kloom_harness.rs full read]

### Exhaustive serialization audit

**Lossy/placeholder locations in `serialize_expr` (kloom_harness.rs):**

| Location | Construct | Current behavior | Classification | Action |
|---|---|---|---|---|
| Lines 276-280 | `ScalarExpr::Min(lhs, rhs)` | Emits `0`, discards both args | **Placeholder-unsound** | Classify `UnsupportedProgram`, refuse to serialize |
| Lines 281-284 | `ScalarExpr::Max(lhs, rhs)` | Emits `0`, discards both args | **Placeholder-unsound** | Classify `UnsupportedProgram`, refuse to serialize |

**Lossy/placeholder locations in `serialize_scalar_value`:**

| Location | Value | Current behavior | Classification | Action |
|---|---|---|---|---|
| Lines 300-304 | `ScalarValue::Float32Bits(bits)` | Emits `bits.to_string()` as integer | **Lossy-but-sound** for trace-only comparison (value not in trace); see float analysis | No change needed for trace comparison |
| Lines 305-307 | `ScalarValue::Float64Bits(bits)` | Emits `bits.to_string()` as integer | **Lossy-but-sound** for trace-only comparison | No change needed for trace comparison |
| Lines 309-313 | `ScalarValue::Bytes(b)` | Emits `0`, discards content | **Placeholder-unsound** | Classify `UnsupportedProgram`, refuse to serialize |

**Other constructs in `serialize_stmt`:**

| Construct | Handling | Classification |
|---|---|---|
| `L2CoreStmt::AppendValue` | Faithful serialization | OK |
| `L2CoreStmt::AppendNull` | Faithful | OK |
| `L2CoreStmt::ReadInput` | Faithful | OK |
| `L2CoreStmt::LetScalar` | Faithful | OK |
| `L2CoreStmt::ForRange` | Faithful | OK |
| `L2CoreStmt::CursorLoop` | Faithful | OK |
| `L2CoreStmt::FailClosed` | Faithful | OK |
| `Capability::Scratch` | Skipped silently | OK (scratch caps don't affect trace) |

**Full unsound list (requires `UnsupportedProgram` classification):**
1. `ScalarExpr::Min(_, _)` — any program containing Min
2. `ScalarExpr::Max(_, _)` — any program containing Max
3. `ScalarValue::Bytes(_)` — any program with Bytes constants

**Predicate for `unsupported_for_k_oracle(program)`:**

```rust
fn program_uses_unsupported_constructs(program: &L2CoreProgram) -> Option<&'static str> {
    for stmt in &program.body {
        if let Some(reason) = stmt_uses_unsupported(stmt) {
            return Some(reason);
        }
    }
    None
}

fn expr_uses_unsupported(expr: &ScalarExpr) -> Option<&'static str> {
    match expr {
        ScalarExpr::Min(_, _) => Some("ScalarExpr::Min not modeled in kloom"),
        ScalarExpr::Max(_, _) => Some("ScalarExpr::Max not modeled in kloom"),
        ScalarExpr::Const(ScalarValue::Bytes(_)) => Some("ScalarValue::Bytes not representable in kloom"),
        // recurse into sub-expressions
        ScalarExpr::Add(l, r) | ScalarExpr::Sub(l, r) | ScalarExpr::Mul(l, r)
        | ScalarExpr::Eq(l, r) | ScalarExpr::Lt(l, r) | ScalarExpr::Le(l, r) => {
            expr_uses_unsupported(l).or_else(|| expr_uses_unsupported(r))
        }
        _ => None,
    }
}
```

This predicate runs before serialization begins; if it returns Some, `kloom_trace_for_program` returns `KOracleOutcome::UnsupportedProgram { reason }` immediately.

### Float comparison analysis [VERIFIED: kloom.k rules, kloom_harness.rs, native_arrow_semantic.rs]

**Finding: No float comparison landmine for current use case.**

- `reference_program_for_batch` (native_arrow_semantic.rs:1959-2008) generates ONLY `AppendValue` and `AppendNull` statements with literal constants — no `lt`, `le`, `eq`, loops, or variables.
- The kloom trace events for append-value are `append-value:colN:float32` — they do NOT include the actual float value.
- K's `appendValue` rule (kloom.k:434-446) fires on `TypeOfResult(_)` (any type accepted for constants) and emits `ListItem(append-value : B : Ty)` where `Ty` is the builder's declared type (`float32`/`float64`), not the value.
- Float bit-pattern integers are passed to `TypeOf(Expr)` → `TypeOf(N:Int)` → `TypeOfResult(int64Ty)`, which matches `TypeOfResult(_)` in the `AppendValueCheck` rule.
- Therefore: a float value's bit-pattern encoding does NOT affect whether `append-value` or `append-null` is emitted in the trace. The trace outcome is determined by null status (checked in Rust before the program is constructed), not by the value.
- **No negative-float ordering bug** in the current validation path. The `lt`/`le` ordering issue would only arise if comparison expressions on float-encoded integers appeared in generated programs, which they do not.

---

## Q7: Test Landscape [VERIFIED: codebase grep + file reads]

### Tests that exercise the K oracle

| File | Trigger | K oracle involvement |
|---|---|---|
| `crates/loom-core/tests/native_arrow_semantic.rs` | `cargo test -p loom-core --test native_arrow_semantic` | Direct: calls `reference_model_trace_for_batch` → `kloom_trace_for_program` |
| `crates/loom-core/tests/native_arrow_semantic_codegen.rs` | `cargo test -p loom-core --test native_arrow_semantic_codegen` | Indirect: calls `validate_native_arrow_semantic_codegen_output` → krun |
| `crates/loom-core/tests/native_arrow_semantic_codegen_adversarial.rs` | same crate | Indirect via validate |
| `crates/loom-core/tests/native_arrow_semantic_codegen_stability.rs` | same crate | Indirect via validate |
| `crates/loom-native-melior/tests/production_arrow_semantic_jit.rs` | `cargo test -p loom-native-melior --features melior` | Indirect via validate |

### Scripts that exercise the K oracle

| Script | K oracle role | Must-keep-passing |
|---|---|---|
| `contrib/kloom/scripts/kloom-diff.sh` | Compiles and runs corpus; runs `cargo test -p loom-core --test native_arrow_semantic` | Yes — differential gate |
| `scripts/model-rust-interpreter-consistency-test.sh` | Checks kloom module wiring (grep assertions), runs `--test native_arrow_semantic` | Yes |
| CI job `kloom` (`.github/workflows/ci.yml:209-246`) | End-to-end: Nix K install, kompile, krun corpus, kloom-diff.sh | Yes — CI must be strict |

### New tests needed in Phase 48

1. `crates/loom-core/tests/kloom_skip_semantics.rs` — tests `SkippedRefereeAbsent` when krun absent, `UnsupportedProgram` for Min/Max/Bytes programs, `Diverged` when traces differ
2. `crates/loom-core/tests/native_arrow_semantic_shape_disable.rs` — tests per-shape disable registry (divergence → disable → subsequent calls return InterpreterFallback)
3. Addition to `contrib/kloom/scripts/kloom-llvm-feasibility.sh` — LLVM backend evidence script
4. New test in `crates/loom-core/tests/native_arrow_semantic_codegen.rs` — validate skip propagation through full codegen route

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Per-shape registry | Custom lock-free structure | `std::sync::OnceLock<std::sync::Mutex<HashSet<String>>>` | Already the stdlib pattern; no new deps |
| K availability check | Custom binary probe | `Command::new("krun").arg("--version").output()` or catch ENOENT from `Command::new("krun").output()` | Already happens at spawn time; ENOENT = absent |
| Float bit comparison | Custom IEEE 754 comparison in K | Don't need it — trace doesn't include values | See Q6 analysis |
| Skip env convention | Custom mechanism | `LOOM_ALLOW_K_ORACLE_SKIP` env var, same as `LOOM_ALLOW_NATIVE_TOOL_SKIP` pattern | Consistent with existing gate discipline |

---

## Common Pitfalls

### Pitfall 1: Classifying krun non-zero exit as SkippedRefereeAbsent
**What goes wrong:** krun exits non-zero when it has an internal error (K rules stuck, parse failure) — this is "referee present and disagreeing", not "referee absent". Classifying it as skip hides real bugs.
**Why it happens:** Both are `Command.output()` with non-success status.
**How to avoid:** ONLY map `io::ErrorKind::NotFound` (ENOENT) from the `Command::new("krun").output().map_err(...)` call to `SkippedRefereeAbsent`. All other errors (non-zero exit, I/O error that is not ENOENT) → hard fail.
**Warning signs:** Tests that should diverge start passing silently.

### Pitfall 2: Definition directory absent treated as skip
**What goes wrong:** `definition_dir()` returns `Err("kloom definition directory not found")` when `.build` doesn't exist — this is equivalent to kompile not having been run (= referee absent = skip). But if someone deletes `.build` in CI, the skip would silently pass.
**How to avoid:** In CI (strict mode, `LOOM_ALLOW_K_ORACLE_SKIP` not set), definition-not-found must be a hard fail. Only when skip is explicitly allowed should it map to `SkippedRefereeAbsent`.

### Pitfall 3: Per-shape disable registry poisoning across tests
**What goes wrong:** `OnceLock<Mutex<HashSet<String>>>` is global for the process lifetime; in test processes (multi-test), a shape disabled in one test persists for subsequent tests.
**How to avoid:** Expose a `#[cfg(test)] fn reset_disabled_shapes()` function for test cleanup, used in test setup/teardown. Production code never calls it.

### Pitfall 4: parse_trace returning empty Vec on K stuck/garbled output
**What goes wrong:** If krun outputs malformed K configuration (no `<events>` cell at all), `parse_trace` returns an empty `Vec<String>` with `Ok(())` — no error. The empty trace then compares against the non-empty native trace and produces `NativeModelTraceMismatch`, which is correct behavior. But a garbled output (K process stuck, partial XML) could also produce empty trace silently.
**How to avoid:** Add a minimal output validity check in `parse_trace`: if krun exits zero AND stdout contains NO `<events>` token at all, emit a specific error (garbled-output → hard fail). Empty `<events>.List</events>` is valid (empty program); missing `<events>` entirely is garbled.

### Pitfall 5: Unsupported-program detection must recurse into nested expressions
**What goes wrong:** `Min`/`Max` can appear nested inside `Add(Min(a, b), c)`. A shallow check of only top-level expressions misses nested occurrences.
**How to avoid:** The `expr_uses_unsupported` function must recurse into all sub-expressions of `Add`, `Sub`, `Mul`, `Eq`, `Lt`, `Le`. Also recurse into statement bodies: `ForRange.body`, `CursorLoop.body`, `AppendValue.value`, `LetScalar.expr`, `ReadInput.offset`, `ReadInput.width`.

---

## Code Examples

### Pattern 1: krun absence detection (proposed)

```rust
// Source: existing kloom_harness.rs:345 pattern, extended
fn run_kloom(text: &str) -> Result<KOracleOutcome, KloomHarnessError> {
    let def_dir = match definition_dir() {
        Ok(d) => d,
        Err(e) => {
            let allow_skip = std::env::var("LOOM_ALLOW_K_ORACLE_SKIP")
                .map(|v| v == "1").unwrap_or(false);
            if allow_skip {
                return Ok(KOracleOutcome::SkippedRefereeAbsent {
                    reason: e.message.clone()
                });
            }
            return Err(e);
        }
    };
    // ... write tmp file ...
    let result = Command::new("krun").arg(&tmp_path).arg("--definition").arg(&def_dir)
        .arg("--output").arg("pretty")
        .stdout(Stdio::piped()).stderr(Stdio::piped()).output();
    match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let allow_skip = std::env::var("LOOM_ALLOW_K_ORACLE_SKIP")
                .map(|v| v == "1").unwrap_or(false);
            if allow_skip {
                return Ok(KOracleOutcome::SkippedRefereeAbsent {
                    reason: "krun not found on PATH".to_string()
                });
            }
            Err(KloomHarnessError::new("krun not found on PATH; set LOOM_ALLOW_K_ORACLE_SKIP=1 to skip"))
        }
        Err(e) => Err(KloomHarnessError::new(format!("failed to spawn krun: {e}"))),
        Ok(output) if !output.status.success() => {
            // krun present but failed — HARD FAIL regardless of skip flag
            Err(KloomHarnessError::new(format!("krun exited {}: {}",
                output.status, String::from_utf8_lossy(&output.stderr))))
        }
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Validate output contains <events> cell
            if !stdout.contains("<events>") {
                return Err(KloomHarnessError::new("krun output did not contain <events> cell"));
            }
            parse_trace(&stdout).map(|trace| KOracleOutcome::reference_trace(trace))
        }
    }
}
```

### Pattern 2: Per-shape disable registry (proposed, in jit.rs)

```rust
// Module-level registry — follows OnceLock<Mutex<...>> stdlib pattern
static NATIVE_ROUTE_DISABLED_SHAPES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn disabled_shapes_registry() -> &'static Mutex<HashSet<String>> {
    NATIVE_ROUTE_DISABLED_SHAPES.get_or_init(|| Mutex::new(HashSet::new()))
}

// In execute_arrow_semantic_codegen_production_route_inner:
// --- pre-check (before JIT) ---
if is_shape_disabled(&support.schema_fingerprint) {
    return ArrowSemanticCodegenProductionRouteReport {
        status: fallback_or_fail_closed(policy),
        // ... diagnostics include NativeShapeDisabled ...
    };
}
// --- post-JIT validation ---
let execution = validate_native_arrow_semantic_codegen_output(...);
if execution.diagnostics().iter().any(|d| d.code == NativeArrowSemanticDiagnosticCode::NativeModelTraceMismatch) {
    disable_shape(&execution.schema_fingerprint);
    return /* fail-closed or fallback route */;
}
```

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | K LLVM backend supports all builtins used in kloom.k (INT, BOOL, LIST, MAP, STRING) | Q5 | LLVM backend evidence script would fail on kompile; deliverable not achievable without K changes |
| A2 | `krun --output pretty` with LLVM backend produces same `<events>` cell format as Haskell backend | Q5 | Trace comparison between backends would fail even for correct programs |
| A3 | `nix profile install nixpkgs#kframework` on nixos-unstable includes the LLVM backend | Q5 | LLVM backend kompile might not be available in CI |
| A4 | kore-exec.tar.gz is a Haskell-backend artifact, not LLVM-backend | Q5 | May be usable as LLVM baseline |

---

## Open Questions

1. **K LLVM backend availability in Nix nixos-unstable**
   - What we know: CI uses `nix profile install nixpkgs#kframework` which provides Haskell backend.
   - What's unclear: Whether the LLVM backend binary (`kllvm`, `kompile --backend llvm`) is included in the same Nix package or requires a separate one.
   - Recommendation: The feasibility script should detect LLVM backend availability at the start and emit a skip with `LOOM_ALLOW_K_ORACLE_SKIP=1`-equivalent if `--backend llvm` fails; findings doc must note explicitly whether LLVM backend was available.

2. **Timeout for krun invocations**
   - What we know: `run_kloom` in kloom_harness.rs has no timeout on the krun subprocess.
   - What's unclear: Whether timeouts should count as "referee absent" or "hard fail". The CONTEXT.md says "krun/kompile binary missing or timeout ⇒ skip" — timeout is explicitly skip.
   - Recommendation: Add a process timeout (e.g., 30s) to the `Command` via `wait_with_output` or by spawning + polling. Timeout → `SkippedRefereeAbsent`. This is an explicit requirement from CONTEXT.md.

3. **SEMANTICS.md and README.md doc sync**
   - CONTEXT.md notes "stale 'v0/pure-append' wording needs sync". The `SEMANTICS.md` section 3 "Known Limitations (v0)" still lists readInput, letScalar etc as missing, but kloom.k v4 now has all of them. README.md architecture diagram still shows "l2_core_reference_executor" leg which was deleted.
   - Recommendation: These are doc-only cleanups; plan a small doc-fixup plan alongside the code work.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| krun (K Framework) | kloom harness, kloom-diff.sh | Not checked locally | — | LOOM_ALLOW_K_ORACLE_SKIP=1 skip |
| kompile (K Framework) | kloom-diff.sh, LLVM feasibility script | Not checked locally | — | Skip with env var |
| Rust (cargo) | all loom-core tests | Available | 1.92+ | None |
| melior feature | jit.rs production codegen path | Compile-time feature flag | — | Codegen falls back to JitUnavailable |

**Missing dependencies with fallback:**
- krun/kompile: `LOOM_ALLOW_K_ORACLE_SKIP=1` enables skip for local dev; CI job installs via Nix.

---

## Security Domain

No external packages are installed. No new network access. Skip enforcement using env vars is consistent with existing gate discipline. No ASVS-relevant concerns.

---

## Sources

### Primary (HIGH confidence — codebase verification)
- `crates/loom-core/src/kloom_harness.rs` — full read; krun invocation, definition_dir, serialize functions, error paths
- `crates/loom-core/src/native_arrow_semantic.rs` — lines 47-86 (diagnostic codes), 679-740 (validate), 1221-1349 (verify_model), 1946-2008 (reference_program_for_batch), 2010-2028 (native_model_trace)
- `crates/loom-native-melior/src/jit.rs` — lines 1-50 (cfg guards), 158-430 (production route), 582-588 (fallback_or_fail_closed), 882-892 (cfg(not) fallback)
- `crates/loom-ffi/src/duckdb_runtime.rs` — lines 24-34 (imports), 495 (prepare_duckdb_runtime call), 1076-1134 (prepare_duckdb_runtime body)
- `crates/loom-ffi/src/ffi.rs` — lines 72-73 (thread_local pattern reference)
- `crates/loom-core/src/runtime_abi.rs` — lines 446-535 (RuntimeCacheKey, schema_fingerprint composition)
- `contrib/kloom/src/kloom.k` — full read; appendValue/appendNull rules, EvalConst, TraceEvent syntax
- `contrib/kloom/scripts/kloom-diff.sh` — full read; kompile flags, krun invocation, strict failure handling
- `.github/workflows/ci.yml` — lines 204-246; kloom CI job, Nix K install, strict no-skip
- `scripts/toolchain-common.sh` — full read; LOOM_ALLOW_NATIVE_TOOL_SKIP convention
- `scripts/production-native-codegen-stabilization-test.sh` — skip-rejection pattern
- `contrib/kloom/docs/SEMANTICS.md` — semantic design, trace format alignment
- `contrib/kloom/README.md` — architecture position, trust model

### Tertiary (LOW confidence — cannot run K locally)
- K Framework LLVM backend behavior (A1-A4 in Assumptions Log)
