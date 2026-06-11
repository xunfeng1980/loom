# Plan 49-03 Summary: Fail-Closed Parse-and-Verify

**Status:** Complete

## Delivered

`verify_l2_core_bytes` in `full_verifier.rs`:
- Decodes from wire bytes, then verifies the AST
- Rejects bad magic, unsupported version, truncated payload, bad discriminants — all with typed `ExplicitFailClosed` diagnostics
- Valid bytes yield identical `VerifiedArtifactFacts` to in-memory AST path
- Acceptance/rejection parity across both paths
- 15 new tests covering all rejection modes and parity invariants

The verified object and distributed object are now byte-identical.

**Key file:** `crates/loom-core/src/full_verifier.rs`
