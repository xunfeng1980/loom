//! `loom-fixtures` — reference fixture builders and oracle decoder.
//!
//! This is the **only** crate in the Loom workspace permitted to depend on
//! `vortex-*` crates (D-02). Keeping Vortex isolated here preserves the
//! independence of `loom-core`'s decode proof: the thing being verified
//! (`loom-core`) must not share code paths with the oracle that verifies it.
//!
//! **This phase (01-01):** placeholder only. No fixture builders are implemented
//! yet — that work arrives in Phase 3 alongside the L1 decode loop.
//!
//! **Phase 3+:** will add programmatic fixture builders that produce
//! Vortex-encoded `ArrayRef` values (BitPacked, FoR, Dict, RLE, FSST) for use
//! by the Phase 5 verification harness.
