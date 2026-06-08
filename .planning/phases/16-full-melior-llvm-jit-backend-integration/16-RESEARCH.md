# Phase 16 Research: Full melior/LLVM/JIT Backend Integration

**Status:** Research report
**Date:** 2026-06-08
**Phase:** 16 — Full melior/LLVM/JIT Backend Integration
**Depends on:** Phase 13 verifier foundation, Phase 14 textual MLIR spike, Phase 15 real Vortex ingress evidence

## Executive Summary

Phase 16 should promote Phase 14 from deterministic textual MLIR evidence into
an optional programmatic MLIR/LLVM/JIT backend, but it should still be a
verifier-gated backend rather than a new execution path.

Recommended first slice:

```text
L2CoreProgram
  -> verify_l2_core accepted report + VerifiedArtifactFacts
  -> Phase 14 lowering support predicate
  -> melior-built MLIR module for bounded Int32 copy
  -> MLIR verification + optional pass pipeline
  -> MLIR ExecutionEngine / LLVM ORC-backed JIT
  -> typed primitive native execution evidence
  -> byte-for-byte comparison with Rust reference copy
```

The important product decision is:

- keep `loom-core` and `loom-ffi` free of mandatory MLIR/LLVM dependencies;
- put `melior`/MLIR/JIT integration in a separate optional crate or feature;
- make normal release gates pass without local LLVM;
- make native/JIT evidence explicit and fail-closed when tooling exists;
- reject unsupported accepted programs before MLIR/JIT artifact creation.

Phase 16 should not expand into a production custom Loom dialect, Arrow raw
buffer construction, vectorization, or multi-column native lowering. Those are
Phase 17+ topics.

## Local Starting Point

Phase 13 delivered the verifier handoff:

- `verify_l2_core` returns `FullVerificationReport`.
- `FullVerificationReport::is_ok()` indicates acceptance.
- `FullVerificationReport::facts()` returns `VerifiedArtifactFacts` only for
  accepted programs.
- `VerifiedArtifactFacts.row_count_bound` is the key native-memory bound for
  the current copy slice.

Phase 14 delivered the lowering boundary:

- `crates/loom-core/src/native_lowering.rs`
- `check_lowering_support(program, report)`
- `lower_to_textual_mlir(program, report)`
- `execute_supported_copy_i32(program, report, input)`
- accepted shape: one bounded `ForRange`, one `ReadInput`, one `AppendValue`,
  one non-null Int32 output, no cursor loop, no nulls, no scratch, no extra
  statements.

Phase 15 delivered real artifact evidence:

- `loom-vortex-ingress` is the only crate allowed to depend directly on
  `vortex-file`.
- Generated non-null Int32 `.vortex` can be inspected and emitted as `LMC1`.
- This is the first real artifact shape Phase 16 can target without overfitting
  only to synthetic unit-test programs.

Current local MLIR/LLVM state:

| Item | Observed |
|---|---|
| `/opt/homebrew/opt/llvm/bin/llvm-config` | `21.1.2` |
| `/opt/homebrew/opt/llvm/bin/mlir-opt` | available |
| `/opt/homebrew/opt/llvm/bin/mlir-translate` | available |
| `/opt/homebrew/opt/llvm/bin/lli` | available |
| tools on `PATH` | not currently on `PATH` except `/usr/bin/clang` |
| current workspace deps | no `melior`, `mlir-sys`, `llvm-sys`, `inkwell`, or `cranelift` |
| crates.io `melior` | `0.27.0` |
| crates.io `mlir-sys` | `220.0.2` |
| crates.io `llvm-sys` | `221.0.1` |

The local Homebrew LLVM 21.1.2 and crates.io `melior`/`mlir-sys` 22.x line do
not obviously match. Phase 16 must therefore probe and document toolchain
compatibility before making any Rust dependency mandatory.

## External Evidence

### melior

`melior` is the Rust binding layer for MLIR. Its README says it wraps the MLIR
C API and aims to provide a safe, complete Rust-facing API with a Rust ownership
model. It also states that LLVM/MLIR 22 must be installed and notes that both
melior and the MLIR C API are still alpha/unstable. It calls out safety caveats:
unloaded dialects can lead to runtime errors or worse, and some IR object
references can be invalidated after ownership-moving calls.

Source: https://github.com/mlir-rs/melior

Implications for Loom:

- `melior` is the right bridge for programmatic MLIR.
- It must be version-pinned and isolated.
- It must not become part of `loom-core`'s normal build until the toolchain
  matrix is proven.
- Phase 16 tests should load/register required dialects explicitly.

### MLIR ExecutionEngine

MLIR's `ExecutionEngine` is a JIT-backed execution engine for MLIR. The class
documentation says it assumes the IR can be converted to LLVM IR and creates a
packed wrapper function interface of the form:

```text
void _mlir_funcName(void **)
```

It supports lookup, packed invocation, symbol registration, and shared-library
initialization hooks.

Source: https://mlir.llvm.org/doxygen/classmlir_1_1ExecutionEngine.html

Implications for Loom:

- Phase 16 JIT should use a small C-compatible function ABI.
- The initial ABI should be typed primitive buffers, not Arrow arrays.
- Runtime symbols must be explicit and minimal.
- JIT invocation should be optional evidence because it depends on host
  toolchain, target, symbol registration, and linking details.

### MLIR to LLVM IR flow

The MLIR LLVM IR target documentation describes a two-stage flow:

1. Convert MLIR to dialects translatable to LLVM IR, such as the LLVM dialect.
2. Translate those MLIR dialects to LLVM IR.

It also says non-trivial transformations should happen inside MLIR before the
translation step. Ranked `memref` values lower to descriptor structs containing
allocated/aligned pointers, offset, sizes, and strides. Function and memref
calling conventions therefore become part of the native ABI story.

Source: https://mlir.llvm.org/docs/TargetLLVMIR/

Implications for Loom:

- Phase 16 should not skip directly from Loom to handwritten LLVM IR.
- A pass pipeline must be explicit and captured in the backend report.
- The first JIT ABI should avoid complex memref descriptors if possible, or
  document them as the ABI under test.
- Direct Arrow raw-buffer mutation should remain deferred.

### LLVM ORC JIT

LLVM ORC v2 is the modern LLVM JIT API family. Its design document describes a
JIT program model that emulates static/dynamic linker rules and supports
arbitrary LLVM IR, linking, symbol resolution, weak/common definitions, and
custom program representations.

Source: https://llvm.org/docs/ORCv2.html

Implications for Loom:

- ORC is powerful enough for the production path, but it is a linker/symbol
  system, not just "run this function."
- Phase 16 should rely on MLIR ExecutionEngine first, because it encapsulates
  the MLIR -> LLVM -> ORC handoff for the spike.
- Direct ORC integration is a later fallback or Phase 17+ productionization
  topic.

### MLIR C API

MLIR's C API is primarily intended to be wrapped by higher-level language
bindings. That is exactly the role `melior` plays.

Source: https://mlir.llvm.org/docs/CAPI/

Implications for Loom:

- Avoid direct ad hoc FFI calls to MLIR C APIs inside core crates.
- Prefer `melior` where possible, and keep any missing low-level calls isolated
  behind a local backend boundary.

## Architecture Options

| Option | Description | Benefits | Risks | Recommendation |
|---|---|---|---|---|
| Optional `loom-native-melior` crate | New crate depends on `melior`; consumes `loom-core` verifier/lowering APIs. | Keeps core clean, explicit toolchain boundary, easy feature gating. | More workspace plumbing; optional tests need skip logic. | **Use first.** |
| `loom-core` feature `melior-backend` | Add optional `melior` dependency directly to core. | Simpler API surface. | Pollutes core with LLVM toolchain concerns; harder no-LLVM build guarantee. | Avoid initially. |
| External `mlir-opt/mlir-translate/lli` subprocess pipeline | Keep Rust dependency-free; call installed tools. | Great for toolchain evidence and debugging. | Not programmatic integration; PATH/version fragility. | Use as supplemental evidence. |
| MLIR ExecutionEngine through `melior` | Build/run MLIR in-process via MLIR's JIT. | Best fit for Phase 16 goal. | ABI, symbol, version, linking complexity. | Main Phase 16 proof point, optional-gated. |
| Direct LLVM ORC | Emit LLVM IR and use ORC APIs directly. | Mature JIT control. | Bypasses MLIR value; larger unsafe/linker surface. | Defer to Phase 17+ or fallback. |
| Inkwell/llvm-sys-only | Rust LLVM bindings without MLIR. | More Rust LLVM examples exist. | Loses MLIR dialect/lowering design path. | Not recommended for Phase 16. |
| Cranelift | Rust-native JIT. | Lower toolchain friction. | Not MLIR/LLVM; conflicts with phase goal. | Keep only as future comparison. |

## Recommended Phase 16 Scope

### In Scope

- Create a Phase 16 backend contract:
  - backend is optional,
  - lower only after accepted `verify_l2_core`,
  - require `VerifiedArtifactFacts`,
  - require Phase 14 support acceptance,
  - unsupported programs fail closed before MLIR module/JIT creation.
- Add a separate optional native backend crate, likely
  `crates/loom-native-melior`.
- Probe toolchain compatibility:
  - find `llvm-config`,
  - record LLVM/MLIR version,
  - detect `mlir-opt`, `mlir-translate`, `lli`,
  - compare against the `melior`/`mlir-sys` expected version line.
- Build the same bounded Int32 copy module programmatically with `melior`.
- Verify the module with MLIR APIs.
- Run an optional pass pipeline / translation validation when compatible tooling
  is present.
- Execute the bounded Int32 copy through MLIR ExecutionEngine/JIT when the local
  toolchain supports it.
- Compare native output against `execute_supported_copy_i32`.
- Add a skip-aware script such as `scripts/melior-jit-test.sh`.
- Keep `scripts/mvp0-verify.sh` green on machines without compatible MLIR by
  treating JIT evidence as skipped optional evidence unless explicitly required.

### Out Of Scope

- Custom Loom MLIR dialect.
- Production pass pipeline design.
- Direct ORC integration.
- Direct generated Arrow raw-buffer writes.
- FSST/ALP/dict/RLE/string/native kernel lowering.
- Multi-column native lowering.
- Vectorization claims.
- Replacing interpreter execution in DuckDB.
- Compiler-correctness proof.
- Making MLIR/LLVM mandatory for `cargo test --workspace`.

## Proposed Crate Boundary

```text
crates/
  loom-core/
    native_lowering.rs        # verifier-gated support predicate + textual artifact
  loom-native-melior/         # optional Phase 16 crate
    toolchain.rs              # llvm-config/mlir tool discovery and version report
    builder.rs                # melior module construction for supported slice
    jit.rs                    # MLIR ExecutionEngine wrapper
    report.rs                 # stable backend facts/diagnostics
```

Suggested public shape:

```rust
pub struct MeliorBackendReport {
    pub supported: bool,
    pub diagnostics: Vec<MeliorBackendDiagnostic>,
    pub toolchain: Option<MlirToolchainFacts>,
    pub entry_symbol: Option<String>,
    pub jit_executed: bool,
    pub rows: Option<u64>,
}

pub fn build_melior_module(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
) -> Result<MeliorModuleArtifact, MeliorBackendReport>;

pub fn execute_copy_i32_jit(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
    input: &[i32],
) -> Result<Vec<i32>, MeliorBackendReport>;
```

The API should not accept standalone `VerifiedArtifactFacts`. It should accept
the verifier report, matching Phase 14's rule that facts are verifier-tied
evidence.

## ABI Recommendation

Use the smallest typed primitive ABI first:

```text
entry(input_ptr: *const i32, output_ptr: *mut i32, rows: usize) -> void
```

or, if using memref lowering directly:

```text
func.func @loom_l2core_copy_i32(
    %input: memref<?xi32>,
    %output: memref<?xi32>,
    %rows: index)
```

The pointer ABI is easier to reason about from Rust/JIT and avoids surprising
ranked-memref descriptor details. The memref ABI stays closer to the Phase 14
textual MLIR. The Phase 16 plan should decide explicitly. A conservative
sequence is:

1. Build memref MLIR programmatically to match Phase 14 textual output.
2. Validate/roundtrip MLIR text and pass pipeline.
3. Introduce a JIT wrapper ABI only after confirming how `melior` exposes MLIR
   ExecutionEngine invocation.

## Fail-Closed Rules

The Phase 16 backend must reject before native artifact creation when:

- `verify_l2_core` rejected the program;
- `FullVerificationReport::facts()` is absent;
- `check_lowering_support` rejects the program;
- required toolchain is absent for a requested JIT run;
- detected LLVM/MLIR version is incompatible with the `melior`/`mlir-sys`
  version line;
- required dialects cannot be loaded;
- MLIR module verification fails;
- pass pipeline fails;
- JIT symbol lookup fails;
- native output length or values differ from Rust reference output.

Normal release gates may skip optional JIT when tooling is absent. Explicit JIT
tests must fail closed instead of silently passing.

## Open Questions For Planning

1. Should Phase 16 pin to `melior 0.27.0` and LLVM/MLIR 22, or install/target a
   version matching local Homebrew LLVM 21.1.2?
2. Should the optional backend crate be part of the workspace by default, or
   kept behind a workspace feature / excluded from default members?
3. Should the first JIT ABI use memref descriptors or an explicit pointer ABI?
4. Should `scripts/mvp0-verify.sh` only report JIT as optional evidence, or
   should a separate strict gate be required for Phase 16 completion?
5. Should Phase 16 produce an object-file dump as evidence, or only in-process
   JIT output comparison?

## Recommended Plan Split

```text
16-01 Toolchain contract and optional backend crate boundary
16-02 Programmatic melior module construction for bounded Int32 copy
16-03 MLIR pass/translation validation plus skip-aware gate
16-04 MLIR ExecutionEngine/JIT execution and Rust reference equivalence
16-05 Final docs, release-gate wiring, and roadmap/state closeout
```

If planning pressure requires four plans, merge 16-03 and 16-04, but keep the
toolchain contract separate from JIT execution. That separation is load-bearing:
without it, Phase 16 can accidentally become "works on one laptop" rather than
a stable verifier-gated backend path.

## Sources

- melior README: https://github.com/mlir-rs/melior
- melior docs: https://mlir-rs.github.io/melior/melior/
- MLIR C API: https://mlir.llvm.org/docs/CAPI/
- MLIR ExecutionEngine: https://mlir.llvm.org/doxygen/classmlir_1_1ExecutionEngine.html
- MLIR LLVM IR target: https://mlir.llvm.org/docs/TargetLLVMIR/
- LLVM ORC v2: https://llvm.org/docs/ORCv2.html
- MLIR Toy lowering to LLVM: https://mlir.llvm.org/docs/Tutorials/Toy/Ch-6/
