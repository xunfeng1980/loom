# Phase 26 Source Ingress Report

**Status:** Plan 26-04 evidence report.
**Contract:** `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-CONTRACT.md`
**Gate:** `scripts/source-ingress-contract-test.sh` is created in Plan 26-04
and will be wired into the main release gate by Plan 26-05.

## Executive Summary

Phase 26 now has a source-neutral ingress contract with a dependency-light
`loom-source-ingress` crate, a Vortex reference adapter that maps existing reader
facts into generic source reports, and verifier-routed accepted artifact handoff.

The accepted path is deliberately narrow: source adapters may return accepted
reports only after emitting `LMC1` wrapping `LMP1` or `LMT1`, running the artifact
verifier, and recording accepted oracle evidence. Unsupported valid sources may
expose facts but emit no bytes. Rejected malformed sources expose diagnostics
only and no trusted facts.

The contract does not implement Lance, Parquet, Iceberg, MCAP, Zarr, LeRobot,
object-store credentials, public SQL/API expansion, host-engine integration,
predicate pushdown, parallel split execution, new native kernels, or arbitrary
Vortex semantic compatibility.

## Implemented Artifacts

| Artifact | Role | Evidence |
|---|---|---|
| `crates/loom-source-ingress` | Generic source-neutral contract crate | Defines `SourceIngressReport`, `SourceFacts`, diagnostics, emission, lowering, oracle, and verifier summary data with no source SDK dependencies. |
| `crates/loom-vortex-ingress/src/source_contract.rs` | Vortex reference adapter mapping layer | Converts `VortexReaderFacts`, coverage, diagnostics, and reports into generic `Source*` types while preserving existing Vortex APIs. |
| `emit_source_ingress_lmc1_from_vortex_buffer` | Accepted artifact handoff helper | Emits through the existing Vortex path, immediately verifies `LMC1`, records oracle evidence, and returns artifact bytes only on accepted reports. |
| `crates/loom-source-ingress/tests/source_ingress_contract.rs` | Generic vocabulary and invariant tests | Locks stable strings, report invariants, and generic dependency hygiene. |
| `crates/loom-vortex-ingress/tests/source_ingress_contract.rs` | Vortex mapping tests | Verifies supported primitive/table mappings, unsupported valid reports, rejected reports, and source-neutral public vocabulary. |
| `crates/loom-vortex-ingress/tests/source_ingress_handoff.rs` | Verifier/oracle handoff tests | Verifies accepted `LMP1`/`LMT1` handoff, source-native oracle evidence, unsupported valid fail-closed behavior, and rejected malformed behavior. |
| `26-SOURCE-INGRESS-CONTRACT.md` | Normative reviewer contract | Records source-neutral model, trust boundaries, non-goals, adapter obligations, and Phase 27 handoff. |
| `scripts/source-ingress-contract-test.sh` | Plan 26-04 guard | Checks docs, implementation markers, focused tests, dependency boundaries, and API creep before Plan 26-05 release wiring. |

## Vortex Mapping

Vortex remains the first real adapter proving the generic contract. It is not the
generic vocabulary.

| Vortex evidence | Generic contract field | Current disposition |
|---|---|---|
| `VortexIngressStatus::{Accepted, Unsupported, Rejected}` | `SourceIngressStatus::{Accepted, Unsupported, Rejected}` | Stable triad preserved. |
| `VortexReaderFacts::source_kind` and `vortex_file_version` | `SourceIdentity::source_kind`, `format`, `format_version` | Source-specific identity is normalized as plain strings. |
| `VortexReaderDTypeFact` | `SourceSchemaFact` | Dtype/schema facts become generic logical kind, nullability, field names, and Arrow summary data. |
| `VortexReaderLayoutFact` | `SourceLayoutFact` | Layout encoding details become layout class, row count, children, physical refs, and metadata byte length. |
| `VortexReaderSegmentFact` | `SourceSegmentFact` | Physical byte ranges and ordering/overlap facts are preserved. |
| `VortexReaderSplitFact` | `SourceSplitFact` | Row split metadata is recorded as facts only, with no split execution. |
| `VortexEncodingCoverage` | `SourceCoverage` | Reader support, emission kind, emission disposition, lowering disposition, splits, stats, and notes are preserved. |
| `VortexReaderEmissionKind::{None, LMP1, LMT1}` | `SourceEmissionKind::{None, Lmp1, Lmt1}` | Only `LMP1` and `LMT1` can participate in accepted reports. |
| `VortexEmissionDisposition` | `SourceEmissionDisposition` | `canonical-raw`, `canonical-table`, `structured-layout`, and `none` remain distinct from support status. |
| `VortexLoweringDisposition` | `SourceLoweringDisposition` | Lowering remains descriptive metadata about the emitted Loom artifact shape. |
| Vortex scan tests | `SourceOracleEvidence` | Source-native scan is evidence, not the implementation path. |
| `verify_artifact` accepted report | `SourceArtifactVerificationSummary` | Accepted source reports require verifier-accepted artifact evidence. |

## Adapter Obligations

Any future source adapter, including Phase 27 Lance/Parquet adapters, must:

1. Map source metadata into Loom-owned `SourceIdentity` and `SourceFacts`.
2. Keep source SDK types, handles, and credentials out of generic reports.
3. Classify every input as `accepted`, `unsupported`, or `rejected`.
4. Emit no bytes for `unsupported` or `rejected`.
5. Emit only `LMC1` wrapping `LMP1` or `LMT1` for accepted reports.
6. Run artifact verification before returning accepted artifact bytes.
7. Record accepted oracle evidence for accepted reports.
8. Keep emission kind, emission disposition, and lowering disposition separate.
9. Preserve stable diagnostic code/path/message fields.
10. Keep source SDK dependencies out of `loom-core`, `loom-ffi`,
    `loom-source-ingress`, DuckDB extension code, and public headers.

## Accepted Emission Matrix

| Source shape | Source report | Emission | Verifier evidence | Oracle evidence | Notes |
|---|---|---|---|---|---|
| Non-null primitive Vortex `i32/i64/f32/f64` | `accepted` | `LMC1(LMP1)`, `canonical-raw` | `verify_artifact` accepted | Vortex source-native row scan | Current production lowering may describe the emitted raw Loom artifact shape, not arbitrary Vortex encoding compatibility. |
| Non-null primitive Vortex struct/table | `accepted` | `LMC1(LMT1)`, `canonical-table` | `verify_artifact` accepted | Vortex source-native table scan | Current table evidence is a canonical Loom table bridge. |

Canonical raw/table emission is not structured source semantic compatibility.
It proves safe, verifier-accepted Loom artifact creation for supported shapes.

## Unsupported and Rejected Matrix

| Case | Source report | Facts | Emission | Oracle | Result |
|---|---|---|---|---|---|
| Valid Vortex UTF-8 source not covered by current emission slice | `unsupported` | Present | none | none | Fact-bearing fail-closed report; no artifact bytes. |
| Valid Vortex table with unsupported field shape | `unsupported` | Present | none | none | Diagnostics record unsupported conversion. |
| Malformed source bytes | `rejected` | None | none | none | Diagnostics only; no trusted facts or bytes. |
| Verifier failure after candidate emission | `unsupported` or `rejected` style error report from adapter path | No accepted handoff | none exposed | none accepted | Candidate bytes do not escape as accepted source artifacts. |
| Oracle failure after verifier acceptance | Non-accepted error report | Facts may remain descriptive | none exposed | none accepted | Accepted report constructor rejects missing or unsupported oracle evidence. |

## Verifier and Oracle Evidence

Plan 26-03 established the current verifier/oracle handoff:

- `emit_source_ingress_lmc1_from_vortex_buffer` calls the existing Vortex emission
  helper and then immediately runs `loom_core::artifact_verifier::verify_artifact`.
- Accepted handoff returns `SourceIngressAcceptedArtifact { bytes, report }`
  only when the verifier accepts the emitted `LMC1`.
- Unsupported valid and rejected malformed paths return `Err(SourceIngressReport)`
  and expose no artifact bytes.
- Accepted single-column and table tests decode the verified Loom artifact and
  compare rows to source-native Vortex scan evidence.
- `SourceOracleEvidence` records strategy, accepted status, row count checked,
  null checking, source-native scan usage, and notes.

Oracle evidence is not a decode bypass. It is test/report evidence that the
verified Loom artifact rows match the source-native reference for supported
fixtures.

## Dependency and API Creep Evidence

The current dependency boundary is:

- `loom-source-ingress` has no runtime dependencies.
- `loom-core` does not depend on `loom-source-ingress` or source SDK crates.
- `loom-ffi` does not depend on source SDK crates.
- `loom-vortex-ingress` is the source-specific crate that owns Vortex SDK usage.
- DuckDB extension code and public headers remain free of source-ingress public
  API expansion.

Plan 26-04 adds `scripts/source-ingress-contract-test.sh` to check:

- required Phase 26 docs exist,
- generic and Vortex adapter markers exist,
- focused Plan 26-01 through 26-03 tests pass,
- source SDK names are absent from generic/core/ffi/DuckDB/public surfaces,
- public SQL/API creep markers are absent from checked surfaces,
- forbidden checks avoid matching their own script literals by constructing
  patterns from smaller pieces where needed.

Plan 26-05 owns wiring the script into `scripts/mvp0-verify.sh`.

## Current-Phase Tradeoffs

| Tradeoff | Decision | Reason |
|---|---|---|
| New crate vs `loom-core` | Use `loom-source-ingress`. | Source provenance and adapter reports are upstream admission concepts. Keeping them out of `loom-core` reduces source SDK pressure on the artifact verifier. |
| Vortex as reference adapter vs generic vocabulary | Use Vortex for real evidence, but expose `Source*` types generically. | Vortex proves the mapping while Phase 27 can target the same contract without depending on Vortex-named APIs. |
| Canonical raw/table emission vs structured source semantics | Accept canonical emission only as a verifier-backed bridge. | Canonical rows prove safe Loom artifact creation; they do not claim native representation of arbitrary source encodings or storage semantics. |
| Facts vs proof | Facts are descriptive evidence, not a trust token. | Accepted reports still require emission, verifier acceptance, and oracle evidence. |
| Descriptive lowering metadata vs new native kernels | Lowering disposition stays metadata-only. | Phase 26 must not expand native kernels or host execution. It only describes whether the emitted Loom artifact shape fits current lowering evidence. |
| Source SDK isolation vs adapter boilerplate | Keep SDKs in source-specific crates. | A little adapter mapping prevents generic/core/ffi/DuckDB surfaces from accumulating external source dependencies. |
| Contract gate evidence vs host-engine integration | Add a standalone guard, do not wire host behavior in this plan. | Plan 26-04 proves the contract and creep checks before Plan 26-05 release-gate wiring; host integration remains out of scope. |

## Non-Goals

Phase 26 explicitly does not deliver:

- Lance implementation.
- Parquet implementation.
- Iceberg refs or table binding.
- MCAP, Zarr, or LeRobot implementation.
- Object-store credentials or remote IO policy.
- Public SQL/API expansion.
- Host-engine integration.
- Predicate pushdown.
- Parallel split execution.
- ArrowArrayStream public exposure.
- New native kernels.
- Persistent source identity or archive rewrite semantics.
- Arbitrary Vortex semantic compatibility.

## Phase 27 Handoff

Phase 27 Lance/Parquet adapters should consume the generic contract directly:

- implement adapter-local mapping into `SourceIdentity`, `SourceFacts`, coverage,
  diagnostics, and support status;
- declare oracle strategy up front, likely source-native or Arrow scan depending
  on adapter evidence;
- emit only verifier-accepted `LMC1(LMP1)` or `LMC1(LMT1)`;
- keep unsupported valid sources fact-bearing but byte-free;
- keep malformed sources rejected without trusted facts;
- keep Lance/Parquet SDK dependencies out of `loom-core`, `loom-ffi`,
  `loom-source-ingress`, DuckDB extension code, and public headers;
- state support matrices and fail-closed behavior before any archival
  readability claims.

Phase 27 must not treat canonical raw/table emission as arbitrary Lance/Parquet
semantic compatibility. Canonical emission only proves the emitted Loom artifact
shape and its oracle-backed rows for supported slices.

## Verification Commands

Plan 26-04 verification:

```bash
bash -n scripts/source-ingress-contract-test.sh
bash scripts/source-ingress-contract-test.sh
rg -q "Current-Phase Tradeoffs" .planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-REPORT.md
```

The second command is expected to run the focused Phase 26 contract and adapter
tests that already cover Plans 26-01 through 26-03.
