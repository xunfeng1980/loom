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

Pending 42-02.

## Closeout Gate

Pending 42-03.
