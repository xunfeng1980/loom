//! `loom-core` — pure-Rust decode library.
//!
//! This crate owns all decode logic: the L1 layout interpretation loop, the L2
//! kernel dispatch table, and the Arrow builder output stage. It has **zero FFI**
//! and **zero `vortex-*` dependencies** (D-01, D-02). The FFI surface and the
//! Vortex reference decoder live in separate crates (`loom-ffi`, `loom-fixtures`).

// Safety invariant: all unsafe code is confined to `loom-ffi`. This attribute
// makes the compiler enforce that boundary — any accidental unsafe in loom-core
// is a compile error (D-01).
#![forbid(unsafe_code)]

/// Typed decode errors (UnimplementedEncoding, BufferTooShort, UnsupportedWidth).
pub mod error;

/// Stable binary parameter format for the FSST L2 kernel.
pub mod fsst_params;

/// Stable binary parameter format for the ALP-style float L2 kernel.
pub mod alp_params;

/// Minimal MVP0 layout payload codec used by the FFI boundary.
pub mod layout_codec;

/// MVP0 table-shaped payload codec composed from one-column layout payloads.
pub mod table_codec;

/// Versioned Loom distribution container codec.
pub mod container_codec;

/// First-pass structural verifier for MVP0 layouts and table payloads.
pub mod verifier;

/// Tiny future-language model and proof-obligation IR for the Phase 13 verifier.
pub mod l2_core;

/// Executable verifier MVP for the Phase 13 `L2Core` slice.
pub mod full_verifier;

/// K Framework harness for L2Core program trace extraction (Phase 40+).
pub mod kloom_harness;

/// Verifier-gated native-lowering support checks for the Phase 14 spike.
pub mod native_lowering;

/// Production native-lowering support gate for Phase 20+.
pub mod production_native_lowering;

/// Loom-owned textual `loom.decode` dialect surface for Phase 20+.
pub mod decode_dialect;

/// Primitive Arrow/raw-buffer builder plans for Phase 20+.
pub mod arrow_buffer_lowering;

/// Arrow semantic artifact substrate for Phase 31+ full source compatibility.
pub mod arrow_semantic;

/// Deterministic `LMA1` / `LMC2` Arrow semantic payload markers and codec.
pub mod arrow_semantic_codec;

/// Verifier scaffold for Arrow semantic artifacts.
pub mod arrow_semantic_verifier;

/// Engine-neutral native execution for Arrow semantic artifacts.
pub mod native_arrow_semantic;

/// Artifact-facing verified-lineage records for MVP1.5.
pub mod verified_lineage;

/// Unified artifact-facing verifier report model for Phase 17+.
pub mod artifact_verifier;

/// Solver-neutral obligation and discharge report model for Phase 19+.
pub mod solver;

/// Host-neutral runtime ABI and execution policy model for Phase 22+.
pub mod runtime_abi;

/// Human-readable MVP0 layout descriptor codec.
pub mod descriptor;

/// Arrow builder output stage.
///
/// Owns the typed Arrow builder operations (append_value / append_null) and
/// materialises the final `ArrayData` → `to_ffi` chain. Typed builders only —
/// no raw buffer writes (ARROW-01).
pub mod arrow_builder_output;

/// L1 declarative layout model and synthesized read loop.
///
/// Represents the declarative layout descriptions for the built-in L1 encodings:
/// bit-packing, frame-of-reference (FOR), dictionary, run-length encoding (RLE).
/// The interpreter loop lives here.
pub mod l1_model;

/// L2 kernel registry.
///
/// Dispatches L2 kernel identifiers to total-function kernel implementations.
/// The first (and for MVP0 only) registered kernel is FSST string decompression.
/// Implemented in Phase 4+.
pub mod l2_kernel_registry;
