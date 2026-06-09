# 15-01 Summary

Implemented the Phase 15 ingress contract and crate boundary.

Changed:

- Added `15-INGRESS-CONTRACT.md`.
- Added workspace crate `ingress/loom-vortex-ingress`.
- Added stable report/diagnostic/fact types.
- Updated dependency guards so direct `vortex-file` usage is scoped to the ingress crate.
- Preserved zero Vortex/FastLanes dependency guarantees for `loom-core` and `loom-ffi`.

Verification:

- `cargo test -p loom-vortex-ingress`
- `bash scripts/check-core-invariants.sh`
- `bash scripts/mvp0-verify.sh`

Decision:

- Real Vortex file APIs live outside `loom-core` and `loom-ffi`; all public ingress data is Loom-owned.
