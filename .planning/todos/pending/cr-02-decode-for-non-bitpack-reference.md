---
id: cr-02-decode-for-non-bitpack-reference
created: 2026-06-07
source: 03-REVIEW.md / 03-VERIFICATION.md
severity: warning
resolves_phase: 4
---

# CR-02: decode_for non-BitPack fallback silently drops the FOR reference

`decode_for` in `crates/loom-core/src/l1_model.rs` (~lines 389–392) delegates a
non-`BitPack` inner to `synthesized_read_loop(inner, builder)` WITHOUT applying the
FOR `reference` scalar — it emits `unpacked[i]` instead of `unpacked[i] + reference`.

**Why deferred:** Unreachable in Phase 3 — `loom-fixtures::vortex_reader` always
constructs a `BitPack` inner for `FrameOfReference` nodes, and the BitPack arm applies
the reference correctly. It becomes a correctness landmine in Phase 4 if FOR-over-non-
BitPack (e.g. FOR-over-Raw or FOR-over-Dict) layouts are constructed.

**How to apply:** When wiring additional L1 encodings in Phase 4, either (a) make the
non-BitPack FOR path apply the reference after the inner decode, or (b) return a typed
`UnimplementedEncoding`/`UnsupportedWidth` error rather than silently producing wrong
data. Add a FOR-over-Raw roundtrip test against the Vortex oracle.
