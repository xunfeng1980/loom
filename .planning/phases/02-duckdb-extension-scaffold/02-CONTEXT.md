# Phase 2: DuckDB Extension Scaffold - Context

**Gathered:** 2026-06-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Build and load a **stub DuckDB v1.5.3 C++ extension** that links the Rust `libloom_ffi.a` staticlib (from Phase 1) and registers a `loom_scan` table function which: calls `loom_decode`, imports the resulting Arrow array across the C Data Interface, and exposes it to DuckDB as queryable rows — **without crashing**. This proves the full CMake + Rust-staticlib + DuckDB-ABI chain end-to-end before any real decode logic exists.

Requirements: DUCK-01 (extension pinned to v1.5.3 builds + loads), DUCK-02 (`loom_scan` invokes the Rust decoder and adopts the Arrow array zero-copy), DUCK-03 (releases the imported array on every teardown path — no leak, no double-free).

**Not this phase:** any real decode logic / L1 encodings (Phase 3), the Vortex reader, FSST (Phase 5). `loom_decode` still returns the hardcoded `Int32Array [1,2,3,null]` from Phase 1 — Phase 2 proves the *plumbing*, so `SELECT * FROM loom_scan(...)` returns exactly those 4 rows.

</domain>

<decisions>
## Implementation Decisions

### Arrow → DuckDB import path
- **D-01 (ORIGINAL):** wrap the imported `FFI_ArrowArray` + `FFI_ArrowSchema` in a one-shot `ArrowArrayStream` and feed DuckDB's built-in `arrow_scan` (public/stable surface), avoiding the version-fragile internal `ArrowToDuckDB()`.
- **D-01 (REVISED during Phase 2 execution — 2026-06-07):** Execution surfaced a hard blocker that wasn't known when D-01 was chosen: **DuckDB's `arrow_scan` requires a top-level STRUCT / record-batch Arrow schema, but `loom_decode` emits a BARE primitive Int32 array** (`format="i"`, `n_children=0`). The `arrow_scan` path therefore cannot consume `loom_decode`'s output as-is — the column would have to be wrapped in a record batch first. Wrapping a single hardcoded primitive column in a struct purely to satisfy `arrow_scan` is premature for this plumbing stub. **Decision (user, after review): adopt direct DataChunk population for the Phase 2 stub** (read the Arrow validity + values buffers and fill the DuckDB vector). The `arrow_scan` / streaming path is **deferred to Phase 3**, where real columnar decode naturally produces a record-batch-shaped output that `arrow_scan` can consume. No dead "stable-surface" scaffolding is retained — Phase 3 builds the stream path against the real output when it's actually needed. (This formally supersedes the earlier insistence on the stream path; the original rationale — stability / forward-reuse — is preserved by the Phase-3 tracked task rather than by premature stub code.)

### DuckDB acquisition
- **D-02:** Link against the **prebuilt DuckDB v1.5.3 release** library + headers, and load the extension into the matching **duckdb 1.5.3 CLI** with `allow_unsigned_extensions`. ABI matches because both sides are the exact 1.5.3 release. Fast, low-disk, reproducible — do NOT build DuckDB from source for MVP0.

### Build harness / layout
- **D-03:** A **hand-rolled minimal CMake** (not the official extension-template). The CMakeLists links `libloom_ffi.a` + `crates/loom-ffi/include/loom.h` against the prebuilt DuckDB lib and emits the loadable `*.duckdb_extension`. The C++ extension lives beside the Rust workspace (e.g. a top-level `duckdb-ext/` or `cpp/` dir). It must trigger `cargo build -p loom-ffi --release` so the staticlib is fresh before linking.

### loom_scan stub interface
- **D-04:** `loom_scan(VARCHAR)` — a single path/string argument, **accepted but ignored** in Phase 2 (the bytes aren't read yet because `loom_decode` returns the hardcoded array). Matches the ROADMAP acceptance shape `loom_scan('test.bin')` and becomes the real contract Phase 3+ fills in. Stable signature from the start.

### Teardown / ownership (DUCK-03)
- The C++ side must invoke the Arrow **release callbacks on every exit path** (success, error, query cancel). With the ArrowArrayStream approach the stream's `release` owns teardown of the array+schema; ensure exactly-once release and no double-free. Mirror the Rust-side ownership contract locked in Phase 1 (release callback is the sole owner after `ptr::write`).

### Claude's Discretion
- Exact CMake structure and the extension directory name/location.
- The precise `ArrowArrayStream` wrapper implementation (get_schema / get_next / release callbacks around the single decoded array).
- The local `allow_unsigned_extensions` load mechanics and how the extension's version/platform metadata is stamped to match the prebuilt 1.5.3 CLI.
- How the ABI/version pin is asserted in CI (e.g. a load smoke-test).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 1 artifacts (the integration surface this phase links against)
- `crates/loom-ffi/include/loom.h` — the generated, self-contained C header: `int32_t loom_decode(const uint8_t*, uintptr_t, FFI_ArrowArray*, FFI_ArrowSchema*)`, with forward-declared Arrow structs
- `crates/loom-ffi/src/ffi.rs` — the FFI contract + ownership protocol (i32 error codes, no `loom_free`, release callback owns teardown)
- `.planning/phases/01-scaffold-and-ffi-boundary/01-CONTEXT.md` — locked FFI contract decisions
- `.planning/phases/01-scaffold-and-ffi-boundary/01-SUMMARY.md` / `01-02-SUMMARY.md` — what was built (staticlib name `libloom_ffi.a`, not `libloom_decoder.a`)

### Stack, Architecture & Pitfalls (project research)
- `.planning/research/STACK.md` — DuckDB 1.5.3 C++ extension surface; built-in `arrow_scan` / `ArrowArrayStream` ingestion; **do NOT use** the deprecated `arrow` community extension; Rust staticlib + cbindgen + CMake link path
- `.planning/research/ARCHITECTURE.md` — the C++ extension component boundary, the Arrow C Data Interface seam, single-array transfer + release semantics
- `.planning/research/PITFALLS.md` — **DuckDB extension ABI is git-hash-keyed** (extension footer must match the loading binary exactly), allocator mismatch (System allocator already set in loom-ffi), release-callback double-free/leak
- `.planning/research/SUMMARY.md` — flags `ArrowToDuckDB()` include path as a tertiary uncertainty (the reason D-01 prefers the public `arrow_scan` stream path)

### Scope & requirements
- `design.md` §6 (typed Arrow output → C Data Interface), §9 (decoder ABI)
- `.planning/REQUIREMENTS.md` — DUCK-01, DUCK-02, DUCK-03
- `.planning/ROADMAP.md` Phase 2 — goal + success criteria (note: criterion text says `libloom_decoder.a`; the actual staticlib is `libloom_ffi.a`)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `libloom_ffi.a` (Rust staticlib) + `crates/loom-ffi/include/loom.h` — the link/include targets for the C++ extension. `loom_decode` is the single entry point; it already exports a real Arrow array with correct release ownership (verified by the Phase 1 roundtrip test).
- Phase 1's CI invariant script (`scripts/check-core-invariants.sh`) and GitHub Actions workflow — extend with a DuckDB load smoke-test rather than starting fresh.

### Established Patterns
- FFI ownership contract is locked (release callback sole owner after `ptr::write`); the C++ side must honor it on the import side.
- `panic = "unwind"` + boundary `catch_unwind` means a Rust-side panic returns a nonzero i32 — the C++ table function must check `loom_decode`'s return code and surface a DuckDB error rather than proceeding.

### Integration Points
- C++ extension → `loom_decode` (via `loom.h`) → `FFI_ArrowArray`/`FFI_ArrowSchema` → one-shot `ArrowArrayStream` → DuckDB rows.
- The extension's version/platform metadata must match the prebuilt duckdb 1.5.3 CLI or `LOAD` is rejected (ABI git-hash pin).

</code_context>

<specifics>
## Specific Ideas

- Acceptance shape (from ROADMAP): `LOAD 'loom_extension'; SELECT * FROM loom_scan('test.bin');` returns the 4 hardcoded rows (`1, 2, 3, NULL`) without crashing or an ABI-version error.
- Keep the C++ surface as thin as possible — it is a plumbing shim, not logic (mirrors the design's "thin FFI, logic stays in Rust").

</specifics>

<deferred>
## Deferred Ideas

**[TRACKED → Phase 3] arrow_scan / ArrowArrayStream import path.** D-01's original stream path was deferred here (see "D-01 REVISED" above). When Phase 3's real columnar decode produces a record-batch-shaped Arrow output (top-level struct schema), replace `LoomScan`'s direct DataChunk population with a one-shot (then multi-batch) `ArrowArrayStream` produce-callback factory delegated to DuckDB's built-in `arrow_scan`. This is the point where the stream path stops being premature and starts being the natural fit. Carried in STATE.md blockers/deferred.

**Research/planning note (not deferred, just a flag):** the "prebuilt lib + hand-rolled CMake" pairing has one real risk to confirm during research — that a locally-built **unsigned** extension can be loaded into the official prebuilt duckdb 1.5.3 CLI given matching version+platform metadata (the extension footer/`DUCKDB_EXTENSION_API` stamping). If the prebuilt CLI refuses unsigned local extensions outright on this platform, fall back to building duckdb from source at v1.5.3 (the rejected alternative in D-02).

</deferred>

---

*Phase: 02-duckdb-extension-scaffold*
*Context gathered: 2026-06-07*
