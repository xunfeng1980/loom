# Phase 12 Safety Contract

**Status:** Active contract for Phase 12 execution
**Scope:** Current implemented byte-to-Arrow safety boundary
**Out of scope:** Full Loom verifier reserved for Phase 13

## Implemented Boundary

Phase 12 covers this implemented boundary only:

```text
LMC1/LMP1/LMT1 bytes
  -> container/layout/table parse helpers
  -> verifier reports and typed decode errors
  -> decode helpers
  -> Arrow ArrayData
  -> FFI / CLI / DuckDB surfaces
```

The boundary includes:

- `loom-core` container, layout, table, verifier, interpreter, and L2 kernel surfaces.
- `loom-ffi::loom_decode` error-code and `catch_unwind` boundary.
- `loom inspect` and `loom decode` behavior for raw and container-wrapped payloads.
- DuckDB `loom_scan` ingress for existing generated `LMC1` fixtures.
- Existing release gates plus the Phase 12 safety-proof gate.

The boundary excludes:

- Future Loom distribution IR beyond current `LMC1`/`LMP1`/`LMT1` payloads.
- Future L2 total-function language proof.
- MLIR/native lowering correctness or memory-safety proof.
- Real Vortex file/container ingress proof.
- Content-hash URI lookup, signatures, attestation, remote fetch, encryption, or native fast-path claims.
- Semantic correctness beyond existing Vortex/synthetic oracle tests.

## Safety Claims

Phase 12 claims safety and well-formed output construction for current implemented surfaces:

1. Malformed attacker-controlled bytes fail closed through typed errors or verifier diagnostics.
2. Malformed input does not reach successful Arrow output unless parsing and verification pass.
3. Public core decode surfaces return `Result` failures for malformed payloads rather than panicking.
4. `loom-ffi::loom_decode` converts malformed input to `LoomError::DecodeFailed` and caught panics to `LoomError::Panicked`.
5. `loom-core` contains no unsafe code; unsafe C ABI operations remain isolated in `loom-ffi`.
6. Parser, verifier, interpreter, and kernel loops are bounded by finite payload-derived values or decoded array lengths.

## Stable Contract Surface

Stable enough for Phase 12 evidence:

- `LoomDecodeError` variants and typed fields.
- `VerificationCode` categories returned by `VerificationDiagnostic`.
- Diagnostic paths identifying the failing structural location.
- `VerificationReport::is_ok`, `diagnostics`, and `first_error`.
- `verify_layout`, `verify_table`, and `verify_container`.
- `decode_layout_payload_maybe_container` and `decode_table_payload_maybe_container`.
- FFI error-code categories: `NullPointer`, `DecodeFailed`, and `Panicked`.

Not stable:

- Exact English wording of diagnostic or error messages, except where current shell gates intentionally grep an existing phrase.
- Internal helper function names.
- Future payload, verifier, module, or compiler IR shapes.

## Fail-Closed Contract

For malformed input, a public surface must do one of the following:

- Return `Err(LoomDecodeError::...)`.
- Return a non-empty `VerificationReport`.
- Return a nonzero FFI `LoomError` code.
- Exit CLI/DuckDB execution with an error before exposing successful decoded rows.

It must not:

- Panic on attacker-controlled malformed bytes in `loom-core`.
- Produce successful Arrow output after verifier failure.
- Treat unknown required `LMC1` features as optional.
- Continue past malformed container section offsets or lengths.
- Cross the FFI boundary with an unwind.

## Decode-Before-Arrow Rule

Arrow output is allowed only after this chain succeeds:

1. Container/raw payload classification.
2. Checked payload parsing.
3. Structural verifier pass or documented decode-time authoritative check.
4. Typed builder/kernel materialization.
5. Arrow `ArrayData` export or CLI/DuckDB presentation.

`decode_layout_to_array_data` calls `verify_layout` before materialization. `decode_table_to_array_data` calls `verify_table` before per-column materialization. FFI ingress decodes, verifies, and only then calls Arrow FFI export.

## Loop-Bound and Termination Contract

Phase 12 does not prove arbitrary user-defined programs terminate. It proves current implementation loops are finite because they are bounded by checked, finite values:

| Loop family | Bound source | Evidence path |
|---|---|---|
| `LMC1` section parsing | `section_count` and input length | `container_codec.rs` |
| `LMT1` table parsing | `column_count` and length-prefixed payload bytes | `table_codec.rs` |
| `LMP1` node parsing | recursive payload bytes consumed by checked reader | `layout_codec.rs` |
| table verification | finite `columns.len()` | `verifier.rs` |
| dictionary verification/decode | finite decoded code/value arrays | `verifier.rs`, `l1_model.rs` |
| RLE verification/decode | finite run-end and values arrays | `verifier.rs`, `l1_model.rs` |
| bitpack unpack | finite `count`, `offset`, `bit_width`, and `values_buf.len()` | `l1_model/bitpack.rs` |
| FSST kernel | finite `count`, offsets, validity, and code bytes | `fsst_params.rs`, `l2_kernel_registry.rs` |
| ALP kernel | finite `count`, mantissas, and validity | `alp_params.rs`, `l2_kernel_registry.rs` |

Any future dynamic module language, recursive distribution IR, or native lowering path belongs to Phase 13+.

## Unsafe Boundary

`loom-core` is compiled with `#![forbid(unsafe_code)]`. Any accidental unsafe block in the core decoder is a compile error.

`loom-ffi` owns the C ABI and raw pointer writes. Its contract is:

- Reject null output pointers.
- Reject null non-empty input.
- Reconstruct input slices only after null checks.
- Wrap `loom_decode_inner` in `std::panic::catch_unwind`.
- Write Arrow FFI structs exactly once after successful decode/materialization.

## Proof Artifact Mapping

The safety contract is implemented through:

- `12-PROOF-OBLIGATIONS.md` for claim-to-evidence mapping.
- Focused Rust safety contract tests added in `12-02`.
- `scripts/safety-proof-test.sh` added in `12-03`.
- Final proof narrative in `12-SAFETY-PROOF.md` added in `12-04`.

