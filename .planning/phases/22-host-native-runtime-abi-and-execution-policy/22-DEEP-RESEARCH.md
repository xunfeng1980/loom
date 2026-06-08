# Phase 22 Deep Research Appendix: Runtime ABI, Papers, Projects, and Best Practices

**Date:** 2026-06-08  
**Phase:** 22 — Host Native Runtime ABI and Execution Policy  
**Status:** Retrospective research appendix; Phase 22 remains complete.

## Executive Takeaway

The Phase 22 direction is sound: Loom needs a host-neutral runtime contract above
Arrow C Data, with verifier-gated planning, explicit projection/predicate/split
policy, cache identity, diagnostics, and opaque handles. External evidence adds
one sharper recommendation:

**Do not freeze the public C ABI until Phase 24 DuckDB and at least one second
consumer pressure test the same model.** Freeze the semantic contract first,
then harden the C ABI through layout/version tests, capability negotiation, and
strict ownership rules.

The current Rust model is already the right center of gravity. The C header
should stay a sketch until it can express:

- ABI version negotiation and capability discovery;
- stable struct sizes or reserved fields for every by-value request/result;
- explicit diagnostic string ownership;
- cancellation and interruptibility;
- allocator/memory-domain policy;
- Arrow C Data export rules for move/release and schema lifetime;
- reentrancy/thread-safety rules per handle kind;
- a second host adapter that is not DuckDB.

## Related Papers

### MonetDB/X100: Hyper-Pipelining Query Execution

Boncz, Zukowski, and Nes established the modern vectorized execution argument:
avoid row-at-a-time interpreter overhead by processing vectors/batches as the
physical unit of execution.

Phase 22 implication:

- `next_batch` should stay batch-oriented; do not add row callbacks to the core
  ABI.
- Split planning should expose enough shape for later morsel/vector scheduling.
- Batch size policy should become explicit in Phase 23/24, even if it remains a
  hint rather than a guarantee.

Source: https://ir.cwi.nl/pub/16497

### Velox: Meta's Unified Execution Engine

Velox is directly relevant because it is an engine-agnostic native execution
library consumed by multiple systems. The paper frames the reusable boundary as
typed vectors, expression evaluation, operators, I/O, resource management, and a
host-provided optimized plan.

Phase 22 implication:

- Loom should not let DuckDB become the control plane. The host owns SQL,
  optimizer, scheduling, and engine-specific semantics; Loom owns verified
  decode execution and output ownership.
- Use a data-plane contract: verified artifact facts plus query-shape envelope
  in, Arrow-compatible batches out.
- Treat semantics as a product surface. Velox exists partly to reduce divergent
  behavior across engines; Loom's verifier and diagnostics should serve the
  same role for decode behavior.
- Encoded/compressed execution is a future-native-backend concern, not a Phase
  22 ABI requirement. The ABI should carry enough layout/facts to allow it.

Source: https://vldb.org/pvldb/vol15/p3372-pedreira.pdf

### Apache Arrow DataFusion: Fast, Embeddable, Modular Analytic Query Engine

DataFusion is useful less as a target and more as a modularity case study: it
separates logical plans, execution plans, extension APIs, streams, partitions,
and Arrow `RecordBatch` output.

Phase 22 implication:

- Keep Loom's runtime plan distinct from host logical/physical plans. A host can
  lower its own plan into Loom's narrow projection/predicate/split envelope.
- Partition/split count belongs in the plan contract because it shapes worker
  creation, cache identity, and concurrency safety.
- Extension points should be semantic, not type-leaky: pass capabilities,
  fingerprints, and envelopes rather than Rust traits, DuckDB structs, MLIR
  handles, or Vortex objects.

Source: https://systemxlabs.github.io/blog/datafusion-paper/apache-datafusion-query-engine.pdf

### Zero-Cost, Arrow-Enabled Data Interface for Apache Spark

This line of work reinforces Arrow as an interoperability layer for eliminating
avoidable copies between systems, but it also shows that the data interface and
the execution policy are different layers.

Phase 22 implication:

- Arrow C Data is the batch handoff layer. Runtime lifecycle, cache, fallback,
  cancellation, and diagnostics must remain Loom-owned.
- Zero-copy is valuable only when ownership is precise. A single double-release
  path is worse than a copy.

Source: https://arxiv.org/abs/2106.13020

## Related Projects

### Apache Arrow C Data and C Stream

Arrow C Data is the strongest existing ABI precedent for Loom output. It is
ABI-stable, language-neutral, and release-callback driven. The C Stream
Interface adds blocking pull-style batch iteration, but explicitly does not
assume thread safety.

Phase 22 implication:

- Keep `loom_runtime_batch_export_arrow` as an adapter, not the whole runtime.
- Consumers must call only the base `release` callback, never child callbacks.
- The producer must place lifetime bookkeeping in `private_data`, mark released
  structures by nulling `release`, and tolerate bitwise moves of the base
  `ArrowArray`.
- Parallelism should happen through Loom scan/worker handles or independent
  streams; never assume one Arrow stream can be concurrently pulled.

Sources:

- https://arrow.apache.org/docs/13.0/format/CDataInterface.html
- https://arrow.apache.org/docs/20.0/format/CStreamInterface.html

### DuckDB C Table Functions

DuckDB's API validates the Phase 22 split between bind-time validation,
init-time projection/split planning, local worker state, and scan execution.
Projection pushdown is only available when declared, and DuckDB provides the
projected column list during init. It also exposes thread-local init and
`max_threads`.

Phase 22 implication:

- Phase 24 should map DuckDB bind data to `RuntimePlan`, init data to
  `ScanHandle`, and local init data to `WorkerHandle`.
- DuckDB's bind data is read-only during execution; Loom should mirror that by
  making plan/scan-global state immutable and putting mutable cursor state in
  worker handles.
- Projection order from DuckDB must feed `ProjectionSet` and the runtime cache
  key exactly, not be reconstructed from output names.

Source: https://duckdb.org/docs/current/clients/c/table_functions

### ADBC and nanoarrow

ADBC is a useful ABI design precedent because it separates API-standard version
from component implementation versions, uses a self-contained C header, integer
status codes, optional detailed error objects, and Arrow output. nanoarrow is a
small vendorable C library around Arrow C Data/Stream and documents that objects
are generally not thread-safe unless stated otherwise.

Phase 22 implication:

- Add `loom_runtime_get_version` and `loom_runtime_get_capabilities` before ABI
  freeze.
- Distinguish standard ABI version, implementation version, backend identity,
  Arrow library version, and target/toolchain identity.
- Prefer integer status codes plus optional diagnostic objects over returning
  borrowed strings as the only error channel.
- Default handle methods to not thread-safe unless the handle kind explicitly
  states otherwise.

Sources:

- https://arrow.apache.org/docs/14.0/format/ADBC.html
- https://arrow.apache.org/docs/16.0/format/ADBC/C.html
- https://arrow.apache.org/nanoarrow/main/reference/c.html

### Substrait

Substrait is a cross-language relational algebra serialization project. It is
not the right runtime ABI for Loom's narrow decode contract, but it is relevant
as a warning: a portable plan format needs versioning, extension policy, and
clear breaking-change rules.

Phase 22 implication:

- Loom's predicate envelope should remain intentionally smaller than Substrait.
  Accepting arbitrary host expressions would silently import host semantics into
  Loom.
- If Phase 26/27 need richer predicates, add a versioned envelope with explicit
  unsupported/fail-closed behavior.

Source: https://substrait.io/spec/specification/

### Vortex

Vortex is the closest source-format pressure test: compressed arrays, layouts,
statistics, scan API, filter pushdown, projection pushdown, and query-engine
integration are first-class concepts.

Phase 22 implication:

- Runtime plans should consume Vortex reader facts only through Loom-owned
  fingerprints and dispositions, preserving the existing dependency boundary.
- Split/chunk/statistics facts should shape scheduling and pruning decisions,
  but not become direct Vortex object handles in `loom-core` or `loom-ffi`.
- Predicate pushdown must distinguish "statistics pruning" from "row-level
  predicate evaluation"; they have different correctness obligations.

Source: https://docs.vortex.dev/concepts/

### Apache Gluten + Velox

Gluten shows a production pattern for native execution under a host scheduler:
the host engine remains responsible for planning and orchestration, while native
backends execute supported physical fragments and fall back for unsupported
features.

Phase 22 implication:

- Loom's native path should be per-artifact/per-fragment gated, not a global
  "native on" switch.
- Fallback must be observable and policy-controlled, especially for lakehouse
  formats where partial support can otherwise hide semantic gaps.

Source: https://apache.github.io/gluten/get-started/Velox.html

## ABI Best Practices for Loom

### 1. Version and Capability Negotiation

Before freezing `loom_runtime.h`, add:

- `loom_runtime_abi_version()`;
- `loom_runtime_implementation_version()`;
- `loom_runtime_get_capabilities(...)`;
- a request field for the caller's max supported ABI version;
- reserved flags/fields for forward-compatible structs.

Treat `RuntimeAbiVersion { major, minor }` like an API standard version:
backward-compatible additions increment minor; incompatible C ABI changes
increment major.

### 2. Opaque Handles, No Cross-Boundary Rust/C++ Objects

Keep:

- `LoomRuntimePlan`
- `LoomRuntimeScan`
- `LoomRuntimeWorker`
- `LoomRuntimeBatch`

opaque in C. Do not expose Rust enums by layout, C++ classes, `std::string`,
`Vec`, `String`, trait objects, MLIR handles, Vortex objects, DuckDB
`DataChunk`, or StarRocks chunks in the host-neutral ABI.

Use `#[repr(C)]` only for plain C-compatible request/result structs. Generate
or validate headers with `cbindgen`/bindgen-style checks.

### 3. Status Codes Plus Owned Diagnostics

`LoomRuntimeStatus` should become a richer stable enum:

- `OK`
- `INVALID_ARGUMENT`
- `INVALID_STATE`
- `UNSUPPORTED`
- `VERIFIER_REJECTED`
- `CONSTRAINT_REJECTED`
- `TOOLCHAIN_MISMATCH`
- `CACHE_MISMATCH`
- `CANCELLED`
- `INTERNAL`

Detailed diagnostics should be retrievable as stable-code/path/message records.
If messages are allocated, the ABI needs a corresponding release function. If
messages are borrowed, the lifetime must be tied to a specific handle and stated
explicitly.

### 4. Arrow Export Ownership Rules

For `loom_runtime_batch_export_arrow`:

- caller allocates the `ArrowArray`/`ArrowSchema` shells;
- Loom writes producer-owned members and `private_data`;
- caller releases exactly once through base release callbacks;
- caller may move the base structs only under Arrow C Data move rules;
- exported buffers are immutable;
- batch release and Arrow release ordering must be specified.

The existing project already guards against redefining Arrow FFI structs and
tracks the historic Arrow FFI double-free pitfall. Keep that gate.

Related risk source: https://rustsec.org/advisories/RUSTSEC-2022-0012.html

### 5. Thread Safety by Handle Kind

Default all handles to single-thread-affine unless stated otherwise:

| Handle | Recommended rule |
| --- | --- |
| `Plan` | immutable after creation; shareable if implementation marks it thread-safe |
| `Scan` | immutable scan-global state plus synchronized split allocator, or single-thread |
| `Worker` | thread-affine mutable cursor state |
| `Batch` | immutable after production; release may happen on any host thread only if documented |

Phase 24 should test DuckDB local-init mapping against these rules. A later
StarRocks adapter should be the second-consumer test.

### 6. Cancellation and Interruptibility

Native execution needs a cancellation hook before production use:

- request-level cancellation token or callback;
- `next_batch` returns `CANCELLED`;
- cancellation is best-effort but must leave handles releasable;
- diagnostics record whether cancellation happened before or during native
  preparation/execution.

This should be in Phase 23/24 before long-running native kernels.

### 7. Allocator and Memory Domain

The ABI should state which side allocates and frees:

- request buffers: host-owned, valid for the call unless retained explicitly;
- plan/scan/worker/batch handles: Loom-owned, released by Loom functions;
- diagnostic strings: borrowed from handle or Loom-owned with release function;
- Arrow buffers: Loom/Arrow-owned through release callback;
- host-native vector adapters: future host-owned or callback-owned, never
  inferred.

Allocator mismatch is an ABI bug class; never require the host to `free()` Loom
allocations unless Loom also exports the matching free function.

### 8. Cache Key as ABI Input, Not Debug Output

The current `RuntimeCacheKey` direction is right. Freeze cache key inputs before
freezing the hash implementation:

- canonical input schema;
- string escaping rules;
- byte ordering for numeric fields;
- sorted CPU feature policy;
- target triple spelling;
- ABI version inclusion;
- policy inclusion.

The hash algorithm can change under a new cache-key version, but stale native
artifacts must never be accepted after a semantic input changes.

## Gap Analysis Against Current Phase 22

Already well covered:

- host-neutral Rust model;
- native/interpreter/fail-closed decision policy;
- projection order and cache key inclusion;
- predicate envelope with fail-closed/scan-all policy;
- row-range/full-scan split model;
- unsafe concurrency diagnostics;
- non-frozen C ABI sketch;
- Arrow C Data as batch boundary rather than whole runtime.

Needs Phase 23/24 hardening:

- ABI version/capability query functions;
- reserved fields and size/version checks for C structs;
- diagnostic ownership/release contract;
- richer status code taxonomy;
- cancellation;
- allocator contract;
- thread-safety matrix in the public C header;
- Arrow export release-order tests across C/C++ consumer code;
- second-consumer validation before ABI freeze.

## Recommended Handoff Updates

Phase 23:

- Consume `RuntimePlan` and `RuntimeCacheKey` as mandatory backend inputs.
- Emit native artifacts only for `native-candidate` reports.
- Add cancellation and backend/toolchain identity into native prepare/execute.
- Add ABI layout tests even if the C ABI remains marked unstable.

Phase 24:

- Map DuckDB bind/init/local-init/function lifecycle directly to
  plan/scan/worker/next-batch.
- Enable projection pushdown only when runtime projection remapping is exact.
- Set DuckDB max threads from Loom split/concurrency planning, not independently.
- Test Arrow C Data release paths in C++ table-function error/cancel paths.

Phase 25:

- Add equivalence tests that compare native/interpreter output under the same
  runtime plan, projection, split, and predicate policy.
- Add stale-cache negative tests over every cache key semantic input.

Phase 27:

- Treat StarRocks or another non-DuckDB consumer as the ABI-falsification test.
  If it requires DuckDB-specific assumptions to be removed, revise the ABI
  before declaring it stable.

## References

- Apache Arrow C Data Interface: https://arrow.apache.org/docs/13.0/format/CDataInterface.html
- Apache Arrow C Stream Interface: https://arrow.apache.org/docs/20.0/format/CStreamInterface.html
- DuckDB C Table Functions: https://duckdb.org/docs/current/clients/c/table_functions
- Rustonomicon FFI: https://doc.rust-lang.org/nightly/nomicon/ffi.html
- Rustonomicon `repr(C)`: https://doc.rust-lang.org/beta/nomicon/other-reprs.html
- ADBC Specification: https://arrow.apache.org/docs/14.0/format/ADBC.html
- ADBC C API Specification: https://arrow.apache.org/docs/16.0/format/ADBC/C.html
- nanoarrow C API: https://arrow.apache.org/nanoarrow/main/reference/c.html
- Velox paper: https://vldb.org/pvldb/vol15/p3372-pedreira.pdf
- MonetDB/X100 paper record: https://ir.cwi.nl/pub/16497
- DataFusion paper: https://systemxlabs.github.io/blog/datafusion-paper/apache-datafusion-query-engine.pdf
- DataFusion docs: https://datafusion.apache.org/user-guide/introduction.html
- Substrait specification: https://substrait.io/spec/specification/
- Vortex concepts: https://docs.vortex.dev/concepts/
- Apache Gluten Velox backend docs: https://apache.github.io/gluten/get-started/Velox.html
- RUSTSEC-2022-0012 Arrow FFI double-free advisory: https://rustsec.org/advisories/RUSTSEC-2022-0012.html
