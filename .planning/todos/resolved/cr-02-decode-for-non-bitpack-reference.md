---
id: cr-02-decode-for-non-bitpack-reference
created: 2026-06-07
resolved: 2026-06-08
source: 03-REVIEW.md / 03-VERIFICATION.md
severity: warning
resolves_phase: 4
resolved_phase: 9
---

# CR-02: decode_for non-BitPack fallback silently drops the FOR reference

`decode_for` previously delegated a non-`BitPack` inner to the child decode path
without applying the FOR `reference` scalar.

## Resolution

Resolved before/through Phase 9 evidence. The current `decode_for` non-BitPack
path decodes the child into `ArrayData`, materializes it as a typed
`DecodedArray`, and calls `append_value_plus_reference` for each row while
preserving nulls.

Evidence:

```bash
cargo test -p loom-core for_over_raw_applies_reference_and_preserves_nulls
```

The regression test constructs `FrameOfReference` over a non-BitPack
`Dictionary` child backed by `Raw` values, applies reference `10`, preserves the
null row, and asserts decoded values `[11, NULL, 11]`.
