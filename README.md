**English** | [中文](README-zh.md)

# A Distribution-Oriented Decoder IR — Design

**Working codename: Loom** · (it "weaves" bytes into Arrow columns; the name is a placeholder)

---

## 0. One-Sentence Positioning

Loom is a **decoder representation that travels with the data**: aimed at server-side data engines, it can be cheaply statically verified, compiled to full-speed native code, outputs legal Arrow, and is target-neutral and version-stable enough to be preserved for decades.

It is **not a smaller WebAssembly**, but a different species: **a non-general, non-Turing-complete, total-function domain language whose only possible output is well-formed Arrow**. Every one of its constraints is a piece of freedom that Wasm / eBPF / LLVM-MLIR cannot afford, because they chose to "be able to run arbitrary computation."

---

## Current MVP0 Implementation

The repository currently implements an interpreter-based MVP0, not the complete distribution IR described below. The working path is:

```
in-memory Vortex fixtures -> Loom layout payload -> loom-core interpreter
  -> Arrow C Data Interface -> DuckDB loom_scan(...) -> SQL checks
```

Supported MVP0 encodings are bitpack, frame-of-reference, dictionary, RLE, FSST strings, and dictionary-over-FSST strings. The current table path wraps multiple single-column layout payloads in an `LMT1` table payload, preserving `LMP1` single-column compatibility while letting CLI and DuckDB scan named columns. A first-pass structural verifier now checks MVP0 layout/table descriptions before decode and reports stable diagnostic code/path/message triples for malformed inputs. The acceptance bar is row and aggregate equality against Vortex's own decoder/oracle for generated fixtures, plus curated negative verifier cases that fail closed before DuckDB execution.

Run the full MVP0 release gate:

```bash
bash scripts/mvp0-verify.sh
```

The gate runs the same underlying checks manually available as:

```bash
cargo test --workspace
cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}'
rg -n 'vortex_file|vortex-file|\.vortex|VortexFile|from_path|read_file' crates/loom-fixtures
bash scripts/verifier-negative-test.sh
bash scripts/duckdb-smoke-test.sh
```

The current `.loom` payload format is an MVP0 internal fixture format. Phase 9's verifier is structural: it rejects malformed buffers, count mismatches, unsupported type/layout combinations, unknown kernels, and related table-shape errors for the implemented MVP0 surface. It is not the formal Loom verifier and does not claim totality or termination proofs; MLIR/native lowering, Arrow stream ABI, the full formal verifier, and full `.vortex` file support remain future milestones.

Phase 7 adds reviewer-facing descriptor and CLI tooling:

```bash
cargo run -p loom-fixtures --bin emit_duckdb_payloads
cargo run --bin loom -- inspect target/loom-duckdb-fixtures/bitpack-i32.loom
cargo run --bin loom -- decode target/loom-duckdb-fixtures/fsst-utf8.loom
cargo run -p loom-fixtures --bin loom_fixture_timing
```

`loom inspect` prints `verification: pass` for valid payloads/descriptors and `verification: fail` with diagnostics for verifier-rejected inputs.

The timing command reports illustrative wall-clock numbers for Loom interpreter decode vs Vortex oracle decode. It is not a benchmark and has no pass/fail speed threshold.

Phase 8 adds a small multi-column table fixture and DuckDB SQL acceptance path:

```bash
cargo run -p loom-fixtures --bin emit_duckdb_payloads
cargo run --bin loom -- inspect target/loom-duckdb-fixtures/mixed-table.loom
cargo run --bin loom -- decode target/loom-duckdb-fixtures/mixed-table.loom
bash scripts/duckdb-smoke-test.sh
```

`mixed-table.loom` exposes `id INT32`, `flag BOOLEAN`, and `label VARCHAR` through `loom_scan(...)`. The extension still uses direct DataChunk population; the ArrowArrayStream route remains a future ABI decision rather than Phase 8's implementation path.

---

## 1. Goals and Non-Goals

**Goals**

- Deliver the decoding logic of any columnar / semi-structured format (Vortex, ROOT, Parquet, custom encodings…) to the doorstep of a data engine **safely** (sandboxed), **portably** (one bytecode, many engines), and **durably** (readable decades from now).
- Given an untrusted decoder + untrusted data, guarantee that it **cannot blow through the host or hang the query to death**.
- Fix the decode output as **Apache Arrow**, handed off to the host zero-copy.

**Explicit Non-Goals (equally important)**

- **No general computation.** You cannot write a web server, you cannot write a query engine. This is the source of its power, not a defect.
- **No catering to browser / edge / IoT.** The target set is only "server-side data engines," so we can assume 64-bit, SIMD, mmap, and a long-resident host.
- **No correctness guarantee.** Only safety + well-formedness is guaranteed (see §7).
- **No responsibility for parallelism.** The decode core is single-threaded; parallelism belongs to the host (see §5).
- **No prescribed execution backend.** The distribution spec only defines "the layer that travels"; how it is compiled into native code is each engine's own business (see §8).

---

## 2. Position in the Data-System IR Landscape

There are three mutually orthogonal "IR jobs" in data systems:

| Axis | Representative | What it does | Cross-system? |
|---|---|---|---|
| Shipping the plan | Substrait | Describes relational computation, letting frontends/backends combine freely | Yes (exchange) |
| **Shipping the decoder** | **Loom** | Distributes decoding logic with the data, safely and portably | **Yes (with the data)** |
| In-engine compilation | MLIR / LingoDB | Compiles queries into native code | No (in-process) |

Loom is **complementary to the other two, not competing**. Its relationship with the execution backend is a relay:

```
[travels with data] Loom distribution IR ──verify──▶ MLIR `decode` dialect ──lower──▶ LLVM IR ──▶ native code
   ↑ must be brand new                       ↑ MLIR only takes over after the trust boundary
   stable/neutral/verifiable/sandboxed       in-process, trusted, close to the machine——exactly MLIR's home turf
```

**Why the distribution layer cannot reuse MLIR/LLVM**: a compilation IR and a distribution IR are two species with opposite design goals—a compilation IR wants unlimited expressiveness, to hug the machine downward, to be mutable across versions, and to trust input by default; a distribution IR wants limited expressiveness, target neutrality, eternal cross-version stability, and verification of untrusted code. PNaCl already wrote this path—"using LLVM bitcode as a distribution format"—in blood once.

---

## 3. Core Design Principle: If You Can Declare It, Don't Write Code

Observation: a real decoder is about 90% **structural layout** (offsets, repetition, RLE, dictionary) and only about 10% **genuine compute kernels** (FSST symbol table, ALP exponent search, decompression).

Therefore Loom is **two layers**:

- **L1 declarative layout layer** —— it is **data**, not code. Zero verification (data cannot go out of bounds, cannot fail to terminate), highest stability. From it the engine **automatically generates** the vectorized decode read loop. Precedents: Kaitai Struct, DFDL (proving this appetite exists), but they are purely declarative and not designed for speed / sandboxed distribution.
- **L2 total-function kernel layer** —— only the genuine compute heavy-lifting that declarative expression cannot capture falls down here, to be verified and lowered to native.

**Principle: if it can be declared, don't write it as code; whatever must be written as code, make it a total function.** This squeezes the "surface area of code that needs verification" to the minimum—the verification burden, the attack surface, and the semantics that must be frozen forever all collapse accordingly.

---

## 4. L1: The Declarative Layout Layer

L1 describes "what the data looks like"; it is a typed physical layout tree:

- **Primitive fields**: fixed-width integers/floats, varint, fixed-/variable-length byte strings, with byte order and alignment.
- **Repetition**: `count` comes from a constant, from the value of another field (`length-prefixed`), or from an outer extent.
- **Offset-driven**: a field's position can be computed from other already-parsed fields within the same record (`offset = f(other_fields)`).
- **Declarative encodings**: RLE, bit-packing, FOR (frame-of-reference), and dictionary are declared directly as **parameterized built-in encodings** (`bitpack(width=11)`, `dict(ref=...)`), with no code to write.
- **Escape to L2**: when a segment needs a custom codec, declare a reference to an L2 kernel (`codec = kernel#3`).

L1 is pure data, so it is free to the verifier; it also drifts over time less than code does, which makes it the most stable layer. The engine takes L1 and synthesizes that vectorized read loop itself.

---

## 5. L2: The Total-Function Kernel Layer

Used only when L1 cannot express it. This is a **deliberately non-general** language.

**5.1 Total Function, Non-Turing-Complete**

- No arbitrary recursion, no `while(true)`.
- Iteration has only two legal forms:
  1. **Count-bounded**: loop over `N` elements, where `N` is a count visible to the verifier, derived from input/output extent.
  2. **Data-monotone**: each round consumes ≥1 byte of finite input, or advances toward a known-bounded output.
- Termination is proven at **verification time** by a **decreasing measure / ranking function** bound to **(remaining input ‖ remaining output)**—free, and strictly superior to a runtime fuel counter.
- Recursion over the schema's nested structure is **structurally bounded** (nesting depth is statically determined by the schema), so it does not break total-function-ness.

**5.2 Data Parallelism Expressed as Structure, Not Concrete SIMD**

- The IR **forbids any concrete vector instruction or width from appearing**. Operations are described as "applied independently of one another over an abstract lane structure."
- The choice of physical vector width (128/256/512/SVE-scalable) is **entirely delegated to the engine's MLIR backend**.
- Borrowing FastLanes' insight of a "unified virtual ISA + forced auto-vectorization": the IR is target-agnostic (hence stable, hence portable), but because parallelism is **explicit structure**, the backend cannot possibly miss vectorization (hence fast). This single stroke dissolves "fast vs. stable."

**5.3 Memory Model**

- `input`: a read-only mmap view (capability handle), the entire encoded file, native 64-bit addressing—no 4 GB ceiling, no Memory64 checking tax.
- `scratch`: a bounded working arena whose upper bound the verifier can compute.
- **No raw output writes**: output can only go through the builder primitives of §6.

**5.4 The host-call surface area = the entire trust interface**

The host capabilities a decoder can call are only two kinds: **fetch an input range**, and **request an output buffer / emit a batch**. No files, no network, no syscalls. This minimal set of callbacks is the entire attack surface, small enough to audit line by line.

---

## 6. The Output Contract: Emit Typed Arrow Events

L2's output primitives are **not "write memory"**, but a set of **typed builder operations**: `append_value`, `append_null`, `begin_list`/`end_list`, `begin_struct`/`end_struct`, and so on.

Consequence: **the output is legal by construction**—the consistency of offsets, null bitmaps, and lengths, and the integrity of child arrays for nested types, are all guaranteed by the builder semantics, and the verifier does **not have to check a single word** of these. And this pile of builder primitives is **fused and optimized into vectorized raw writes** in the MLIR backend.

> Again the same division of labor: **the IR layer guarantees safety, the native layer takes speed back.**

The output is ultimately materialized as the Arrow C Data Interface's `ArrowArray` / `ArrowSchema`, handed off to the host zero-copy.

---

## 7. The Safety Boundary: Guarantee Safety and Well-Formedness, **Not** Correctness

**What the verifier is obligated to prove**

- Memory safety: all accesses fall within declared regions, no arbitrary pointer arithmetic.
- capability-only: no syscalls, no ambient authority.
- Total-function-ness: the decreasing measure guarantees termination (compile time).
- Output well-formedness: by builder construction + schema type checking.

**What the verifier does not prove**: **correctness**. A malicious / buggy decoder can perfectly well, safely and well-formedly, produce Arrow whose **data is entirely wrong**. This is on par with today's native readers (native readers mis-decode too), so it is ruled out of scope. But be clear-eyed:

> Self-decoding frees you from "will the decoder blow through the process," it does **not** free you from "is the decoder's author trustworthy."

Correctness can only be patched by orthogonal means (computing checksums over the output, etc.), and you usually have no independent second decode to compare against—this one is basically unsolvable and can only be accepted.

---

## 8. Execution: Lowered to Native via MLIR

Distribution form (Loom) → a `decode` MLIR dialect inside the engine:

- Express the read loop synthesized from L1, and L2's decode primitives (bit-unpack, FOR, delta, dict, FSST, ALP…), as MLIR ops.
- Lower to LLVM IR → native code; **only at this step is the physical SIMD width chosen**, reusing MLIR's ready-made CSE, constant-folding, and auto-vectorization passes (LingoDB has already proven this works).
- The builder events of §6 are fused into vectorized raw writes here.

No detail of the distribution form **leaks** to the target machine; target-dependence exists only after lowering. **The trust boundary = the seam between Loom and MLIR**: before the boundary nothing may be MLIR, only at and after the boundary does MLIR take over.

---

## 9. ABI: The Decoder Entry Points

```
schema()                                              -> ArrowSchema
decode_batch(input, range, projection_mask, state)    -> ArrowArray
statistics(input, range)                              -> ColumnStats   // optional
```

- `range`: a row range, for random access.
- `projection_mask`: column projection / pruning, decode only the columns needed.
- `state`: explicit, owned state, for formats with cross-record dependencies (such as ROOT's inter-frame dependency).
- `statistics`: returns per-column min/max/null-count, letting the engine skip whole segments—the portable expression of predicates plugs into **Substrait**.

---

## 10. Distribution, Trust, and the Fast Path

**Distribution artifact** = a versioned container: `{ schema, L1 layout description, L2 kernel module, feature flags, (optional) multi-tier kernels }`.

- **Travels with the data** (self-decoding), or is referenced by a **content-hash URI**.
- **Hybrid fast path**: a hash hit on a well-known format the host has already audited → use the host's **native implementation** directly, skipping verification/JIT (following AnyBlox's decoder-URI + checksum mechanism).
- The verifier is the safety boundary; signatures / remote attestation are optional, not the boundary itself.

---

## 11. Version Evolution and Durability

- **Freeze a minimal, never-changing core** + **feature flags** in the header declaring which features this decoder uses.
- An old engine encountering an unknown feature → **cleanly refuses**, never executes wildly; a new engine is **always backward-compatible** with old data.
- **The signature weapon (a gift sent back by code traveling with the data)**: the same decoder can bundle **multiple tiers of implementation** (baseline + aggressively optimized version) together into the container, and the engine picks the highest tier it can understand to run. A format embedded in the system cannot do this.

---

## 12. Positioning Against Existing Approaches

Detailed comparison note: [.planning/research/POSITIONING.md](.planning/research/POSITIONING.md).

| | Distribution-portable | Untrusted sandbox | Total function (provably terminating) | Native full-speed | Target-neutral / version-stable | Mandatory Arrow output |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| Wasm / AnyBlox | ✓ | ✓ | ✗ (Turing-complete, relies on fuel) | △ (~1.5x sandbox tax) | △ | ✗ |
| eBPF / uBPF | △ | ✓ | ✓ (but too restrictive) | ✓ | ✗ | ✗ |
| LLVM IR / raw native | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ |
| MLIR / LingoDB | ✗ (compilation-internal) | ✗ | ✗ | ✓ | ✗ | △ |
| Substrait | ✓ (shipping the plan) | n/a | n/a | n/a | ✓ | n/a |
| **Loom** | **✓** | **✓** | **✓** | **✓ (tax paid at verification time)** | **✓** | **✓** |

There is only one root reason Loom can max out every column at once: **it dares to be non-general.** All the other approaches pay a price for "being able to run arbitrary computation"—hard to verify, large semantics, or non-portable.

---

## 13. The Hard Bones Honestly Left on the Table

1. **The verifier and the JIT themselves enter the TCB.** The IR being small and structured makes a "formally verified verifier" achievable (the eBPF verifier shipping CVEs is the counterexample), but that is real work, not free.
2. **The correctness crack cannot be patched** (§7), and can only be accepted.
3. **Who freezes v1, and freezes it well enough?** A format that claims to "never break" must leave almost no regrets in its very first version—the most anti-human engineering requirement, and also the place most likely to die at the starting line.
4. **The adoption dilemma.** LingoDB proves that ambitious, IR-based data infrastructure **can be built and extended**, but it is an **in-engine** game (you only have to convince yourself); Loom is a **cross-system exchange** game (you have to convince every engine to adopt a shared format + accept the untrusted threat model), an order of magnitude harder.

> The conclusion is consistent with the historical pattern: Loom will not be built because "it is correct," it will only be built by some **MPP engine that brings its own host**—when it is forced to build it by an explosion of untrusted data/formats—after which everyone else free-rides; just as WebAssembly once rose from PNaCl's grave. Until then, it will keep looking "unviable."

---

## 14. The Whole Thing in One Paragraph

Loom locks itself into being "**a total-function language that eats finite bytes and emits well-formed Arrow**": whatever can be declared goes through L1 (data, zero verification), whatever must be computed goes through L2 (total function, termination proven at verification time), parallelism is expressed as an abstract lane structure rather than concrete SIMD, and output goes through a typed builder so it is legal by construction. As a result it gets to be **small, stable, cheaply verifiable, and forever portable** all at once; while taking "full speed" back by sinking it into the engine-internal MLIR `decode` dialect. The trust boundary lands exactly on the seam between Loom and MLIR: **Loom is responsible for delivering decoding logic to the doorstep safely, portably, and durably; MLIR is responsible for compiling it into native code once it is inside.**
