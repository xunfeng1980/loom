# Phase 42 Context: Verified + Native Coverage Expansion

## Objective

Widen the verified/native coverage surface after MVP1.5 verified-lineage
closeout. Every additional shape must carry an explicit disposition:

- verified-lineage-backed + native-supported;
- verified-lineage-backed + interpreter-only;
- canonicalized bridge;
- fail-closed/deferred.

The phase must not silently promote source compatibility, native eligibility, or
engine coverage. Phase 43 is the next engine phase; Phase 44 freezes the ABI only
after this matrix is known.

## Current Evidence

- Phase 21 records Vortex reader/encoding coverage with support, emission,
  oracle, and native-lowering dispositions.
- Phase 28 turns Vortex coverage into a semantic compatibility matrix and
  rejects overclaims such as `canonical-raw-overclaim` and
  `native-evidence-missing`.
- Phase 31 emits source-backed `LMC2(LMA1)` Arrow semantic artifacts:
  - Vortex: primitive, UTF-8, and struct/table Arrow materialization.
  - Parquet/Lance: nullable scalar Boolean/Int32/Utf8 plus nested List/Struct.
- Phase 35 supports native execution only for one-batch nullable fixed-width
  primitive Boolean/Int32/Int64/Float32/Float64 Arrow semantic artifacts.
- Phase 41 provides `VerifiedLineageRecord` and the closeout gate. Accepted
  shapes can name verifier, solver, Lean, differential validation, and TCB
  evidence without claiming correctness.

## Phase 42 Shape Targets

### Vortex Wave

- Promote Vortex source semantic coverage from a small focused test into a
  reviewer-visible Phase 42 matrix row set.
- Include primitive, UTF-8, struct/table, nullable/deferred, dictionary/RLE,
  bitpack/FOR, and canonical raw rows using existing Phase 21/28 evidence.
- Native support must be stated against the emitted `LMC2(LMA1)` Arrow semantic
  shape, not the original Vortex encoding.

### Lance/Parquet Wave

- Record nullable scalar, Utf8, Struct, and List source shapes as accepted
  source semantic rows when existing roundtrip/oracle/verifier evidence exists.
- Mark native-supported only for fixed-width primitive subsets that Phase 35/40
  actually validate; Utf8 and nested rows remain interpreter-only.
- Preserve legacy direct `LMA1` bridge rows as regression evidence only; default
  source distribution stays `LMC2(LMA1)`.

## Non-Goals

- No arbitrary Vortex decode.
- No StarRocks/second-engine runtime integration.
- No ABI freeze.
- No verified compilation claim.
- No source-data correctness claim.

## Required Closeout Evidence

- Focused tests for each matrix row family.
- `scripts/verified-lineage-test.sh` remains green.
- A Phase 42 living matrix/report documents every supported/deferred row and
  the evidence behind it.
- A focused Phase 42 gate is wired into the broad verifier before closeout.
