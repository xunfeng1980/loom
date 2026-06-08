# Phase 12 Proof Obligations

**Status:** Execution matrix
**Boundary:** Current `LMC1`/`LMP1`/`LMT1` byte-to-Arrow implementation
**Contract:** `12-SAFETY-CONTRACT.md`

## Status Legend

- **Existing:** Evidence already existed before Phase 12 execution.
- **Planned:** Evidence will be added by a later Phase 12 plan.
- **Complete:** Evidence is implemented and gated.
- **Deferred:** Deliberately out of Phase 12 and assigned to a future phase.

## Obligation Matrix

| ID | Claim | Boundary | Source evidence | Executable evidence | Gate evidence | Status | Gaps |
|---|---|---|---|---|---|---|---|
| `OBL-12-01` | `loom-core` contains no unsafe code; C ABI unsafety is isolated in `loom-ffi`. | Rust core and FFI boundary | `crates/loom-core/src/lib.rs`, `crates/loom-ffi/src/lib.rs`, `crates/loom-ffi/src/ffi.rs` | `crates/loom-ffi/tests/ffi_contract.rs`; existing FFI tests | Planned static check in `scripts/safety-proof-test.sh`; existing `scripts/check-core-invariants.sh` references `catch_unwind` | Planned | Add dedicated safety-proof static check. |
| `OBL-12-02` | `LMC1` container parsing rejects malformed headers, features, and section directories before wrapped payload decode. | Container bytes -> wrapped payload | `crates/loom-core/src/container_codec.rs`, `crates/loom-core/src/verifier.rs` | `crates/loom-core/tests/safety_contract.rs::obl_12_02_container_malformed_bytes_do_not_panic`; existing `container_codec` tests; existing `scripts/container-negative-test.sh` | Existing `scripts/mvp0-verify.sh`; planned `scripts/safety-proof-test.sh` | Planned | Tie existing gate to this obligation and add proof-gate ID check. |
| `OBL-12-03` | Raw `LMP1`/`LMT1` payloads remain compatible but fail closed on parse errors. | Raw payload parse helpers | `crates/loom-core/src/layout_codec.rs`, `crates/loom-core/src/table_codec.rs`, `crates/loom-core/src/container_codec.rs` | `crates/loom-core/tests/safety_contract.rs::obl_12_03_raw_payload_parse_failures_do_not_panic`; existing codec tests | Planned `scripts/safety-proof-test.sh` | Planned | Add proof-gate coverage in `12-03`. |
| `OBL-12-04` | Verifier diagnostics are typed, path-addressed, and exposed before successful decode output. | Verifier reports and CLI inspect | `crates/loom-core/src/verifier.rs`, `crates/loom-cli/src/main.rs` | `crates/loom-core/tests/safety_contract.rs::obl_12_04_05_06_verifier_failure_blocks_arrow_output`, `obl_12_04_05_table_verifier_failure_blocks_arrow_output`; existing verifier unit tests; existing `scripts/verifier-negative-test.sh` | Existing `scripts/mvp0-verify.sh`; planned `scripts/safety-proof-test.sh` | Planned | Add proof-gate coverage in `12-03`. |
| `OBL-12-05` | Decode helpers call verifier before Arrow output and return typed errors on verifier failure. | Decode helpers -> Arrow `ArrayData` | `crates/loom-core/src/l1_model.rs`, `crates/loom-core/src/table_codec.rs`, `crates/loom-ffi/src/ffi.rs` | `crates/loom-core/tests/safety_contract.rs::obl_12_04_05_06_verifier_failure_blocks_arrow_output`, `obl_12_04_05_table_verifier_failure_blocks_arrow_output`; `crates/loom-ffi/tests/ffi_contract.rs` | Planned `scripts/safety-proof-test.sh` | Planned | Add proof-gate coverage in `12-03`. |
| `OBL-12-06` | Current parser/interpreter/kernel loops terminate because they are bounded by finite payload-derived counts or decoded array lengths. | Parser, verifier, interpreter, kernel loops | `container_codec.rs`, `layout_codec.rs`, `table_codec.rs`, `verifier.rs`, `l1_model.rs`, `l1_model/bitpack.rs`, `fsst_params.rs`, `alp_params.rs`, `l2_kernel_registry.rs` | `crates/loom-core/tests/safety_contract.rs` malformed count/shape tests; existing unit tests | Planned final proof and safety gate | Planned | Final `12-SAFETY-PROOF.md` must include loop-bound table. |
| `OBL-12-07` | L2 kernel params fail closed and kernel panics do not cross the public boundary. | `KernelEscape` -> FSST/ALP kernels | `crates/loom-core/src/l2_kernel_registry.rs`, `crates/loom-core/src/fsst_params.rs`, `crates/loom-core/src/alp_params.rs`, `crates/loom-ffi/src/ffi.rs` | Existing FSST/ALP param tests; `crates/loom-ffi/tests/ffi_contract.rs::ffi_contract_panic_sentinel_returns_panicked` | Planned `scripts/safety-proof-test.sh` | Planned | Add proof-gate coverage in `12-03`. |
| `OBL-12-08` | CLI and DuckDB ingress do not convert verifier/container failures into successful scans. | CLI inspect/decode and DuckDB `loom_scan` | `crates/loom-cli/src/main.rs`, `duckdb-ext/loom_extension.cpp`, negative scripts | Existing `verifier-negative-test.sh`, `container-negative-test.sh`, `duckdb-smoke-test.sh` | Existing `scripts/mvp0-verify.sh`; planned `scripts/safety-proof-test.sh` | Planned | Safety proof gate should run negative scripts; DuckDB remains success smoke for valid fixtures. |
| `OBL-12-09` | Release verification continuously checks docs, obligation IDs, tests, static invariants, and negative gates together. | Release gate | `scripts/mvp0-verify.sh` | Planned `scripts/safety-proof-test.sh` | Planned `scripts/mvp0-verify.sh` invocation | Planned | Implement and wire safety proof gate. |

## Loop-Bound Audit

| Area | Loop or recursion | Finite bound | Failure mode if malformed | Obligation |
|---|---|---|---|---|
| Container codec | Section directory iteration | `section_count`, checked against header/input lengths | `MalformedContainer` | `OBL-12-02`, `OBL-12-06` |
| Layout codec | Recursive node decode | Input bytes consumed by checked reader; unknown tags fail | `MalformedLayoutPayload` | `OBL-12-03`, `OBL-12-06` |
| Table codec | Column loop | `column_count` and length-prefixed payload bytes | `MalformedLayoutPayload` | `OBL-12-03`, `OBL-12-06` |
| Verifier | Table columns | `table.columns.len()` | `VerificationReport` diagnostic | `OBL-12-04`, `OBL-12-06` |
| Verifier | Dictionary code scan | decoded `codes` length | `invalid-dictionary-code` diagnostic | `OBL-12-04`, `OBL-12-06` |
| Verifier | RLE run-end scan | decoded `run_ends` length | `invalid-run-end` diagnostic | `OBL-12-04`, `OBL-12-06` |
| Bitpack | Chunk/lane unpack loops | `count`, `offset`, `t_bits`, and checked buffer length | `BufferTooShort` or verifier diagnostic | `OBL-12-05`, `OBL-12-06` |
| Interpreter | Raw decode rows | `count` and checked `count * elem_size` | `BufferTooShort` | `OBL-12-05`, `OBL-12-06` |
| Interpreter | Dictionary decode rows | decoded `codes.len()` | `InvalidDictionaryCode` | `OBL-12-05`, `OBL-12-06` |
| Interpreter | RLE expansion | `run_ends.len()` and `previous..current` bounded by `count` | typed RLE errors | `OBL-12-05`, `OBL-12-06` |
| FSST params | Symbols, offsets, rows, validity | declared counts validated against expected row count and byte lengths | typed FSST errors | `OBL-12-07`, `OBL-12-06` |
| ALP params | Mantissas, validity, rows | declared counts validated against expected row count | typed ALP errors | `OBL-12-07`, `OBL-12-06` |

## Unsafe-Boundary Audit

| Crate | Unsafe policy | Evidence | Obligation |
|---|---|---|---|
| `loom-core` | Unsafe forbidden | `#![forbid(unsafe_code)]` in `crates/loom-core/src/lib.rs` | `OBL-12-01` |
| `loom-ffi` | Unsafe allowed only for C ABI and Arrow FFI writes | `crates/loom-ffi/src/lib.rs`, `crates/loom-ffi/src/ffi.rs` | `OBL-12-01` |
| DuckDB extension | C++ host boundary, no verifier authority | `duckdb-ext/loom_extension.cpp`; Rust decoder remains authoritative for single-column decode | `OBL-12-08` |

## Planned Evidence Updates

- `12-02` adds `crates/loom-core/tests/safety_contract.rs` and `crates/loom-ffi/tests/ffi_contract.rs`.
- `12-03` adds `scripts/safety-proof-test.sh` and wires it into `scripts/mvp0-verify.sh`.
- `12-04` writes `12-SAFETY-PROOF.md`, updates public docs, and closes `PROOF-01` through `PROOF-05`.

## Deferred To Phase 13+

The full Loom verifier is not part of this matrix. It must later cover:

- Future distribution IR beyond current `LMC1`/`LMP1`/`LMT1`.
- Future L2 total-function language.
- Module/kernel manifest semantics.
- Resource-bound proof for distributable modules.
- Native lowering preconditions and fast-path safety contracts.
