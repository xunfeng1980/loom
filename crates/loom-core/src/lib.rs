//! `loom-core` — pure-Rust decode library.
//!
//! This crate owns all decode logic: the L1 layout interpretation loop, the L2
//! kernel dispatch table, and the Arrow builder output stage. It has **zero FFI**
//! and **zero `vortex-*` dependencies** (D-01, D-02). The FFI surface and the
//! Vortex reference decoder live in separate crates (`loom-ffi`, `loom-fixtures`).
//!
//! Decode logic and module implementations arrive in Phase 3. This phase only
//! establishes the workspace skeleton and the crate boundary invariants.

// Safety invariant: all unsafe code is confined to `loom-ffi`. This attribute
// makes the compiler enforce that boundary — any accidental unsafe in loom-core
// is a compile error (D-01).
#![forbid(unsafe_code)]

/// Arrow builder output stage.
///
/// Owns the typed Arrow builder operations (append_value / append_null / list /
/// struct) and materialises the final `ArrowArray` + `ArrowSchema` pair handed
/// to the FFI export shim. Implemented in Phase 3.
pub mod arrow_builder_output {}

/// L1 declarative layout model.
///
/// Represents the declarative layout descriptions for the built-in L1 encodings:
/// bit-packing, frame-of-reference (FOR), dictionary, run-length encoding (RLE).
/// The interpreter loop lives here. Implemented in Phase 3.
pub mod l1_model {}

/// L2 kernel registry.
///
/// Dispatches L2 kernel identifiers to total-function kernel implementations.
/// The first (and for MVP0 only) registered kernel is FSST string decompression.
/// Implemented in Phase 3.
pub mod l2_kernel_registry {}
