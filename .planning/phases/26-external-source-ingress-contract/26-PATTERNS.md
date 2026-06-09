# Phase 26: External Source Ingress Contract - Pattern Map

**Mapped:** 2026-06-09
**Scope:** Source-neutral ingress contract patterns only; no implementation changes.
**Files analyzed:** 18 required files plus focused analog searches across `crates/`, `scripts/`, and prior phase artifacts.

## Existing Patterns

### Crate Boundaries

The workspace keeps source SDK dependencies outside the core verifier and FFI crates.

- `Cargo.toml` lines 3-11 lists `loom-core`, `loom-ffi`, `loom-fixtures`, `loom-cli`, `loom-vortex-ingress`, `loom-native-melior`, and `loom-solver-smt` as explicit workspace members.
- `Cargo.toml` lines 13-41 centralizes shared dependency versions and pins Arrow/Vortex versions.
- `crates/loom-core/Cargo.toml` lines 5-16 states `loom-core` is pure Rust, zero FFI, zero Vortex dependencies, and depends only on Arrow/RON/Serde/FSST.
- `crates/loom-ffi/Cargo.toml` lines 13-17 depends on `loom-core`, `loom-native-melior`, and Arrow only.
- `ingress/loom-vortex-ingress/Cargo.toml` lines 7-15 is the current source-specific crate that owns `vortex-file`, `vortex-io`, `vortex-layout`, and `vortex-session`.
- `scripts/mvp0-verify.sh` lines 33-53 and `scripts/check-core-invariants.sh` search results enforce that Vortex/FastLanes do not leak into `loom-core`/`loom-ffi`, and that `vortex-file` is isolated to `loom-vortex-ingress`.

**Pattern to copy:** generic contract types should live where they do not introduce new external source SDK dependencies. If Phase 26 creates a new crate, it should be dependency-light like `loom-solver-smt` rather than source-specific like `loom-vortex-ingress`. If it lives in `loom-core`, it must be pure Loom-owned data and must not add Lance/Parquet/Iceberg/MCAP/Zarr/object-store dependencies.

### Report, Facts, Diagnostic Triad

`ingress/loom-vortex-ingress/src/lib.rs` is the closest source-ingress analog.

Copy the shape, but not the Vortex-specific public names:

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 45-66
pub enum VortexIngressStatus {
    Accepted,
    Unsupported,
    Rejected,
}

impl VortexIngressStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}
```

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 68-116
pub enum VortexIngressDiagnosticCode {
    NotYetInspected,
    OpenFailed,
    UnsupportedLayout,
    UnsupportedDType,
    UnsupportedConversion,
}

pub struct VortexIngressDiagnostic {
    pub code: VortexIngressDiagnosticCode,
    pub path: String,
    pub message: String,
}
```

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 365-405
pub struct VortexIngressReport {
    pub status: VortexIngressStatus,
    pub facts: Option<VortexFileFacts>,
    pub diagnostics: Vec<VortexIngressDiagnostic>,
}

impl VortexIngressReport {
    pub fn accepted(facts: VortexFileFacts) -> Self { ... }
    pub fn unsupported(facts: Option<VortexFileFacts>, ...) -> Self { ... }
    pub fn rejected(...) -> Self { ... }
}
```

Also copy the artifact-verifier report discipline from `crates/loom-core/src/artifact_verifier.rs` lines 42-57, 82-104, and 231-289: accepted reports expose facts, rejected/unsupported reports hide facts.

### Reader Facts and Coverage

`VortexReaderFacts` already contains the contract dimensions Phase 26 needs. The generic contract should preserve these categories with source-neutral names.

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 345-363
pub struct VortexReaderFacts {
    pub source_kind: VortexIngressSourceKind,
    pub vortex_file_version: u16,
    pub row_count: u64,
    pub root_dtype: VortexReaderDTypeFact,
    pub root_layout_encoding: String,
    pub layout_facts: Vec<VortexReaderLayoutFact>,
    pub dtype_facts: Vec<VortexReaderDTypeFact>,
    pub segment_facts: Vec<VortexReaderSegmentFact>,
    pub split_facts: Vec<VortexReaderSplitFact>,
    pub statistics_present: bool,
    pub footer_approx_byte_size: Option<usize>,
    pub support: VortexReaderSupport,
    pub emission_kind: VortexReaderEmissionKind,
    pub coverage: VortexEncodingCoverage,
    pub diagnostics: Vec<VortexReaderDiagnostic>,
}
```

`VortexEncodingCoverage` is the analog for separating source support, emission, and native/lowering disposition:

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 230-246
pub struct VortexEncodingCoverage {
    pub dtype_kind: String,
    pub nullable: Option<bool>,
    pub root_layout_encoding: String,
    pub layout_class: String,
    pub array_encoding: String,
    pub has_splits: bool,
    pub has_statistics: bool,
    pub reader_support: VortexReaderSupport,
    pub emission_kind: VortexReaderEmissionKind,
    pub emission_disposition: VortexEmissionDisposition,
    pub lowering_disposition: VortexLoweringDisposition,
    pub notes: Vec<String>,
}
```

### Emission and Verification

Supported emission flows through `LMC1` wrappers and then through the artifact verifier.

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 953-1004
pub fn emit_supported_lmc1_from_vortex_buffer(
    bytes: &[u8],
) -> Result<Vec<u8>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let mut facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);

    if let Ok(table) = scan_supported_table(&file) {
        facts.supported_loom_payload = true;
        let payload = encode_table_payload(&table).map_err(|err| { ... })?;
        return wrap_table_payload(&payload).map_err(|err| { ... });
    }

    let desc = scan_supported_single_column_layout(&file).map_err(|message| { ... })?;
    facts.supported_loom_payload = true;
    let payload = encode_layout_payload(&desc);
    wrap_layout_payload(&payload).map_err(|err| { ... })
}
```

Artifact targets are defined in:

- `crates/loom-core/src/container_codec.rs` lines 1-16 for `LMC1` wrapping `LMP1`/`LMT1`.
- `crates/loom-core/src/container_codec.rs` lines 449-475 for `wrap_layout_payload` / `wrap_table_payload`.
- `crates/loom-core/src/table_codec.rs` lines 20-71 for `TableDescription`, `TableColumn`, and `encode_table_payload`.
- `crates/loom-core/src/artifact_verifier.rs` lines 344-415 for `verify_artifact`.

Phase 26 should describe emission as a contract obligation. It should not introduce new source-specific byte readers or native kernels.

### Oracle Evidence

Vortex scan helpers are used as oracle evidence, not as the Loom decode path:

```rust
// ingress/loom-vortex-ingress/src/lib.rs lines 1006-1010
/// Scan the supported real Vortex slice through Vortex and return Loom-owned rows.
///
/// This is oracle evidence for tests and diagnostics; it does not expose Vortex
/// types or bypass the emitted `LMC1` verifier/decode path.
pub fn scan_i32_values_from_vortex_buffer(...)
```

`ingress/loom-vortex-ingress/tests/single_column_to_loom.rs` lines 57-65 verifies emitted bytes with `verify_artifact` before decoding. Lines 74-122 compare decoded rows against Vortex oracle rows.

`ingress/loom-vortex-ingress/tests/table_to_loom.rs` lines 88-121 scans the source-native table oracle, and lines 132-156 compare the verified Loom table against the oracle.

## Closest Analogs

| Likely Phase 26 Deliverable | Role | Data Flow | Closest Existing Analog | Match Quality | Copy Pattern |
|---|---|---|---|---|---|
| `ingress/loom-source-ingress` or source-neutral module | model/service crate | request-response/transform | `ingress/loom-vortex-ingress/Cargo.toml` lines 1-19 and `crates/loom-core/Cargo.toml` lines 5-16 | role-match | Isolate source SDK deps in source-specific crates; generic crate stays Loom-owned and dependency-light. |
| `SourceIngressStatus` / support enum | model | classification | `VortexIngressStatus` lines 45-66; `ArtifactVerificationStatus` lines 42-57 | exact | Keep `accepted`, `unsupported`, `rejected` stable strings. |
| `SourceIngressDiagnosticCode` / diagnostic | model | diagnostics | `VortexIngressDiagnosticCode` lines 68-116; `ArtifactVerificationDiagnostic` lines 82-104 | exact | Stable code/path/message fields; source-neutral code families. |
| `SourceFacts` / `SourceReaderFacts` | model | transform | `VortexReaderFacts` lines 345-363 | exact | Facts are Loom-owned strings/enums, not external SDK types. |
| `SourceLayoutFact`, `SourceSegmentFact`, `SourceSplitFact` | model | facts/reporting | `VortexReaderLayoutFact`, `VortexReaderSegmentFact`, `VortexReaderSplitFact` lines 308-343 | exact | Use path/index/range fields and plain strings for summaries. |
| `SourceIngressReport` | model | request-response | `VortexIngressReport` lines 365-405; `ArtifactVerificationReport` lines 231-289 | exact | Accepted has facts; rejected has no facts; unsupported may have facts when input is valid. |
| `SourceEmissionKind` | model | artifact emission | `VortexReaderEmissionKind` lines 171-190 | exact | Preserve `none`, `LMP1`, `LMT1` strings; do not imply native execution. |
| `SourceEmissionDisposition` | model | artifact emission | `VortexEmissionDisposition` lines 192-210; Phase 21 matrix lines 45-54 | exact | Preserve `none`, `canonical-raw`, `canonical-table`, `structured-layout`. |
| `SourceLoweringDisposition` | model | lowering metadata | `VortexLoweringDisposition` lines 212-228; Phase 21 matrix lines 56-63 | exact | Preserve descriptive metadata: interpreter-only, production-lowering-supported, fail-closed/deferred. |
| Vortex-to-generic adapter mapping | service/adapter | transform | `reader_facts_from_file` lines 470-526 and `coverage_from_reader_shape` lines 528-598 | role-match | Map existing Vortex facts into generic facts without changing scanner internals. |
| Contract tests for stable vocabulary | test | request-response | `reader_facts_contract.rs` lines 51-88 | exact | Assert stable `.as_str()` output for every enum. |
| Contract tests for accepted facts | test | transform | `reader_facts_contract.rs` lines 90-143 | exact | Assert facts completeness, support, emission, coverage, layout/segment invariants. |
| Fail-closed unsupported test | test | request-response | `reader_facts_contract.rs` lines 145-173; `single_column_to_loom.rs` lines 141-156; `table_to_loom.rs` lines 159-170 | exact | Valid unsupported sources expose facts but emit no bytes. |
| Fail-closed rejected test | test | request-response | `reader_facts_contract.rs` lines 175-181 | exact | Malformed input returns rejected with no facts. |
| Oracle equivalence test | test | transform/oracle | `single_column_to_loom.rs` lines 67-123; `table_to_loom.rs` lines 123-157 | exact | Source-native oracle compared to verified Loom decode. |
| Phase 26 gate script | script/config | batch | `scripts/complete-vortex-reader-test.sh` lines 1-130 and `scripts/vortex-encoding-coverage-test.sh` lines 1-89 | exact | Check docs, implementation markers, focused tests, artifact verifier handoff, dependency guards. |
| MVP release-gate wiring | script/config | batch | `scripts/mvp0-verify.sh` lines 29-122 | role-match | Add a single Phase 26 gate invocation near prior phase gates only after the gate exists. |
| Final contract/report doc | planning/report | evidence | `18-READER-CONTRACT.md` lines 7-100; `21-COVERAGE-MATRIX.md` lines 3-63 | exact | State scope, pipeline, facts model, support states, dependency boundary, oracle evidence, non-goals. |

## Recommended File/Crate Map

### Preferred Home for Generic Types

Create a small Loom-owned contract surface rather than expanding Vortex-specific public names.

Recommended option:

- `ingress/loom-source-ingress/Cargo.toml`
- `ingress/loom-source-ingress/src/lib.rs`
- Add the crate to root `Cargo.toml` workspace members only if Phase 26 implementation chooses a new crate.

Why: the generic contract is not artifact decoding itself, so keeping it out of the hot `loom-core` decode modules reduces churn. It also avoids making `loom-vortex-ingress` the owner of generic vocabulary.

Acceptable narrower option:

- `crates/loom-core/src/source_ingress.rs`
- `pub mod source_ingress;` from `crates/loom-core/src/lib.rs`

Only choose this if the implementation stays pure data/traits with existing dependencies. Do not add external source crates to `loom-core`.

### Vortex Adapter Proof

Likely files if implementing the adapter during Phase 26:

- `ingress/loom-vortex-ingress/src/lib.rs` or a narrow new module such as `ingress/loom-vortex-ingress/src/source_contract.rs`
- `ingress/loom-vortex-ingress/tests/source_ingress_contract.rs`

Copy from:

- `reader_facts_from_vortex_buffer` in `ingress/loom-vortex-ingress/src/lib.rs` lines 908-921.
- `reader_facts_from_file` in lines 470-526.
- `coverage_from_reader_shape` in lines 528-598.

Keep the adapter as a mapping layer. Avoid rewriting `open_options`, scan helpers, table support, or supported emission unless tests reveal a contract mismatch.

### Phase Report and Gate

Likely planning/report files:

- `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-CONTRACT.md`
- `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-REPORT.md`
- `scripts/source-ingress-contract-test.sh`

Copy doc structure from:

- `.planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md` lines 7-100.
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md` lines 3-63 and 82-113.

Copy script structure from:

- `scripts/complete-vortex-reader-test.sh` lines 1-29 for shell header/color helpers.
- `scripts/complete-vortex-reader-test.sh` lines 33-52 for required artifact checks.
- `scripts/vortex-encoding-coverage-test.sh` lines 55-86 for implementation marker and matrix marker checks.
- `scripts/mvp0-verify.sh` lines 29-53 for dependency-boundary guard style.

## Test Patterns

### Stable Vocabulary Tests

Use a focused contract test like `ingress/loom-vortex-ingress/tests/reader_facts_contract.rs` lines 51-88.

Required generic assertions:

- `SourceIngressStatus::{Accepted, Unsupported, Rejected}.as_str()` returns `accepted`, `unsupported`, `rejected`.
- `SourceEmissionKind::{None, Lmp1, Lmt1}.as_str()` returns `none`, `LMP1`, `LMT1`.
- `SourceEmissionDisposition::{None, CanonicalRaw, CanonicalTable, StructuredLayout}.as_str()` returns the Phase 21 strings.
- `SourceLoweringDisposition` preserves `interpreter-only`, `production-lowering-supported`, and `fail-closed/deferred`.
- Diagnostic codes use source-neutral families: open/read/schema/layout/support/conversion/verification/oracle.

### Accepted Facts Tests

Copy the invariant style from `reader_facts_contract.rs` lines 90-143.

Assert:

- source identity/kind/version/fingerprint is populated,
- row count and schema/dtype facts are populated,
- support is accepted,
- emission kind/disposition matches `LMP1` or `LMT1`,
- lowering disposition is descriptive only,
- layout/segment/split facts are internally consistent where available,
- diagnostics can be empty for clean accepted cases.

### Unsupported and Rejected Tests

Copy the exact fail-closed split:

- Valid unsupported source: `reader_facts_contract.rs` lines 145-173 and `single_column_to_loom.rs` lines 141-156. Facts remain available, emission is `none`, no bytes are emitted.
- Malformed/rejected source: `reader_facts_contract.rs` lines 175-181. Report status is rejected and facts are absent.
- Unsupported table shape: `table_to_loom.rs` lines 159-170. Diagnostics must be non-empty.

### Oracle/Equivalence Tests

Copy the verifier-first pattern:

- `single_column_to_loom.rs` lines 57-65 verifies the `LMC1` container before decoding.
- `single_column_to_loom.rs` lines 74-122 compares decoded arrays with source-native oracle rows.
- `table_to_loom.rs` lines 132-156 verifies `LMT1` and compares each column with source-native oracle output.

Phase 26 should include Vortex as the real adapter proof plus mock/source-neutral fixtures for edge cases that would otherwise make the generic model Vortex-shaped.

## Script/Gate Patterns

Recommended script: `scripts/source-ingress-contract-test.sh`.

Copy these behaviors:

1. Shell strict mode and repo root:

```bash
# scripts/complete-vortex-reader-test.sh lines 1-7
#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"
```

2. Color helpers and `info`/`ok`/`fail` functions from `scripts/complete-vortex-reader-test.sh` lines 9-23.

3. Planning artifact checks from `scripts/complete-vortex-reader-test.sh` lines 33-52, adjusted to Phase 26 docs:

- `26-CONTEXT.md`
- `26-PATTERNS.md`
- `26-SOURCE-INGRESS-CONTRACT.md`
- `26-SOURCE-INGRESS-REPORT.md`
- `26-SUMMARY.md`

4. Implementation marker checks like `scripts/vortex-encoding-coverage-test.sh` lines 55-62:

- generic status/facts/report types exist,
- Vortex adapter mapping exists,
- release report contains Vortex mapping, adapter obligations, non-goals, and Phase 27 handoff.

5. Focused test commands like `scripts/vortex-encoding-coverage-test.sh` lines 64-75:

- `cargo test -p <generic-contract-crate-or-loom-core> --test source_ingress_contract`
- `cargo test -p loom-vortex-ingress --test source_ingress_contract`
- `cargo test -p loom-vortex-ingress --test reader_facts_contract`
- `cargo test -p loom-vortex-ingress --test single_column_to_loom`
- `cargo test -p loom-vortex-ingress --test table_to_loom`
- `cargo test -p loom-core --test artifact_verifier`

6. Dependency-boundary guards from `scripts/mvp0-verify.sh` lines 33-53:

- `cargo tree -p loom-core` must not contain `vortex|fastlanes|lance|parquet|iceberg|mcap|zarr|object_store`.
- `cargo tree -p loom-ffi` must not contain source SDK crates.
- source-specific dependencies stay allowlisted to source-specific crates.

Only wire the new script into `scripts/mvp0-verify.sh` after the script and tests are implemented.

## Naming Rules

- Generic public names must not include `Vortex`.
- Use `Source` or `SourceIngress` prefixes for generic types:
  - `SourceIngressStatus`
  - `SourceIngressReport`
  - `SourceIngressDiagnostic`
  - `SourceIngressDiagnosticCode`
  - `SourceFacts`
  - `SourceSchemaFact` or `SourceDTypeFact`
  - `SourceLayoutFact`
  - `SourceSegmentFact`
  - `SourceSplitFact`
  - `SourceSupport`
  - `SourceEmissionKind`
  - `SourceEmissionDisposition`
  - `SourceLoweringDisposition`
  - `SourceOracleEvidence`
- Preserve stable wire/display strings already accepted by Phase 18/21:
  - `accepted`, `unsupported`, `rejected`
  - `none`, `LMP1`, `LMT1`
  - `canonical-raw`, `canonical-table`, `structured-layout`
  - `interpreter-only`, `production-lowering-supported`, `fail-closed/deferred`
- Facts should use Loom-owned strings/enums and primitive fields. Do not expose external SDK structs, lifetimes, sessions, buffers, file handles, or array types in generic public types.
- Use path fields such as `$`, `$.payload`, `$.schema`, `$.layout`, and `$.oracle` consistently with existing diagnostics.
- Use source-neutral diagnostic code families, then let adapters include source-specific detail in messages.
- Do not name generic concepts after Vortex internals: avoid `VortexReaderFacts`, `vortex_file_version`, `VortexEncodingCoverage`, `VortexReaderEmissionKind`, `VortexLayoutFact` in new generic public APIs.

## Risks

- **Dependency leakage:** Adding Lance/Parquet/Iceberg/MCAP/Zarr/object-store crates to `loom-core`, `loom-ffi`, or DuckDB-facing code would violate the established boundary. Use source-specific crates and dependency guards.
- **Generic contract owned by Vortex crate:** Putting source-neutral public types only in `loom-vortex-ingress` would make Phase 27 copy Vortex vocabulary. Prefer a neutral crate/module and a Vortex mapping adapter.
- **Facts treated as proof:** Existing contracts say facts are handoff evidence, not independent correctness proof. Accepted emission still needs `verify_artifact`.
- **Unsupported partial emission:** Valid unsupported sources may expose facts, but must not emit partial `.loom` bytes. Preserve the tests from `single_column_to_loom.rs` and `table_to_loom.rs`.
- **Rejected facts leakage:** Rejected/malformed inputs should have diagnostics only. Copy `ArtifactVerificationReport` and `VortexIngressReport` behavior.
- **Native-lowering overclaim:** `production-lowering-supported` describes the emitted Loom artifact shape, not native support for every original source encoding. Keep lowering metadata descriptive.
- **Over-broad plugin framework:** Phase 26 context explicitly asks for contract/scaffolding, not a broad reader framework. Add traits/adapters only if they reduce real duplication.
- **Risky files to avoid or touch narrowly:** `Cargo.toml`, `crates/loom-core/Cargo.toml`, `crates/loom-ffi/Cargo.toml`, `crates/loom-core/src/container_codec.rs`, `crates/loom-core/src/layout_codec.rs`, `crates/loom-core/src/table_codec.rs`, and `crates/loom-core/src/artifact_verifier.rs`. These define workspace membership, dependency hygiene, and artifact verification semantics. Prefer additive modules/tests over changing existing verifier/codec behavior.
- **Do not edit planning state in this phase mapping task:** `ROADMAP.md` and `STATE.md` are explicitly out of scope for this pattern map.

## No Analog Found

No existing source-neutral ingress crate exists yet. Use the Vortex ingress crate as the reference adapter and the artifact verifier report model as the generic report discipline.

## Metadata

**Analog search scope:** `Cargo.toml`, `crates/*/Cargo.toml`, `ingress/loom-vortex-ingress/src/lib.rs`, required ingress/core tests, `crates/loom-core/src/*codec.rs`, `crates/loom-core/src/artifact_verifier.rs`, `scripts/*.sh`, Phase 18/21 planning artifacts.
**Primary analogs:** `ingress/loom-vortex-ingress/src/lib.rs`, `ingress/loom-vortex-ingress/tests/reader_facts_contract.rs`, `ingress/loom-vortex-ingress/tests/single_column_to_loom.rs`, `ingress/loom-vortex-ingress/tests/table_to_loom.rs`, `crates/loom-core/src/artifact_verifier.rs`, `scripts/complete-vortex-reader-test.sh`, `scripts/vortex-encoding-coverage-test.sh`.
**Pattern extraction date:** 2026-06-09
