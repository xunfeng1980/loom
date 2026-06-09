# 15-03 Summary

Implemented one supported real `.vortex` to `LMC1` conversion slice.

Changed:

- Added `emit_supported_lmc1_from_vortex_buffer`.
- Added `scan_i32_values_from_vortex_buffer` as Loom-owned Vortex scan oracle evidence.
- Supported only real Vortex files that scan to non-null `Int32` rows.
- Added `ingress/loom-vortex-ingress/tests/real_file_to_loom.rs`.
- Added deterministic fixture emitter `emit_vortex_ingress_fixtures`.
- Generated tiny fixtures under `fixtures/vortex/` and `fixtures/loom/`.

Verification:

- `cargo test -p loom-vortex-ingress real_file_to_loom`
- `cargo run -p loom-vortex-ingress --bin emit_vortex_ingress_fixtures`
- `bash scripts/vortex-ingress-test.sh`

Decision:

- Vortex scan is oracle evidence for the narrow slice; emitted bytes still go through existing `LMC1` verifier/decode.
