# 15-02 Summary

Implemented real Vortex buffer/path inspection.

Changed:

- `inspect_vortex_buffer` opens in-memory real Vortex files.
- `inspect_vortex_path` opens local real Vortex paths.
- Valid files emit deterministic `VortexFileFacts`.
- Malformed buffers return `INGRESS_OPEN_FAILED` diagnostics and no facts.
- Added `crates/loom-vortex-ingress/tests/ingress_facts.rs`.
- Added `scripts/vortex-ingress-test.sh`.

Verification:

- `cargo test -p loom-vortex-ingress ingress_facts`
- `bash scripts/vortex-ingress-test.sh`

Decision:

- Metadata facts are emitted before broad conversion support. Unsupported files fail closed.
