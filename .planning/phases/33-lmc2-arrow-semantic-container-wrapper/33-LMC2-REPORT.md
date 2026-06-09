# Phase 33 LMC2 Report

## Scope

Phase 33 implemented the `LMC2` Arrow semantic distribution wrapper around
verifier-backed `LMA1` payloads. The phase closed the ambiguity between direct
Arrow semantic payloads and distribution artifacts: source defaults now emit
`LMC2(LMA1)`, and the artifact verifier recognizes the wrapper before inspecting
the inner Arrow semantic payload.

## Implemented Artifact Grammar

`LMC2` is a semantic-specific container, not a replacement for every Loom
container family. Version 1 carries checked section metadata, required feature
names, and exactly one Arrow semantic payload section. The decoder rejects wrong
magic, unsupported versions, unknown required features, duplicate payload
sections, malformed offsets, trailing bytes, and malformed inner `LMA1` bytes.

The inner `LMA1` payload remains the Arrow semantic carrier. `LMC2` is the
distribution contract around it.

## Verifier Facts

`loom_core::artifact_verifier` routes `LMC2` before legacy container decoding.
Accepted reports expose `artifact: LMC2`, `container_version: 1`, feature names,
`payload: Arrow semantic payload`, schema presence, row-count facts, and lowering
diagnostics. Native lowering remains unsupported for Arrow semantic payloads.

Malformed wrappers fail closed with diagnostics rooted at `$.lmc2`.

## Source-Ingress Cutover

Parquet, Lance, and Vortex source-ingress semantic emission now produces
`LMC2(LMA1)` distribution artifacts by default. The source-emission label reports
`LMC2(LMA1)`, and tests decode through the wrapper before comparing Arrow schema,
values, nulls, and metadata.

Per the current project direction, the old `emit_source_ingress_lma1_*` entry
names continue to emit verifier-accepted direct `LMA1` artifacts. Default
source reports and the new `emit_source_ingress_lmc2_*` entry names emit
`LMC2(LMA1)`, keeping wrapper distribution evidence separate from direct bridge
evidence.

## Compatibility Bridge

Direct `LMA1` remains present as explicit bridge evidence for legacy readability
and bounded source SQL e2e tests. The fixture generators create
`*-duckdb-bridge-lma1.loom` files from the old lma1-named source entry points,
while the default source `*.loom` files are emitted through the lmc2-named entry
points.

That bridge is not the default source distribution artifact and is not a claim
that direct `LMA1` is the product/default source path.

## Focused Gate Evidence

`scripts/lmc2-arrow-semantic-container-test.sh` checks the wrapper codec,
artifact-verifier routing, source-ingress emission cutover, CLI report markers,
and default-vs-bridge fixture visibility. It is wired into
`scripts/mvp0-verify.sh` after `scripts/full-arrow-semantic-compatibility-test.sh`.

## Broad Release-Gate Evidence

`scripts/mvp1-verify.sh` inherits `scripts/mvp0-verify.sh`, so it now runs the
full Arrow semantic compatibility gate, the Phase 33 LMC2 wrapper gate, later
binding/query gates, and then the MVP1 DuckDB source e2e gate.

## Non-Goals

Phase 33 did not broaden DuckDB SQL shape support. DuckDB still consumes explicit
direct-`LMA1` bridge fixtures for the source e2e slice until Phase 34 implements
SQL over default `LMC2(LMA1)` artifacts.

Phase 33 did not add native Arrow semantic execution. Artifact verification and
CLI reports continue to classify Arrow semantic payload native lowering as
unsupported.

Phase 33 did not add live StarRocks runtime integration, remote distribution
trust features, signatures, encryption, or a universal replacement for `LMC1`.

## Risks Carried To Phase 34/35

Phase 34 must teach DuckDB `loom_scan(path)` to recognize `LMC2`, unwrap the
inner `LMA1`, and scan Arrow semantic data in staged layers: multi-column
primitive plus nullable first, then logical types, then nested/list/struct.

Phase 35 must keep native Arrow semantic execution engine-neutral and separate
from DuckDB queryability. Native correctness should come from backend/runtime
evidence, not from wrapper acceptance or interpreter fallback.

## Verification Commands

The Phase 33 closeout verification commands are:

```bash
bash scripts/lmc2-arrow-semantic-container-test.sh
LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp1-verify.sh
rg -q "Implemented Artifact Grammar|Verifier Facts|Source-Ingress Cutover|Compatibility Bridge|Non-Goals|Verification Commands|lmc2-arrow-semantic-container-test" .planning/phases/33-lmc2-arrow-semantic-container-wrapper/33-LMC2-REPORT.md
git diff --check
```
