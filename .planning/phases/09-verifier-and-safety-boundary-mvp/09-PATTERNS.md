---
phase: 09-verifier-and-safety-boundary-mvp
status: complete
created: 2026-06-08
---

# Phase 09 Patterns

## Planned File Map

| Planned File | Role | Closest Existing Analog | Pattern To Reuse |
|---|---|---|---|
| `crates/loom-core/src/verifier.rs` | verifier API, report, diagnostics, recursive tree walk | `crates/loom-core/src/l1_model.rs`, `crates/loom-core/src/error.rs` | Pure Rust, no unsafe, typed results, recursive `LayoutNode` match |
| `crates/loom-core/src/lib.rs` | module export | existing `pub mod table_codec;` | short module doc comment plus `pub mod verifier;` |
| `crates/loom-core/src/l1_model.rs` | decode entry verifier routing | `decode_layout_to_array_data` | keep decode API stable; verify before builder/decode where practical |
| `crates/loom-core/src/table_codec.rs` | table verifier integration | `TableDescription::validate`, `decode_table_to_array_data` | reuse existing table validation, add verifier call before column decode |
| `crates/loom-ffi/src/ffi.rs` | FFI ingress validation | `loom_decode_inner` | parse payload, verify, decode, map failures to `DecodeFailed` |
| `crates/loom-cli/src/main.rs` | inspect verifier display | existing `inspect` branching for table/single/descriptor | keep CLI concise; print status before descriptor/tree details |
| `scripts/verifier-negative-test.sh` | CLI-level negative gate if needed | `scripts/duckdb-smoke-test.sh`, `scripts/mvp0-verify.sh` | deterministic shell gate with explicit PASS/FAIL lines |
| `.planning/todos/pending/cr-02-decode-for-non-bitpack-reference.md` | stale todo to close/update | Phase summary/todo pattern | move or update only after implementation evidence exists |

## Existing Patterns To Preserve

- `loom-core` has `#![forbid(unsafe_code)]`; verifier must stay safe Rust.
- Malformed inputs return typed errors; normal malformed input should not rely on panic handling.
- CLI errors are surfaced through `display_decode_error` and `Result<(), String>`.
- Release verification is centralized in `scripts/mvp0-verify.sh`.
- Table payloads compose single-column payloads; avoid changing `LMP1` compatibility.

## Suggested Public Symbols

These are planned symbols and should be listed in plan artifacts:

- `loom_core::verifier::VerificationReport`
- `loom_core::verifier::VerificationDiagnostic`
- `loom_core::verifier::VerificationCode`
- `loom_core::verifier::verify_layout`
- `loom_core::verifier::verify_table`

