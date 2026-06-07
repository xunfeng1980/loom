# Pitfalls Research

**Domain:** Rust decoder core + Arrow C Data Interface FFI + C++ DuckDB extension — Vortex single-column decode prototype
**Researched:** 2026-06-07
**Confidence:** HIGH (Arrow C Data Interface spec + official Rust docs + DuckDB GitHub issues + fsst-rs README + RustSec advisory)

---

## Critical Pitfalls

### Pitfall 1: Arrow C Data Interface — Release-Callback Ownership and Move Semantics

**What goes wrong:**
`to_ffi` in arrow-rs transfers ownership of the `ArrayData` buffers into the `FFI_ArrowArray` struct.
The struct carries a `release` callback that Rust installed; DuckDB's C++ side is required to call `release`
when it is done. If the Rust side also drops the `ArrayData` that was passed into `to_ffi`, the buffers are
freed twice — one from Rust's Drop and once from the release callback fired by DuckDB. Conversely, if the
C++ side never calls `release` (e.g., an early-return path, exception, or the scan function simply forgets),
the buffers are leaked permanently. A subtler variant: the caller `std::ptr::write`s the `FFI_ArrowArray`
into a caller-provided pointer, then also lets the local `FFI_ArrowArray` binding go out of scope — that
drop fires the release callback before DuckDB ever touches the data.

**Why it happens:**
The C Data Interface is a C-level protocol with no Rust borrow checker enforcement. `to_ffi` returns an
owning value; if you don't consume it with `std::ptr::write` (moving it out of Rust's drop graph), Rust
drops it — and the release callback fires. The code compiles silently. The arrow2 crate had a confirmed
double-free (RUSTSEC-2022-0012) from a `#[derive(Clone)]` on the FFI struct; the same conceptual mistake
(shallow-copy + double-drop) applies whenever an `FFI_ArrowArray` is duplicated without zeroing the
source's release pointer.

**How to avoid:**
Use exactly one `std::ptr::write(out_array, ffi_array)` per produced `FFI_ArrowArray`. The `write` call
moves the value bitwise into the output pointer; the Rust binding is then forgotten (no Drop). Never clone
an `FFI_ArrowArray` or `FFI_ArrowSchema` struct. Never hold a Rust reference into the data after the write.
On the C++ side, call `release(array)` in the scan state's destructor, in every early-return path, and
after `ArrowToDuckDB` finishes copying — not before. Set `release = nullptr` immediately after calling it
to prevent double-free. Audit all early-exit and error paths in the C++ scan callback.

**Warning signs:**
- Crash or SIGSEGV inside `malloc`/`free`/`jemalloc` in a DuckDB scan, not in application code.
- Valgrind or AddressSanitizer reports "invalid free" or "heap use after free" on a pointer from the Rust
  decoder.
- Scan works in the happy path but crashes on re-scan, query error, or when DuckDB's parallel executor
  touches the same stream from a different thread.
- `release` callback fires during Rust's test run (not during the DuckDB scan).

**Phase to address:**
Phase 1 (FFI boundary setup). Establish the exact move protocol in the first commit that crosses the FFI
boundary. Add a test that exercises the C++ release path explicitly (call `release` manually in a unit test
outside DuckDB) before integrating into the table function.

---

### Pitfall 2: Schema/Array Lifetime Mismatch — Schema Freed Before Array Is Consumed

**What goes wrong:**
The Arrow C Data Interface explicitly states: "The ArrowSchema and ArrowArray are independent; their
lifetimes are not tied to each other." DuckDB PR #15632 fixed a bug where the `FFI_ArrowArray` was
dropped before child vectors were done with it because DuckDB's scan code didn't extend the array's
lifetime to child `Vector` objects. The analogous Loom failure: if Loom produces an `FFI_ArrowSchema` and
then drops its backing `Field`/`DataType` allocations (because the Rust schema object goes out of scope
before DuckDB finishes reading the schema), DuckDB reads a dangling pointer.

**Why it happens:**
`to_ffi` for the schema takes the `Field` value and stores its data in the `FFI_ArrowSchema`. The C++
importer does not know how long it will need the schema (it may cache it across multiple scan calls). If
the Rust function that produced the schema returns and the original `Field` is dropped, the `FFI_ArrowSchema`
now points to freed memory even though its `release` callback has not been called yet.

**How to avoid:**
The `FFI_ArrowSchema` produced by `to_ffi` owns its data independently — treat it exactly like
`FFI_ArrowArray`: move it via `std::ptr::write`, not by reference. Never return a reference into a
`FFI_ArrowSchema` that a Rust stack frame owns; the value must be moved into caller-allocated storage.
In the C++ init/bind callback that receives the schema, call `schema.release(&schema)` as soon as the
type information has been read into DuckDB's `LogicalType` — do not hold onto the raw `FFI_ArrowSchema`
pointer longer than necessary.

**Warning signs:**
- Garbage `LogicalType` inside DuckDB for a column whose type looks correct during bind but wrong during scan.
- SIGSEGV inside DuckDB's schema-parsing code path, not in user code.
- Bug only appears when the Rust function that produces the schema goes out of scope before the first
  `Scan` callback fires.

**Phase to address:**
Phase 1 (FFI boundary setup). Write a test fixture that simulates the DuckDB lifetime pattern: produce
schema, drop all Rust values, then read the schema from C++. Should exercise `release` as well.

---

### Pitfall 3: Panic Across the extern "C" Boundary — Process Abort Without Recovery

**What goes wrong:**
Any Rust `panic!` / `unwrap()` / `expect()` / out-of-bounds index that fires inside a function declared
`extern "C"` previously caused undefined behavior (RFC 2945 history). Current Rust (stable, since 1.73)
aborts the process when a panic would unwind past an `extern "C"` frame — but "aborts the process" means
DuckDB's entire process dies. In a demo context this crashes the shell; in a server context it kills the
database. Unwrapping inside a scan callback (`decode().unwrap()`) is the immediate failure mode.

**Why it happens:**
Rust's `?` operator and `.unwrap()` on `Result`/`Option` panic on `None`/`Err`. Arrow builder methods,
Vortex encoding dispatch, and FSST decompression all return `Result`. Forgetting to convert these to
explicit returns converts normal decode errors into process-killing panics. The DuckDB community extension
template and quack-rs both enforce `panic = "abort"` in release profiles; the danger is in debug builds
or when `catch_unwind` is not used.

**How to avoid:**
Set `panic = "abort"` in `[profile.release]` in the Rust decoder's `Cargo.toml`. In all `extern "C"`
entry points (every function called from C++), wrap the body in `std::panic::catch_unwind(|| { ... })`
and convert the caught panic into a structured error return that the C++ side can check before calling
into DuckDB error reporting. Never use `.unwrap()` or `.expect()` in any function reachable from an
`extern "C"` entry point — always use `?` or explicit `match`.

**Warning signs:**
- DuckDB session exits entirely instead of returning a SQL error.
- `"Aborted (core dumped)"` or `"Illegal instruction"` in the terminal during scan.
- A panic message printed to stderr (thread 'main' panicked at ...) right before DuckDB exits.

**Phase to address:**
Phase 1 (FFI boundary setup). Establish the `catch_unwind` wrapper as a mandatory boundary contract in
the first extern-C function; code review should reject any PR that adds a direct `extern "C"` fn without
a `catch_unwind` wrapper.

---

### Pitfall 4: DuckDB Extension ABI — C++ Extension Must Match DuckDB Version Byte-for-Byte

**What goes wrong:**
DuckDB's extension loader validates a 512-byte footer in the `.duckdb_extension` binary that embeds the
exact DuckDB git hash the extension was compiled against. If the DuckDB binary version differs from the
DuckDB version the extension was compiled against — even a patch release difference — the loader rejects
the extension:

    "The file was built specifically for DuckDB version 'ef50246314' and can only be loaded with that version."

This is not a warning; it is a hard load failure. The C++ API is also not stable across DuckDB releases
in the sense that internal types, method signatures, and vtables change. An extension compiled against
1.5.2 headers may link against a 1.5.3 binary but call into a method that moved, leading to silent
corruption or a crash rather than a clean load failure.

**Why it happens:**
DuckDB uses the git commit hash (not just semver) as the ABI key. The extension-template Makefile has a
`DUCKDB_GIT_VERSION` variable; if you pull DuckDB's CMake dependency separately (e.g., via vcpkg or a
system install) and it resolves to a different commit than the one in the Makefile, the mismatch is
invisible until load time.

**How to avoid:**
Pin `DUCKDB_GIT_VERSION=v1.5.3` in the extension-template Makefile. Use `make update_duckdb_headers` to
fetch the matching headers before first build. Do not install DuckDB from Homebrew/apt and then build the
extension from source — the binary and the headers will diverge. Use a single version variable in CI and
the local build. For the demo, build DuckDB from source at the same tag, or download the exact DuckDB
binary from duckdb.org that matches `v1.5.3`.

**Warning signs:**
- "Failed to load extension" with a git hash in the error message.
- Extension loads in `make test` (which builds its own DuckDB) but fails when run with the system DuckDB.
- Functions registered via `ExtensionUtil::RegisterFunction` exist but calling them crashes rather than
  returning a SQL error.

**Phase to address:**
Phase 2 (DuckDB extension scaffold). Lock the version before writing any C++ extension code. Add a CI step
that verifies the DuckDB binary version equals the build target version.

---

### Pitfall 5: Rust staticlib Symbol Clashes and Allocator Mismatch with DuckDB

**What goes wrong:**
The Rust `staticlib` is linked into the DuckDB extension `.duckdb_extension` dylib. Both DuckDB and the
Rust standard library may define `malloc`/`free`/`realloc` (via different global allocators). On Linux
with glibc, malloc is a weak symbol and DuckDB's jemalloc extension overrides it; the Rust allocator may
resolve to a different malloc than jemalloc. If a buffer is allocated on the Rust side (using Rust's
allocator) and freed on the C++ side (using DuckDB's allocator or jemalloc), you get a heap corruption.
On macOS, the system `malloc` is a strong symbol; symbol duplication between Rust's `libstd` and the C++
runtime manifests differently (linker warnings, or one implementation silently wins).

The second surface: any Rust crate that uses `link_name` to export a symbol also exported by DuckDB
(e.g., `duckdb_open`, `duckdb_connect`, or internal DuckDB utility names) will cause a linker error or
silent symbol hijacking if the names collide.

**Why it happens:**
A Rust `staticlib` bundles the Rust standard library, runtime, and all dependencies. When linked into a
C++ shared library, all symbols are visible at the dylib level unless explicitly hidden. DuckDB extensions
are loaded into the DuckDB process alongside the core DuckDB dylib; all exported symbols from all dylibs
share a single flat namespace on Linux/macOS by default.

**How to avoid:**
Use `#[global_allocator]` with the system allocator (`std::alloc::System`) in the Rust decoder staticlib
to avoid Rust pulling in its own allocator that conflicts with DuckDB's. Never allocate on the Rust side
and free on the C++ side or vice versa — ownership of heap allocations must not cross the language boundary.
The FFI_ArrowArray buffers are owned by Rust's `ArrayData` and freed by the `release` callback (which
calls back into Rust) — this is the correct pattern, no cross-allocator free occurs. Add
`-Wl,--version-script` (Linux) or `-Wl,-exported_symbols_list` (macOS) to the C++ link step to hide all
Rust symbols that are not the deliberate `extern "C"` API.

**Warning signs:**
- Linker warnings about "multiple definition of `malloc`" or "symbol `_Znwm` defined in multiple
  compilation units."
- Heap corruption or `malloc: Heap corruption detected` errors that appear non-deterministically.
- The extension loads and scans correctly in isolation but crashes when DuckDB's jemalloc extension is
  also loaded.

**Phase to address:**
Phase 2 (DuckDB extension scaffold / build system). Set `System` allocator in the first iteration of the
Rust staticlib before adding any heap-allocating code. Verify with `nm -g` on the staticlib that no
unexpected malloc/free symbols are exported.

---

### Pitfall 6: FSST Little-Endian-Only and "Not Production Ready" Warning

**What goes wrong:**
The `fsst-rs` crate (pulled transitively by `vortex-fsst 0.74`) carries two explicit limitations in its
README: (1) "This current implementation is still in-progress and is not production ready, please use at
your own risk." (2) "This crate only works on little-endian architectures currently. There are no current
plans to support big-endian targets." For MVP0 these are mostly academic (demo on x86-64 macOS/Linux is
always little-endian), but the "not production ready" caveat means FSST decode may have correctness bugs
on edge-case inputs — including inputs that the Loom reference check would expose.

**Why it happens:**
FSST symbols are stored packed into a 64-bit register; the bit-level layout assumes little-endian
ordering for symbol extraction. The crate was written by the SpiralDB team for their own pipeline; it
is battle-tested on their data but not on the full adversarial input space.

**How to avoid:**
Use the reference decoder (`vortex-fsst`'s `FsstEncoding::decode`) as the oracle for the verification
harness. Do not write an independent FSST decoder — call the same `Decompressor` type that vortex-fsst
uses, to guarantee bit-for-bit agreement. Generate test vectors via `vortex-fsst`'s encoder, then verify
that the Loom L2 kernel path produces identical output. Pay special attention to: the empty string (zero
bytes of encoded output), strings that are entirely escape sequences (byte 255 followed by the original
byte), and the maximum symbol length (8 bytes). Run all tests on x86-64 only; do not attempt ARM
big-endian support in MVP0.

**Warning signs:**
- Decoded string values are garbled (wrong bytes, wrong length) but no panic or error is raised.
- Empty strings decode to non-empty output, or non-empty strings decode to empty.
- Output matches for ASCII-only inputs but diverges on strings with bytes >= 128.

**Phase to address:**
Phase 3 (FSST L2 kernel). First action in this phase: build the verification harness (Loom output vs.
`vortex-fsst` oracle) before writing a single line of FSST decode logic. Every FSST test must compare
byte-for-byte against the oracle.

---

### Pitfall 7: Vortex Validity / Null Handling — Outer Encoding Strips Inner Nullability

**What goes wrong:**
In Vortex's encoding model, each `ArrayRef` may carry an optional validity (null bitmap) independently
of its encoding. A `BitPackedArray` wrapping a non-nullable inner array is itself allowed to have
nulls, stored in a separate validity buffer. When Loom's L1 read loop decodes the bitpacked values it
will correctly produce `u32` values, but if it discards the outer `validity` buffer without checking
`array.validity()`, all nulls become phantom values — DuckDB receives valid integers that should be
null, and the row-for-row comparison fails.

The converse bug: if the L1 loop emits `append_null` for rows where validity is absent (non-nullable
column), it emits null bits into the Arrow output that should not be there, causing DuckDB to treat
valid values as NULL.

**Why it happens:**
`into_canonical()` on a Vortex array handles null propagation automatically. When building the Loom
interpreter instead of calling `into_canonical()`, validity must be threaded manually at each layer.
The pattern `array.validity().is_some()` must gate every `append_value`/`append_null` decision in the
read loop. Forgetting this check is easy because most test inputs use non-nullable arrays.

**How to avoid:**
In the L1 read loop, always check `array.validity()` before emitting values. The pattern is:
```rust
match array.validity() {
    Some(validity) => {
        for (i, val) in values.iter().enumerate() {
            if validity.is_valid(i) { builder.append_value(val); }
            else { builder.append_null(); }
        }
    }
    None => {
        for val in values.iter() { builder.append_value(val); }
    }
}
```
Test with at least one nullable column (validity bitmap present) and one non-nullable column for each
encoding (bitpack, FOR, dict, RLE) before marking any phase done.

**Warning signs:**
- Row-for-row comparison passes on non-nullable test columns but fails on nullable ones.
- DuckDB `COUNT(*)` vs `COUNT(col)` discrepancy (null rows counted differently).
- Aggregate results (SUM, AVG) differ between Loom and Vortex by the value at null positions.

**Phase to address:**
Phase 3 (L1 decode loop). Add one nullable test vector for each L1 encoding in the initial test matrix.

---

### Pitfall 8: Pulling in the Full Vortex File Format (vortex-file / vortex-serde) Instead of In-Memory Array Serialization

**What goes wrong:**
The Vortex file format (`.vortex`) requires a footer, a layout tree, and a postscript to be parseable;
layouts are explicitly "not self-describing." A naive approach to producing a test input — e.g.,
`VortexFile::write_array(col)` — drags in `vortex-file` and `vortex-serde`, which in turn require
reading the footer and layout tree back when decoding. MVP0's explicit out-of-scope list says "Full
.vortex file layout (footer / layout tree / multi-chunk)." If the MVP input format accidentally requires
footer parsing, the entire file-reader layer (thousands of lines of Vortex) must be imported, and any
test failure may be in the reader rather than the Loom decoder. Scope creep will be invisible until
the first integration test fails with a cryptic flatbuffer deserialization error.

**Why it happens:**
It is not obvious from the vortex-array API how to produce a standalone serialized `ArrayRef` without
the file container. The Vortex docs focus on file-level I/O. Developers reach for the most visible
serialization API, which is the file-level one.

**How to avoid:**
For MVP0, construct the test input array programmatically in Rust using `vortex-array`'s builder APIs
(`BitPackedArray::try_new`, `FoRArray::try_new`, etc.) without serializing it to disk at all. Pass
the `ArrayRef` directly to the Loom decode function in tests. For the end-to-end demo where data must
come from "outside," serialize only using `arrow-ipc` (producing an Arrow IPC file containing the
pre-decoded values, used as oracle data) — not a `.vortex` file. If a serialized Vortex format is
truly needed for the demo, use `vortex-ipc` (the IPC/stream format, not the file format), which does
not require a footer. Do not add `vortex-file` as a dependency in MVP0.

**Warning signs:**
- `vortex-file` or `vortex-serde` appears in `Cargo.lock` or `Cargo.toml`.
- A test fixture reads from a `.vortex` file path.
- Compile errors referencing `Footer`, `Postscript`, `LayoutReader`, or `RowFilter`.
- Scope discussion slides into "we need to handle the layout tree" or "multi-chunk support."

**Phase to address:**
Phase 1 (project scaffold). Lock the Cargo.toml dependencies list: `vortex-array`, `vortex-fastlanes`,
`vortex-dict`, `vortex-fsst` only. Any PR adding `vortex-file`, `vortex-serde`, or `vortex-ipc` must be
explicitly approved as scope expansion.

---

### Pitfall 9: arrow-rs Version Skew — Duplicate Arrow Types Across the FFI Boundary

**What goes wrong:**
If any crate in the Cargo workspace resolves to a different version of `arrow-array` or `arrow-schema`
than the version pinned in `Cargo.toml`, Rust treats the types as distinct even if they are
structurally identical. Passing an `ArrayData` from `arrow-array` 58.2 into `to_ffi` from `arrow` 58.3
produces a compile error ("expected type X found type X" where both Xs are from different crate versions).
More dangerous: if the workspace somehow resolves two compatible minor versions and the conflict is
hidden, the `FFI_ArrowArray` structs from both versions are ABI-identical (they are `repr(C)`) but any
Rust-level API boundary between them will still fail because Rust considers the types distinct.

**Why it happens:**
`vortex-array 0.74` pins arrow-array/arrow-schema/arrow-data at `58.x`. If the workspace also has an
explicit `arrow = "58"` dependency that resolves to a different patch, Cargo may select two minor
versions within the 58.x range depending on the Cargo resolver version (v1 vs v2) and semver
compatibility rules. Patch releases within a major are generally compatible per semver, but Cargo's
resolver may still select distinct instances if the dependency graphs diverge.

**How to avoid:**
Add `[patch.crates-io]` entries in the workspace `Cargo.toml` to force all arrow-related crates to
exactly `58.3.0`. Run `cargo tree -d` to check for duplicate instances of any `arrow-*` crate before
the first FFI integration test. Pin `arrow-array`, `arrow-schema`, `arrow-data`, and `arrow` together
to the same patch version. If vortex-array 0.74 pins to a specific 58.x that differs from 58.3.0,
match vortex's pin exactly using `Cargo.lock` — do not fight it.

**Warning signs:**
- Compile error: "expected `arrow_data::ArrayData`, found `arrow_data::ArrayData`" (same name, different
  crate instance).
- `cargo tree -d` shows two instances of `arrow-array` in the dependency tree.
- `to_ffi` accepts your `ArrayData` in unit tests but rejects it after adding a new vortex crate
  dependency.

**Phase to address:**
Phase 1 (project scaffold). Run `cargo tree -d | grep arrow` as part of the initial Cargo.toml setup
checklist. Do not proceed to FFI integration until the tree is clean.

---

### Pitfall 10: Sliding Into MLIR / Verifier / Sandbox Work (Scope Trap)

**What goes wrong:**
The design doc (§5, §7, §8, §13) describes a full safety story: formal termination proofs, a verifier
over L1/L2, MLIR lowering to native code, sandbox isolation. All of this is explicitly out of scope
for MVP0. The trap: when the L1 interpreter is working, it becomes tempting to add "just one"
termination check, or to model L1 as an actual bytecode format with a verifier, or to invoke MLIR to
speed up the decode loop. Each addition is individually small but collectively pulls MVP0 off the
"does the chain produce correct SQL results" acceptance bar and onto the "can we prove safety" bar,
which is a different project.

The second form of this trap: the "single encoded array" scope becomes a "let's also handle the
file layout tree" scope because the demo wants to read real Parquet or a real Vortex file. This is
also explicitly out of scope.

**Why it happens:**
The design doc is compelling. Reviewers and collaborators who read it naturally want to build the whole
thing. Each individual "just add X" request sounds reasonable in isolation. There is no explicit
rejection criterion in the code; nothing in the build system enforces the narrow scope.

**How to avoid:**
The PROJECT.md "Out of Scope" list is the enforcement artifact. Every PR must be checked against it.
The acceptance criterion is narrow and concrete: "DuckDB SELECT over a Loom-decoded column matches
Vortex's decoder row-for-row for the specified encodings." If a proposed change does not contribute
directly to that acceptance criterion, it is deferred. Do not create any file, module, or crate with
names like `verifier`, `mlir`, `jit`, `sandbox`, `layout_tree`, or `vortex_file` in MVP0.

**Warning signs:**
- A PR description includes "while we're here, let's also support reading .vortex files."
- A module named `verifier.rs` or `ir.rs` appears that contains abstract type checking logic.
- Build times increase from seconds to minutes because LLVM is being linked.
- MLIR or inkwell appears in Cargo.toml.
- The test matrix grows to "also test the file reader integration" rather than "test the decode chain."

**Phase to address:**
Every phase. The MVP0 project scope is the anti-scope-creep contract for all phases.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `.unwrap()` in decode path | Faster to write | Process abort instead of SQL error when bad data hits | Never in `extern "C"` functions; acceptable in internal unit-test-only code paths |
| Skipping `catch_unwind` wrapper on `extern "C"` fns | Less boilerplate | Any panic kills DuckDB process | Never acceptable for functions called from DuckDB |
| Constructing test arrays in Rust without a serialized format | Simpler test setup | Demo requires real serialized input; gap surfaces late | Acceptable for unit tests; need a serialization strategy before the public demo |
| Not checking `array.validity()` in L1 loop | Works on non-nullable test data | Null rows become garbage values silently | Never — always check validity |
| Using `vortex-array`'s `into_canonical()` instead of Loom's L1 loop for initial tests | Faster to reach a passing test | Defeats the purpose of MVP0: the point is to prove Loom's decoder works | Never — reference and Loom decoder must be independent code paths |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Arrow C Data Interface | Calling `release` on child arrays directly from C++ | Only call `release` on base arrays; the release callback walks children |
| Arrow C Data Interface | Holding `FFI_ArrowSchema` pointer after calling its `release` | Call `release` once and immediately set `release = nullptr`; treat it as consumed |
| DuckDB `arrow_scan` | Passing a raw `ArrowArray*` instead of `ArrowArrayStream*` | `arrow_scan` expects a stream pointer; use `FFI_ArrowArrayStream` or the `ArrowToDuckDB` helper |
| Rust staticlib in C++ extension | Linking Rust's `libstd` allocator against DuckDB's jemalloc | Override with `#[global_allocator] static A: std::alloc::System = std::alloc::System;` |
| cbindgen header | `FFI_ArrowArray` type not matching between Rust and C++ | Do not define `FFI_ArrowArray` in the cbindgen header; use DuckDB's own Arrow headers or the arrow-rs `ffi.h` directly |
| vortex-array encoding dispatch | Calling `BitPackedArray::try_from(&array)` on an array that is actually a `FoRArray` | Check `array.encoding().id()` before downcasting; use the encoding id constants from each crate |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Cloning `ArrayData` before FFI export | Extra allocation + copy before every scan | Use `into_data()` which consumes and moves, not `data().clone()` | Every scan call — will be obvious from profiling but may hide correctness issues |
| Building Arrow typed builder output row-by-row across the FFI call per-element | Slow decode | Fill the entire column in one L1 pass before any FFI handoff | Invisible at small test sizes; noticeable at demo scale |
| Importing `vortex-file` for serialization creates transitive flatbuffer parse overhead | Slow test setup | Use in-memory programmatic array construction for all tests | From the first test that uses file I/O |

---

## "Looks Done But Isn't" Checklist

- [ ] **FFI release path:** The `release` callback fires on DuckDB's query completion path, not just the
  happy scan path — verify by intentionally cancelling a query mid-scan.
- [ ] **Null handling:** At least one test vector per L1 encoding (bitpack, FOR, dict, RLE) has nulls in
  it; the row-for-row comparison checks `IS NULL` rows, not just non-null values.
- [ ] **FSST edge cases:** Empty string, all-escape-sequence string, and max-length (8-byte symbol) string
  are in the FSST test vectors.
- [ ] **DuckDB version match:** The extension loads cleanly against the DuckDB binary used for the demo,
  not just the one embedded in the build system's test runner.
- [ ] **Cargo tree clean:** `cargo tree -d | grep arrow` produces no duplicate entries.
- [ ] **No vortex-file in Cargo.lock:** `grep vortex-file Cargo.lock` returns nothing.
- [ ] **panic=abort set:** `[profile.release] panic = "abort"` is in the decoder's Cargo.toml.
- [ ] **Reference decoder is independent:** The Vortex oracle path uses `into_canonical().into_arrow()`;
  the Loom path uses the Loom L1 loop. They share no code other than the input array construction.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Double-free in FFI release callback | MEDIUM | Add AddressSanitizer build (`RUSTFLAGS="-Z sanitizer=address"` + clang ASan on C++); bisect to the specific `write`/drop site |
| Schema lifetime crash | LOW | Add a RAII wrapper in C++ that calls `release` in its destructor; never hold raw `FFI_ArrowSchema` past the bind callback |
| Panic kills DuckDB process | LOW | Add `catch_unwind` at all `extern "C"` entry points; convert panics to error codes returned to C++ |
| DuckDB extension version mismatch | LOW | Set `DUCKDB_GIT_VERSION=v1.5.3` and run `make update_duckdb_headers`; rebuild extension |
| arrow-rs version skew | LOW-MEDIUM | Add `[patch.crates-io]` for all arrow-* crates; run `cargo tree -d` until clean |
| FSST decode mismatch | MEDIUM | Use `vortex-fsst`'s `FsstEncoding` as the bit-for-bit oracle; add test cases for the failing input pattern |
| Scope creep into vortex-file | MEDIUM | Remove `vortex-file` from Cargo.toml; rebuild in-memory test fixture using programmatic array construction |
| Scope creep into MLIR | HIGH | Delete any MLIR-related code; re-anchor to the acceptance criterion; the verifier/JIT is a next milestone |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| FFI release callback / double-free (P1) | Phase 1: FFI boundary setup | Unit test that manually calls `release` on exported array outside DuckDB |
| Schema/array lifetime mismatch (P2) | Phase 1: FFI boundary setup | Test fixture that drops Rust schema before C++ reads it |
| Panic across extern "C" (P3) | Phase 1: FFI boundary setup | Force a panic inside the decode path in a test; verify DuckDB returns SQL error not process abort |
| DuckDB ABI version lock (P4) | Phase 2: DuckDB extension scaffold | `make test` and then load extension in the pinned DuckDB binary — both must pass |
| Symbol clash / allocator mismatch (P5) | Phase 2: DuckDB extension scaffold | `nm -g libloom_decoder.a | grep -i malloc`; should be absent |
| FSST little-endian / not prod ready (P6) | Phase 3: FSST L2 kernel | All FSST test vectors compared byte-for-byte against vortex-fsst oracle |
| Vortex validity / null handling (P7) | Phase 3: L1 decode loop | Nullable test column per encoding; COUNT(*) vs COUNT(col) in DuckDB SQL harness |
| vortex-file scope creep (P8) | Phase 1: project scaffold | `grep vortex-file Cargo.lock` returns nothing at end of each phase |
| arrow-rs version skew (P9) | Phase 1: project scaffold | `cargo tree -d | grep arrow` returns zero duplicate entries |
| MLIR/verifier scope trap (P10) | Every phase | PR review against PROJECT.md Out of Scope list; acceptance criterion is "row-for-row SQL match" only |

---

## Sources

- [Apache Arrow C Data Interface specification](https://arrow.apache.org/docs/format/CDataInterface.html) — canonical release callback ownership rules (HIGH confidence)
- [RUSTSEC-2022-0012: arrow2 double-free](https://rustsec.org/advisories/RUSTSEC-2022-0012.html) — confirmed double-free pattern from Clone + Drop on FFI struct (HIGH confidence)
- [arrow-rs ffi module source](https://arrow.apache.org/rust/src/arrow_data/ffi.rs.html) — `to_ffi` implementation and memory model (HIGH confidence)
- [DuckDB PR #15632: Fix ArrowArray lifetime bug in arrow scan](https://github.com/duckdb/duckdb/pull/15632) — confirmed DuckDB-side child vector lifetime bug (HIGH confidence)
- [DuckDB Issue #16337: version mismatch building extensions](https://github.com/duckdb/duckdb/issues/16337) — exact error message and fix for ABI version mismatch (HIGH confidence)
- [Rust RFC 2945: C-unwind ABI](https://rust-lang.github.io/rfcs/2945-c-unwind-abi.html) — panic across FFI boundary semantics (HIGH confidence)
- [Rust Reference: panic behavior](https://doc.rust-lang.org/reference/panic.html) — extern "C" function abort-on-unwind guarantee (HIGH confidence)
- [spiraldb/fsst README](https://github.com/spiraldb/fsst) — "not production ready" and little-endian-only caveats (HIGH confidence)
- [DuckDB jemalloc extension docs](https://duckdb.org/docs/current/internals/jemalloc) — allocator model and static linking (MEDIUM confidence)
- [Apache Arrow C stream interface](https://arrow.apache.org/docs/format/CStreamInterface.html) — schema/array independent lifetime spec (HIGH confidence)
- [Apache Arrow FFI stream struct](https://arrow.apache.org/rust/arrow/array/ffi_stream/struct.FFI_ArrowArrayStream.html) — Rust FFI stream API (HIGH confidence)
- [Vortex file format spec](https://docs.vortex.dev/specs/file-format) — footer/layout-tree requirement for file reads (MEDIUM confidence)
- [Vortex arrays concept doc](https://docs.vortex.dev/concepts/arrays) — encoding structure and validity model (MEDIUM confidence)

---
*Pitfalls research for: Loom MVP0 — Rust decoder core + Arrow C Data Interface FFI + C++ DuckDB extension*
*Researched: 2026-06-07*
