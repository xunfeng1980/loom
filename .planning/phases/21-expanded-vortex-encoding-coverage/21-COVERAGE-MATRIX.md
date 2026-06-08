# Phase 21 Coverage Matrix

## Scope

Phase 21 expands the real Vortex reader coverage matrix beyond the Phase 18
non-null primitive and primitive-struct slice. The goal is not arbitrary Vortex
support. The goal is a finite, reviewer-visible matrix where every shape records
reader support, artifact emission, oracle evidence, and native lowering
disposition separately.

## Coverage Dimensions

Each covered Vortex shape is classified by:

- dtype kind and nullability,
- root layout encoding and layout class,
- array/encoding family,
- split/chunk presence,
- statistics presence,
- reader support state,
- artifact emission kind,
- oracle evidence status,
- native lowering disposition.

Reader facts are Loom-owned strings/enums and must not expose public Vortex Rust
types outside `loom-vortex-ingress`.

## Support States

- `accepted`: valid Vortex input and current Loom reader can emit a verifier
  routed artifact.
- `unsupported`: valid Vortex input and reader facts are available, but current
  Loom cannot emit a complete artifact.
- `rejected`: input cannot be opened as valid Vortex.

Support state is about reader/artifact eligibility. It is not native execution
permission.

## Emission Kinds

- `none`: no `.loom` bytes may be emitted.
- `LMP1`: single-column layout payload wrapped in `LMC1`.
- `LMT1`: table payload wrapped in `LMC1`.

Phase 21 may also record an emission disposition:

- `none`: no emission.
- `canonical-raw`: Vortex scan/canonical rows are emitted as Loom raw layout.
- `canonical-table`: Vortex scan/canonical rows are emitted as a Loom table.
- `structured-layout`: original encoding is represented as a Loom L1 layout
  such as dictionary, run-end, bitpack, or FOR.

Canonicalized emission is a semantic bridge backed by verifier/oracle evidence;
it is not a claim that native lowering understands the original Vortex encoding.

## Lowering Dispositions

- `interpreter-only`: Loom can emit/decode through the interpreter, but Phase 20
  native lowering does not support this shape yet.
- `production-lowering-supported`: current Phase 20 production native-lowering
  seed supports the emitted artifact shape.
- `fail-closed/deferred`: valid input remains fact-bearing but does not emit or
  lower in Phase 21.

## Initial Matrix

| Priority | Shape | Reader facts | Emission | Lowering disposition | Oracle evidence |
|---|---|---|---|---|---|
| P1 | Non-null primitive `i32/i64/f32/f64` | accepted | `LMP1` / `canonical-raw` | `production-lowering-supported` | Vortex scan rows |
| P1 | Non-null primitive struct/table | accepted | `LMT1` / `canonical-table` | `production-lowering-supported` | Vortex scan rows |
| P1 | Nullable primitive `i32/i64/f32/f64` | fact-bearing | none unless validity is fully represented | `fail-closed/deferred` first | null-preserving oracle required |
| P1 | Chunked/split primitive | fact-bearing | none unless row order is deterministic | `fail-closed/deferred` or `interpreter-only` | row-order oracle required |
| P2 | Dictionary primitive | fact-bearing | structured `Dictionary` if facts are complete | `interpreter-only` first | code/value oracle required |
| P2 | RunEnd / sequence primitive | fact-bearing | structured `RunEnd` or equivalent if facts are complete | `interpreter-only` first | expanded-row oracle required |
| P2 | Bitpack integer | fact-bearing | structured `BitPack` if facts are complete | `interpreter-only` until native delta | unpack oracle required |
| P2 | FOR integer | fact-bearing | structured `FrameOfReference` if facts are complete | `interpreter-only` until native delta | decoded-row oracle required |
| P3 | UTF-8 / VarBin / FSST-compatible | fact-bearing | only with Loom-owned params | `interpreter-only` or `fail-closed/deferred` | string oracle required |
| P3 | ALP / PCodec floats | fact-bearing | only with Loom-owned params | `interpreter-only` or deferred | float oracle required |
| P4 | Zoned/statistics layouts | fact-bearing | none unless child data is supported | ABI handoff only | metadata evidence |
| P4 | Custom/extension/WASM encodings | unsupported facts | none | `fail-closed/deferred` | unsupported diagnostic |

## Implemented Matrix

Phase 21 implemented the following finite matrix. The reader may canonicalize
some real Vortex encoded arrays into raw Loom artifacts after Vortex scan oracle
evidence. That is not the same as native support for the original encoding.

| Shape | Reader support | Emission | Lowering disposition | Evidence |
|---|---|---|---|---|
| Non-null primitive `i32/i64/f32/f64` | accepted | `LMP1` / `canonical-raw` | `production-lowering-supported` | Vortex scan oracle + artifact verifier |
| Non-null primitive struct/table | accepted | `LMT1` / `canonical-table` | `production-lowering-supported` | Vortex scan oracle + artifact verifier |
| Nullable primitive `i32/i64/f32/f64` | unsupported, fact-bearing | none | `fail-closed/deferred` | null-preserving oracle, no artifact emission |
| Chunked primitive `i32` fixture | accepted when canonicalized | `LMP1` / `canonical-raw` | root-layout dependent: production only when canonical primitive; otherwise `interpreter-only` | row-order oracle |
| Dictionary primitive fixture | accepted when canonicalized | `LMP1` / `canonical-raw` | production only when Vortex file exposes canonical primitive; original dictionary native support remains deferred | row oracle + artifact verifier |
| Run-end/RLE primitive fixture | accepted when canonicalized | `LMP1` / `canonical-raw` | production only when Vortex file exposes canonical primitive; original run-end native support remains deferred | row oracle + artifact verifier |
| Bitpack integer fixture | accepted when canonicalized | `LMP1` / `canonical-raw` | production only when Vortex file exposes canonical primitive; original bitpack native support remains deferred | row oracle + Phase 20 fail-closed native gate |
| FOR integer fixture | accepted when canonicalized | `LMP1` / `canonical-raw` | production only when Vortex file exposes canonical primitive; original FOR native support remains deferred | row oracle + Phase 20 fail-closed native gate |
| UTF-8 / VarBin | unsupported, fact-bearing | none | `fail-closed/deferred` | valid file facts, no Loom-owned string params |
| ALP/PCodec/FSST-compatible compression via real Vortex file | not implemented in Phase 21 | none | `fail-closed/deferred` | deferred until Loom-owned params can be extracted and verified |

## Native Delta Backlog

Phase 23 must treat dictionary, run-end/RLE, bitpack, FOR, ALP/PCodec, and
string compression as backend deltas unless a Phase 21 fact explicitly says
`production-lowering-supported` for the emitted artifact shape. The current
Phase 20 native gate still rejects complex structured kernels directly.

## Oracle Evidence

Accepted emission requires Vortex scan oracle evidence for row values and null
positions where applicable. Unsupported valid inputs may still have reader facts
without oracle equivalence. Rejected inputs have diagnostics only.

## Native Handoff

Phase 22 consumes split/chunk/statistics facts for host runtime ABI decisions:
projection, predicate pushdown, row partitioning, and concurrency ownership.

Phase 23 consumes the native delta backlog: each newly emitted encoding must say
whether it requires a production `loom.decode` dialect/native kernel delta or
is intentionally interpreter-only.

## Non-Goals

- Arbitrary Vortex support.
- WASM decompression or extension execution.
- New solver strategy.
- Production compiled ODS dialect implementation.
- `melior` pass pipeline, LLVM lowering, or LLVM/JIT backend implementation.
- DuckDB native execution.
- Iceberg or multi-engine query integration.
