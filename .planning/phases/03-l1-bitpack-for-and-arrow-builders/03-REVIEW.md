---
phase: 03-l1-bitpack-for-and-arrow-builders
reviewed: 2026-06-07T00:00:00Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - crates/loom-core/src/arrow_builder_output.rs
  - crates/loom-core/src/error.rs
  - crates/loom-core/src/l1_model.rs
  - crates/loom-core/src/l1_model/bitpack.rs
  - crates/loom-core/src/lib.rs
  - crates/loom-fixtures/Cargo.toml
  - crates/loom-fixtures/src/lib.rs
  - crates/loom-fixtures/src/oracle.rs
  - crates/loom-fixtures/src/vortex_reader.rs
  - crates/loom-fixtures/tests/bitpack_roundtrip.rs
  - crates/loom-fixtures/tests/for_roundtrip.rs
  - crates/loom-fixtures/tests/wave0_checks.rs
findings:
  critical: 2
  warning: 6
  info: 4
  total: 12
status: issues_found
---

# Phase 3: Code Review Report

**Reviewed:** 2026-06-07
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

Reviewed the loom-core pure-Rust FastLanes bit-unpack core (`bitpack.rs`,
`l1_model.rs`, `arrow_builder_output.rs`, `error.rs`) plus the loom-fixtures
Vortex bridge/oracle and the roundtrip test suites. The bit-unpack arithmetic in
`bitpack.rs` was traced for out-of-bounds word access and straddle correctness;
the buffer-bounds path (`checked_mul` + length guard) is sound and the straddle
branch correctly guards `load_word(next_word)` behind `remaining > 0`, so the
hot path does not panic on the validated parameters.

The defects below cluster in two areas: (1) the contract that "no malformed
input panics" (T-03-01/T-03-03) is **broken** in `decode_raw` and in
`from_*_array` reader entry points, which use unchecked multiplication and
`expect()`/`panic!()` on attacker- or fixture-controlled values; and (2) a
silent-wrong-result path in `decode_for` where a non-`BitPack` inner node drops
the FOR reference entirely. Several test helpers also contain latent
out-of-bounds bugs that masquerade as passing because the chosen fixtures never
exercise the last FastLanes row.

No `<structural_findings>` block was provided, so this report contains narrative
findings only.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: `decode_raw` uses unchecked multiplication — overflow defeats the bounds check and panics on malformed input

**File:** `crates/loom-core/src/l1_model.rs:266-273`
**Issue:**
```rust
let stride = elem_size as usize;
let needed = count * stride;          // UNCHECKED multiply
if data.len() < needed { return Err(BufferTooShort { .. }); }
for i in 0..count {
    let bytes = &data[i * stride..(i + 1) * stride];   // i*stride also unchecked
```
`count` is an arbitrary `usize` taken from the (fixture/Vortex-derived)
`LayoutNode::Raw`. If `count * stride` overflows `usize`, `needed` wraps to a
small value, the `data.len() < needed` guard passes, and the loop then indexes
`data[i * stride..]` out of the slice — a panic. This violates the explicit
T-03-01 / T-03-03 contract documented in `error.rs:1-7` ("no arm ... may
`panic!()` ... every error path surfaces a typed variant"). Note the sibling
`unpack_all` *does* use `checked_mul` (bitpack.rs:161-167), so this is an
inconsistency, not an intended exemption.
**Fix:**
```rust
let stride = elem_size as usize;
let needed = count.checked_mul(stride).ok_or(LoomDecodeError::BufferTooShort {
    needed: usize::MAX,
    got: data.len(),
})?;
if data.len() < needed {
    return Err(LoomDecodeError::BufferTooShort { needed, got: data.len() });
}
```

### CR-02: `decode_for` silently drops the FOR reference when the inner node is not `BitPack`

**File:** `crates/loom-core/src/l1_model.rs:381-385`
**Issue:**
```rust
_ => {
    // Non-BitPack inner: apply the full loop (supports nested FOR trees).
    // For Phase 3 this path is unreachable in practice; delegate for correctness.
    return synthesized_read_loop(inner, builder);
}
```
This fallback decodes the inner node **without applying `reference`**. Any
`FrameOfReference` whose inner is `Raw`, a nested `FrameOfReference`, or any
non-`BitPack` node will emit `unpacked[i]` instead of
`unpacked[i].wrapping_add(reference)` — a silent, row-for-row-wrong result with
no error returned. The comment claims this is "for correctness," but it is the
opposite: it produces incorrect data rather than failing. The reader
(`from_for_array`) currently always builds a `BitPack` inner, so this is not hit
by today's tests, but it is a latent data-corruption bug the moment a different
inner node is constructed (e.g. Phase 4 dictionary-of-FOR, or a directly
constructed `LayoutNode`). The doc comment on the enum even advertises FOR as
`decoded[i] = unpacked[i].wrapping_add(reference)` (l1_model.rs:93)
unconditionally.
**Fix:** Either reject non-`BitPack` inners explicitly, or apply the reference
to whatever the inner decode produced. The narrowest correct fix:
```rust
_ => {
    return Err(LoomDecodeError::UnimplementedEncoding(
        "FrameOfReference over non-BitPack inner",
    ));
}
```
(Add the variant or reuse an existing one.) Silently delegating must not remain.

## Warnings

### WR-01: Reader entry points panic on malformed Vortex input, bypassing the typed-error / `catch_unwind` contract

**File:** `crates/loom-fixtures/src/vortex_reader.rs:128-152, 191-203`
**Issue:** `from_for_array` calls `.expect("FoR reference must be non-null")`,
`.expect("FoRArray inner must be a BitPackedArray (Phase 3)")`, and
`pvalue_to_i128` ends with `panic!("FoR reference must be an integer PValue
...")`. These convert malformed/unsupported Vortex arrays into hard panics. The
phase's stated design (error.rs:1-7, l1_model.rs:38-43) is that malformed input
yields a typed `LoomDecodeError` so the `loom-ffi` `catch_unwind` boundary is
"never triggered by normal ... input." A non-integer FOR reference or a
non-bitpacked inner is exactly the kind of malformed input that should be a typed
error, not a panic. While `vortex_reader` is the isolation boundary (not
`loom-core` proper), these panics will propagate across FFI in the integration
path.
**Fix:** Change `from_for_array` / `from_bitpacked_array` to return
`Result<LayoutNode, LoomDecodeError>` and map the `None`/downcast-failure/
non-integer cases to typed errors instead of `expect`/`panic!`.

### WR-02: Lossy `as i32` / `as i64` truncation after FOR wrapping-add can silently corrupt values

**File:** `crates/loom-core/src/l1_model.rs:414-419, 430-435` (and bitpack path 338-341, 348-351)
**Issue:** `let result = (*val as i128).wrapping_add(reference) as i32;`. The
`as i32` is a truncating cast. If the decoded logical value does not actually fit
in the target Arrow type (e.g. a malformed layout pairs an `Int32` builder with a
`bit_width`/`reference` combination whose sum exceeds `i32` range), the high bits
are silently discarded and the output disagrees with the source with no error.
For well-formed Vortex input the values fit by construction, so tests pass, but
there is no validation that builder width matches the value domain. This is a
correctness landmine for any layout not produced by the trusted reader.
**Fix:** For the `Int32` path, validate the post-add value fits `i32`
(`i32::try_from(result).map_err(...)`) and return a typed error on overflow, or
document and assert the invariant that the builder width always matches the
declared logical type.

### WR-03: Validity vector length is not validated against `count`; short/extra validity bits are silently tolerated

**File:** `crates/loom-core/src/l1_model.rs:344-356, 425-443`
**Issue:** `if bits.get(i).copied().unwrap_or(false)` treats a missing validity
entry (when `bits.len() < count`) as **null**, silently. Conversely, if
`bits.len() > count`, extra entries are ignored. A length mismatch between the
validity bitmap and `count` is a malformed-input condition that should surface as
an error, not be papered over by `unwrap_or(false)`. `extract_validity`
(vortex_reader.rs:60-77) builds the vector via `.take(len)`, so a `BoolArray`
shorter than `len` yields a too-short vec that this code then silently treats as
all-null past its end.
**Fix:** Before the loop, `if bits.len() != count { return
Err(LoomDecodeError::BufferTooShort { needed: count, got: bits.len() }); }` (or a
dedicated `ValidityLengthMismatch` variant), then index `bits[i]` directly.

### WR-04: `unpack_all` `BufferTooShort` reports `needed: usize::MAX` on overflow, producing a misleading error

**File:** `crates/loom-core/src/l1_model/bitpack.rs:161-167`
**Issue:** When `checked_mul` overflows, the error is constructed with
`needed: usize::MAX`. The Display impl (error.rs:61-66) will then print "need
18446744073709551615 bytes" — a nonsense value that obscures the real cause
(arithmetic overflow from absurd `count`/`offset`). It is correct that it does
not panic, but the diagnostic is misleading.
**Fix:** Add a distinct variant (e.g. `LoomDecodeError::SizeOverflow { count,
offset, bit_width }`) for the overflow case so the message reflects the true
failure mode.

### WR-05: Test helper `encode_test_values` straddle branch can write out of bounds (latent)

**File:** `crates/loom-core/src/l1_model.rs:712-728`
**Issue:** The straddle branch computes `next_byte_off = (next_word * lanes +
found_lane) * byte_size` and calls `set_word_le` on it. As shown by the layout
math, for the **last row** of a block (`found_row == t_bits - 1`) the condition
`next_word > curr_word` is true while `remaining == 0`; `next_word` then equals
`bit_width`, which is one word group past the single-block buffer
(`elems_per_chunk = lanes * bit_width`), so `set_word_le` would index out of
bounds and panic. The current test vectors are tiny (4 elements) and never land
on row 31, so this never fires — but it is a real bug in the test oracle that
would surface the moment a value maps to the last row. Note `encode_for_test` in
bitpack.rs:454-459 guards the hi-write with `if remaining > 0` and so is correct;
this helper does not, and additionally its `hi_mask` (line 716) omits the
`remaining == 0` guard present in the bitpack.rs twin (line 447).
**Fix:** Guard the high-word write with `if remaining > 0 { ... }`, mirroring
`encode_for_test`, and add the `remaining == 0` arm to the `hi_mask` match.

### WR-06: Duplicated bit-pack encode/`set_word_le` helpers across two test modules — divergent and drift-prone

**File:** `crates/loom-core/src/l1_model.rs:692-768` and `crates/loom-core/src/l1_model/bitpack.rs:418-501`
**Issue:** `encode_test_values`/`set_word_le`/`find_packed_position` (l1_model.rs)
and `encode_for_test`/`or_word_le`/`find_pack_position` (bitpack.rs) are
near-identical copies of the same FastLanes pack logic. They have **already
diverged**: bitpack.rs guards the straddle hi-write and the `remaining == 0`
mask; l1_model.rs does not (see WR-05). Maintaining two copies of subtle bit
arithmetic guarantees one will rot. Because these helpers are the
"known-correct" buffers the roundtrip tests compare against, a bug in a helper
silently weakens the test (a wrong encoder + matching wrong decoder still
roundtrips).
**Fix:** Hoist a single `pub(crate)` test encoder into one module (e.g.
`bitpack::tests::encode_for_test`) and have l1_model tests call it, deleting the
l1_model copy.

## Info

### IN-01: Doc comment ranges for `bit_width` are inconsistent with validation

**File:** `crates/loom-core/src/l1_model.rs:78` vs `crates/loom-core/src/l1_model/bitpack.rs:106`
**Issue:** The `BitPack.bit_width` doc says "Bits per packed value (1..=64)" while
the `unpack_all` doc says "1..=t_bits" and the code accepts `bit_width == 0`
(returns all-zero, bitpack.rs:145-147). The "1..=" lower bound is contradicted by
the (correct) zero-width handling. Minor doc drift.
**Fix:** State "0..=t_bits" and document the bit_width==0 all-zero fast path on
the enum field.

### IN-02: `unreachable!()` in `t_bits` match arms relies on an unstated invariant

**File:** `crates/loom-core/src/l1_model.rs:340, 350, 421, 437`
**Issue:** `match t_bits { 32 => ..., 64 => ..., _ => unreachable!() }`. `t_bits`
comes from `builder.t_bits()` which today only returns 32 or 64, so this is
sound, but the `unreachable!()` is a panic-on-violation that depends on a
cross-module invariant. If `OutputBuilder` ever gains an `Int16` variant this
becomes a live panic. Low risk given the closed enum, but worth a comment tying
it to `OutputBuilder`.
**Fix:** Add `// SAFETY: t_bits() only ever returns 32 or 64 (OutputBuilder is a
closed 2-variant enum)` or handle the arm with a typed error.

### IN-03: `nulls().map_or(false, ...)` is the verbose form; minor style

**File:** `crates/loom-fixtures/tests/bitpack_roundtrip.rs:161`, `crates/loom-fixtures/tests/for_roundtrip.rs:175`, `crates/loom-fixtures/tests/wave0_checks.rs:161-163`
**Issue:** `array_data.nulls().map_or(false, |n| n.is_null(i))` repeated across
three test files. `is_some_and` reads clearer. Purely stylistic; no behavioral
issue.
**Fix:** `array_data.nulls().is_some_and(|n| n.is_null(i))`.

### IN-04: Large explanatory comment block left in a finished test

**File:** `crates/loom-core/src/l1_model.rs:584-606`
**Issue:** `bitpack_per_row_validity_routes_nulls` carries ~22 lines of
stream-of-consciousness commentary about approaches considered ("Simpler:
construct a Raw node...", "The easiest approach...") that no longer describes
what the final test does. This is commented-out reasoning, not documentation.
**Fix:** Replace with a 1-2 line description of the actual fixture (2-bit pack of
[1,0,3,2] with validity [t,f,t,f]).

---

_Reviewed: 2026-06-07_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
