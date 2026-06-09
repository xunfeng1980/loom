# Phase 42 Living Coverage Matrix

## Scope

This matrix tracks source/shape coverage after MVP1.5 verified-lineage closeout.
Rows separate:

- original source shape;
- emitted Loom artifact shape;
- source oracle evidence;
- artifact verifier / verified-lineage evidence;
- native execution disposition;
- interpreter/fail-closed fallback disposition.

Native support is assigned only to the emitted verified artifact shape with
native/model or execution-engine evidence. It is not inferred from canonical raw
bridges, DuckDB visibility, or toolchain skips.

## Vortex Rows

| Shape ID | Original Vortex shape | Emitted Loom shape | Disposition | Native class | Evidence |
|---|---|---|---|---|---|
| `vortex-lmc2-fixed-width-primitive` | non-null primitive leaf | `LMC2(LMA1)` Arrow semantic fixed-width primitive | verified-lineage-backed + native-supported | execution-engine validated | Vortex Arrow oracle, artifact verifier, verified-lineage record, native execution/model evidence |
| `vortex-lmc2-utf8` | UTF-8 / VarBin materialized by Vortex Arrow executor | `LMC2(LMA1)` Arrow semantic Utf8 | verified-lineage-backed + interpreter-only | interpreter-only | Vortex Arrow oracle, artifact verifier, native unsupported-shape fail-closed |
| `vortex-lmc2-struct-table` | struct/table materialized by Vortex Arrow executor | `LMC2(LMA1)` Arrow semantic struct/table | verified-lineage-backed + interpreter-only | interpreter-only | Vortex Arrow oracle, artifact verifier, native unsupported-shape fail-closed |
| `vortex-canonical-dictionary-i32` | dictionary primitive | `LMC1(LMP1)` canonical raw | canonicalized bridge | interpreter-only | Vortex scan row oracle, artifact verifier, structured dictionary facts deferred |
| `vortex-canonical-run-end-i32` | run-end/RLE primitive | `LMC1(LMP1)` canonical raw | canonicalized bridge | interpreter-only | Vortex scan row oracle, artifact verifier, structured run-end facts deferred |
| `vortex-canonical-bitpack-i32` | bitpack integer | `LMC1(LMP1)` canonical raw | canonicalized bridge | interpreter-only | Vortex scan row oracle, artifact verifier, structured bitpack native delta deferred |
| `vortex-canonical-for-i32` | frame-of-reference integer | `LMC1(LMP1)` canonical raw | canonicalized bridge | interpreter-only | Vortex scan row oracle, artifact verifier, structured FOR native delta deferred |
| `vortex-nullable-validity-deferred` | nullable primitive validity bitmap | none | fail-closed/deferred | deferred | fact-bearing unsupported row; no emitted artifact bytes |

## Lance/Parquet Rows

| Shape ID | Source | Source schema shape | Emitted Loom shape | Disposition | Native class | Evidence |
|---|---|---|---|---|---|---|
| `parquet-nullable-i32` | Parquet | nullable fixed-width primitive | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + native-supported | native-supported | Parquet Arrow oracle, artifact verifier, verified-lineage record, Phase 35/40 native-model validation |
| `parquet-utf8` | Parquet | nullable Utf8 | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + interpreter-only | interpreter-only | Parquet Arrow oracle, artifact verifier, native unsupported-shape fail-closed |
| `parquet-list-int32` | Parquet | nullable List<Int32> | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + interpreter-only | interpreter-only | Parquet Arrow oracle, artifact verifier, native unsupported-shape fail-closed |
| `parquet-struct` | Parquet | nullable Struct | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + interpreter-only | interpreter-only | Parquet Arrow oracle, artifact verifier, native unsupported-shape fail-closed |
| `lance-nullable-i32` | Lance | nullable fixed-width primitive | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + native-supported | native-supported | Lance scanner oracle, artifact verifier, verified-lineage record, Phase 35/40 native-model validation |
| `lance-utf8` | Lance | nullable Utf8 | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + interpreter-only | interpreter-only | Lance scanner oracle, artifact verifier, native unsupported-shape fail-closed |
| `lance-list-int32` | Lance | nullable List<Int32> | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + interpreter-only | interpreter-only | Lance scanner oracle, artifact verifier, native unsupported-shape fail-closed |
| `lance-struct` | Lance | nullable Struct | `LMC2(LMA1)` semantic Arrow | verified-lineage-backed + interpreter-only | interpreter-only | Lance scanner oracle, artifact verifier, native unsupported-shape fail-closed |

## Closeout Gate

`scripts/verified-native-coverage-expansion-test.sh` validates this matrix by
running:

- Vortex Phase 42 matrix tests;
- Parquet Phase 42 matrix tests;
- Lance Phase 42 matrix tests;
- full Arrow semantic source compatibility;
- verified-lineage closeout.

`scripts/mvp2-verify.sh` is the broad MVP2 entry point and inherits MVP1 before
running the Phase 42 gate.
