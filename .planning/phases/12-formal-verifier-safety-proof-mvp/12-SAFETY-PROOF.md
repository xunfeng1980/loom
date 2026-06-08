# Phase 12 Safety Proof MVP

**Status:** Final proof artifact for Phase 12
**Boundary:** Current implemented `LMC1`/`LMP1`/`LMT1` byte-to-Arrow path
**Contract:** `12-SAFETY-CONTRACT.md`
**Obligations:** `12-PROOF-OBLIGATIONS.md`

## Theorem

For the current implemented Loom MVP0 boundary, any byte input accepted through the supported `LMC1`, raw `LMP1`, or raw `LMT1` surfaces either:

1. fails closed through a typed parse/decode error, a verifier diagnostic, a nonzero FFI error code, or a CLI/DuckDB error before exposing successful decoded rows; or
2. reaches Arrow output only after checked parsing and structural verification have succeeded.

Within that boundary, the parser, verifier, interpreter, and implemented L2 kernels terminate because every loop is bounded by finite payload-derived counts, checked byte lengths, decoded array lengths, or fixed section/column lists. `loom-core` contributes no unsafe Rust, and C ABI unsafety plus panic containment remain isolated in `loom-ffi`.

This theorem is a safety and well-formed-output argument for the implemented MVP0/v3 path. It is not a correctness proof that decoded values are semantically true to an external file format.

## Assumptions

- Rust and arrow-rs uphold their safe-code memory-safety contracts.
- `loom-core` is compiled with `#![forbid(unsafe_code)]`.
- Public decode callers use the checked parse/decode helpers and do not bypass verifier entry points.
- The DuckDB extension treats nonzero `loom_decode` results and Rust-side parse/decode failures as scan failures.
- The generated fixture and shell gates are run from the repository root with the checked-in workspace configuration.

## Out Of Scope

The following are out of scope for Phase 12 and do not weaken the theorem above because they are outside the implemented boundary:

- Full Loom distribution IR verification beyond current `LMC1`/`LMP1`/`LMT1`.
- Future L2 total-function language proof, module manifests, resource-bound proof, and dynamic kernel semantics.
- MLIR/native lowering correctness, native fast-path safety, and SIMD codegen proof.
- Real Vortex file/container ingress, remote artifact lookup, content-hash URI, signatures, attestation, encryption, or fetch policy.
- Semantic correctness beyond existing Vortex/synthetic oracle tests.
- Exhaustive malformed table-container rejection through DuckDB SQL; this remains deferred until a table FFI ABI exists. Core/CLI table verification is covered now, and valid mixed-table DuckDB smoke coverage remains gated.

## Proof Structure

The proof is by obligation coverage. Each safety claim maps to source evidence, executable evidence, and a release gate in `12-PROOF-OBLIGATIONS.md`.

### No Unsafe Core

`OBL-12-01` establishes that `loom-core` forbids unsafe code at the crate root. The safety proof gate checks for `#![forbid(unsafe_code)]`, and the workspace test gate compiles this invariant continuously. Unsafe C ABI work is constrained to `loom-ffi`, where pointer checks and Arrow FFI writes are boundary-specific rather than part of core decoding.

### Checked Bytes Before Decode

`OBL-12-02` covers `LMC1` container parsing. Header version, feature flags, section count, section offsets, section lengths, duplicate payload sections, and unknown required features are rejected before wrapped payload decode.

`OBL-12-03` covers raw compatibility payloads. Raw `LMP1` and `LMT1` inputs remain accepted for compatibility, but malformed magic, truncation, or length-prefix failures become typed parse/decode failures rather than panics.

### Verification Before Arrow

`OBL-12-04` covers verifier diagnostics. Failures are typed by diagnostic code and carry paths to the structural location.

`OBL-12-05` covers decode routing. `decode_layout_to_array_data` and `decode_table_to_array_data` invoke verification before materializing Arrow arrays. FFI ingress decodes and verifies before exporting Arrow C Data Interface structs. Therefore successful Arrow output is reachable only after parse and verification success or after a documented decode-time authoritative check.

### Termination

`OBL-12-06` covers termination for the current implementation. Phase 12 does not prove termination of a future user-defined language. It proves the implemented loops are finite:

| Area | Bound |
|---|---|
| `LMC1` container directory | `section_count` plus checked input length |
| `LMP1` layout parser | finite input bytes consumed by checked readers |
| `LMT1` table parser | `column_count` plus length-prefixed column payloads |
| verifier table walk | `table.columns.len()` |
| verifier dictionary scan | decoded `codes` length |
| verifier RLE scan | decoded `run_ends` length |
| raw interpreter | `count * elem_size`, checked against buffer length |
| bitpack interpreter | `count`, `offset`, `bit_width`, and checked packed buffer length |
| dictionary interpreter | decoded code array length |
| RLE interpreter | finite `run_ends` and `values` arrays, with monotonicity checks |
| FSST kernel | finite row count, offsets, validity, symbol, and code byte arrays |
| ALP kernel | finite row count, mantissas, exponent, and validity arrays |

Malformed bounds fail closed through typed parse, verifier, or decode errors; they do not create unbounded interpreter loops.

### L2 And Panic Boundary

`OBL-12-07` covers implemented L2 kernels and panic containment. FSST and ALP params are parsed and checked before materialization. Unknown kernels or malformed params fail through verifier/decode diagnostics. `loom-ffi::loom_decode` wraps the Rust entry in `catch_unwind`, converting caught panics to the stable `Panicked` error code instead of crossing the C ABI with an unwind.

### CLI And DuckDB Surfaces

`OBL-12-08` covers user-facing ingress. `loom inspect` reports verifier pass/fail status and exits unsuccessfully on malformed container or descriptor inputs. DuckDB `loom_scan(...)` remains gated for valid container-wrapped single-column and mixed-table fixtures, and the release gate prevents verifier/container failures from being converted into successful fixture scans.

### Continuous Gate

`OBL-12-09` covers the release mechanism. `scripts/safety-proof-test.sh` checks proof documents, obligation IDs, static unsafe/panic invariants, focused core/FFI safety tests, and existing negative verifier/container gates. `scripts/mvp0-verify.sh` invokes that safety proof gate inside the full MVP0 release gate.

## Obligation Summary

- `OBL-12-01`: Complete. No unsafe core; unsafe/FFI boundary isolated.
- `OBL-12-02`: Complete. Malformed `LMC1` containers fail before wrapped payload decode.
- `OBL-12-03`: Complete. Raw `LMP1`/`LMT1` parse failures fail closed.
- `OBL-12-04`: Complete. Verifier diagnostics are typed and path-addressed.
- `OBL-12-05`: Complete. Decode helpers verify before Arrow output.
- `OBL-12-06`: Complete. Current loops are bounded; future dynamic language proof is Phase 13+.
- `OBL-12-07`: Complete. L2 params fail closed; FFI catches panics.
- `OBL-12-08`: Complete for the implemented surfaces, with malformed table-container SQL rejection deferred until a table FFI ABI exists.
- `OBL-12-09`: Complete. Safety proof gate is part of the release gate.

## Gate Evidence

Phase 12 final verification is:

```bash
cargo test --workspace
bash scripts/safety-proof-test.sh
bash scripts/mvp0-verify.sh
git diff --check
```

`scripts/mvp0-verify.sh` includes the Phase 12 safety proof gate, so the one-command release gate checks the safety proof surface along with workspace tests, dependency hygiene, fixture hygiene, and DuckDB SQL smoke coverage.

## Conclusion

The implemented Loom MVP0/v3 boundary is not a full formal verifier for the future Loom IR. It is now a reviewable safety proof MVP: the scope is explicit, every current safety obligation has source and executable evidence, malformed inputs are gated as fail-closed cases, unsafe core code is forbidden, C ABI panics are contained, and current parser/interpreter/kernel loops have concrete finite bounds.
