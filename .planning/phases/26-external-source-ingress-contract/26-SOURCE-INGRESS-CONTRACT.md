# Phase 26 Source Ingress Contract

**Status:** Normative for Phase 26.
**Scope:** Source-neutral ingress facts, diagnostics, support classification,
artifact emission handoff, oracle evidence, dependency isolation, and Phase 27
adapter obligations.

## Scope

Phase 26 defines how an external source adapter may describe source metadata,
classify support, and hand verifier-routed Loom artifacts to downstream decode,
artifact verification, and host execution phases.

The contract is source-neutral. Vortex is the current reference adapter, but the
generic contract vocabulary is `Source*` and `SourceIngress*`, not Vortex-named.
Adapter code may carry source-specific names internally. Reviewer-facing generic
reports must expose Loom-owned strings, enums, and primitive fields only.

The only accepted artifact target in this phase is `LMC1` wrapping either:

- `LMP1` for a single-column layout payload.
- `LMT1` for a table payload.

Every accepted artifact must be routed through the existing Loom artifact
verifier before the source-ingress report can claim `accepted`.

## Non-Goals

Phase 26 does not implement or expose:

- Lance or Parquet ingestion.
- Iceberg refs, table binding, or catalog semantics.
- MCAP, Zarr, LeRobot, or additional source readers.
- Object-store credentials, remote IO policy, or auth controls.
- Public SQL or public C/API expansion.
- Host-engine integration.
- Predicate pushdown.
- Parallel split execution.
- New native kernels or backend deltas.
- Arbitrary Vortex semantic compatibility.

These items remain deferred to later phases unless explicitly replanned.

## Trust Boundaries

External source metadata is untrusted until classified by an adapter. Source
facts are reviewer-visible evidence, not proof that bytes may be emitted.

Artifact bytes are trusted only after all of the following are true:

1. The source report status is `accepted`.
2. Emission kind is `LMP1` or `LMT1`.
3. Emission disposition describes the canonical or structured Loom artifact.
4. Artifact verification is required and accepted.
5. Oracle evidence for the accepted shape is present and accepted.

Unsupported valid sources may expose facts, but emit no `.loom` bytes. Rejected
malformed sources expose no trusted facts and no oracle evidence.

## Generic Type Vocabulary

The source-neutral contract is currently represented by `loom-source-ingress`.
The required public vocabulary is:

| Type | Contract role |
|---|---|
| `SourceIngressStatus` | Stable triad: `accepted`, `unsupported`, `rejected`. |
| `SourceIdentity` | Source kind, format, optional format version, optional fingerprint, and optional display path. |
| `SourceDiagnosticCode` / `SourceDiagnosticFamily` / `SourceDiagnostic` | Stable code, family, path, message, and optional source detail. |
| `SourceFacts` | Source identity, row count, schema facts, layout facts, segment facts, split facts, and coverage. |
| `SourceSchemaFact` | Logical schema or dtype summary without source SDK types. |
| `SourceLayoutFact` | Layout or physical organization summary without source SDK handles. |
| `SourceSegmentFact` | Physical segment byte ranges and ordering/overlap facts. |
| `SourceSplitFact` | Row split metadata as facts only, not an execution plan. |
| `SourceCoverage` | Support, emission, lowering, nullability, layout, encoding, split, and statistics classification. |
| `SourceEmissionKind` | `none`, `LMP1`, or `LMT1`. |
| `SourceEmissionDisposition` | `none`, `canonical-raw`, `canonical-table`, or `structured-layout`. |
| `SourceLoweringDisposition` | `interpreter-only`, `production-lowering-supported`, or `fail-closed/deferred`. |
| `SourceOracleStrategy` / `SourceOracleEvidence` | Oracle strategy and evidence status, separate from implementation success. |
| `SourceArtifactVerificationSummary` | Plain-data verifier handoff summary; `loom-core` does not depend on source ingress. |
| `SourceIngressReport` | Status, identity, optional facts, diagnostics, emission, verifier summary, and oracle evidence. |

No generic type may expose source SDK objects, host-engine handles, credentials,
or Arrow stream ownership objects.

## Report Invariants

`SourceIngressReport` must preserve these invariants:

- `accepted` requires `Some(SourceFacts)`.
- `accepted` requires `SourceEmissionKind::Lmp1` or `SourceEmissionKind::Lmt1`.
- `accepted` requires artifact verification to be required, accepted, and tied
  to non-empty artifact bytes.
- `accepted` requires accepted oracle evidence.
- `unsupported` may carry `Some(SourceFacts)` when the source was valid enough
  to inspect.
- `unsupported` must have emission kind `none`, emission disposition `none`,
  verifier summary `not-applicable`, and no accepted oracle evidence.
- `rejected` must not carry trusted facts.
- `rejected` must have emission kind `none`, emission disposition `none`,
  verifier summary `not-applicable`, and no oracle evidence.

Facts are useful for review and handoff. They are not a trust token.

## Accepted, Unsupported, and Rejected Semantics

### accepted

`accepted` means the adapter has enough trusted facts, the current Loom source
slice can produce a complete Loom artifact, the artifact is `LMC1` wrapping
`LMP1` or `LMT1`, the artifact verifier accepted it, and oracle evidence exists
for the accepted shape.

`accepted` does not mean public SQL expansion, native execution permission,
predicate support, split execution, or arbitrary source semantic compatibility.

### unsupported

`unsupported` means the input source is valid enough for facts or diagnostics,
but Phase 26 cannot emit a complete verified Loom artifact for that shape.

Unsupported valid sources may expose schema/layout/segment/split facts. They may
not expose partial artifact bytes. They may not claim accepted oracle evidence.

### rejected

`rejected` means the adapter could not open or parse the source into trustworthy
facts. Rejected reports include diagnostics, but no trusted facts, no artifact
bytes, and no oracle evidence.

## Source Identity and Facts

`SourceIdentity` describes provenance and format identity in bounded terms. Its
optional fingerprint is reserved for future adapters; Phase 26 does not define a
persistent content-hash or archive identity scheme.

`SourceFacts` can include schema, layout, physical segment, row split, coverage,
and statistics-presence facts. These facts are descriptive. They do not require
every source to look like Vortex. A source may expose schema-only facts,
row-group facts, fragment facts, chunk facts, or no split facts.

Split facts are metadata only in Phase 26. They must not trigger parallel split
execution.

## Diagnostics

Diagnostics must carry:

- stable code,
- stable family,
- path,
- message,
- optional source detail.

Generic code families cover open, read, schema, layout, support, conversion,
verification, and oracle failures. Source adapters may include source-specific
text in `source_detail`, but must not expose credentials, SDK handles, or
host-engine state.

## Emission Kind and Disposition

Emission kind records the Loom payload type:

- `none`: no `.loom` bytes may be emitted.
- `LMP1`: single-column layout payload, wrapped in `LMC1`.
- `LMT1`: table payload, wrapped in `LMC1`.

Emission disposition records how the source shape became a Loom artifact:

- `none`: no emission.
- `canonical-raw`: source-native rows were canonicalized into a Loom raw layout.
- `canonical-table`: source-native rows were canonicalized into a Loom table.
- `structured-layout`: source facts were represented as a Loom structured
  layout such as dictionary, run-end, bitpack, or frame-of-reference.

Canonical raw/table emission is a verifier-backed bridge. It does not imply
that arbitrary source semantics, storage modes, or encodings are now represented
inside Loom.

## Verifier Handoff

Accepted emission is verifier-routed:

1. The source adapter emits candidate `LMC1` bytes wrapping `LMP1` or `LMT1`.
2. The adapter immediately runs `verify_artifact` with the current Loom
   registry and verifier options.
3. Only accepted verifier output may populate
   `SourceArtifactVerificationSummary::accepted`.
4. Unsupported, rejected, malformed, verifier-failed, or oracle-failed paths
   return a `SourceIngressReport` without artifact bytes.

The generic `loom-source-ingress` crate stores verifier results as plain
source-contract data so `loom-core` remains source-neutral and does not depend
on source adapters.

## Oracle Evidence

Each adapter must declare an oracle strategy:

- `source-native-scan`: compare verified Loom output to source-native decoded
  rows.
- `arrow-scan`: compare against a source-provided Arrow scan path.
- `decoded-row-fixture`: compare against a stable decoded row fixture.
- `unsupported`: no accepted oracle evidence is available.

Oracle evidence is separate from implementation success. A source-native scan is
evidence for tests and reports; it must not become the Loom decode path or bypass
artifact verification.

## Lowering Disposition

Lowering disposition is descriptive metadata only. It describes the emitted Loom
artifact shape, not arbitrary compatibility with the original external source.

- `interpreter-only`: the emitted artifact is intended for Loom interpreter
  decode only.
- `production-lowering-supported`: the emitted Loom artifact shape is within the
  current production lowering slice.
- `fail-closed/deferred`: the shape is valid or fact-bearing but cannot lower in
  the current phase.

Phase 26 must not add native kernels, host execution routes, or public lowering
controls based on this metadata.

## Dependency Boundary

Source SDK dependencies belong only in source-specific adapter crates.

The following surfaces must remain free of source SDK and source-format
dependencies:

- `loom-core`
- `loom-ffi`
- `loom-source-ingress`
- DuckDB extension code
- public headers

The generic contract crate must not depend on Vortex, Lance, Parquet, Iceberg,
MCAP, Zarr, LeRobot, object-store SDKs, DuckDB, host-engine APIs, or native
backend crates.

The current Vortex adapter may depend on Vortex crates because it is the
source-specific proof adapter. That dependency direction must not invert.

## Adapter Obligations

Every future source adapter must:

1. Map source-specific metadata into `SourceIdentity` and `SourceFacts` without
   exposing SDK types.
2. Classify reports as `accepted`, `unsupported`, or `rejected`.
3. Emit no bytes for `unsupported` or `rejected`.
4. Emit only verifier-accepted `LMC1` wrapping `LMP1` or `LMT1` for `accepted`.
5. Record `SourceEmissionKind`, `SourceEmissionDisposition`, and
   `SourceLoweringDisposition` separately.
6. Declare oracle strategy and accepted oracle evidence for accepted emission.
7. Keep source SDK dependencies out of generic/core/ffi/DuckDB/public surfaces.
8. Preserve stable diagnostics with code/path/message.
9. Treat object-store credentials, public SQL/API, predicate pushdown, parallel
   split execution, and native kernels as out of scope unless a later phase
   explicitly changes the contract.

## Phase 27 Handoff

Phase 27 Lance/Parquet work must consume this contract rather than copying
Vortex-specific APIs. Lance and Parquet adapters must declare source identity,
facts, diagnostics, support state, emission kind/disposition, verifier handoff,
oracle evidence, and lowering disposition before any artifact is accepted.

Phase 27 may choose source-native or Arrow-scan oracle strategies for its own
files. It must still emit only verifier-accepted `LMC1` wrapping `LMP1` or
`LMT1` and must keep source SDK dependencies out of `loom-core`, `loom-ffi`,
`loom-source-ingress`, DuckDB extension code, and public headers.

Phase 27 must not treat canonical emission as arbitrary Lance/Parquet semantic
compatibility. It must state support matrices and fail closed for unsupported or
malformed inputs.
