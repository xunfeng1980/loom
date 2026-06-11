//! `loom-core` — re-export shim.
//!
//! This crate is a thin wrapper that re-exports `loom-ir-core` and
//! `loom-container`. All modules live in those crates. Existing
//! consumers continue to `use loom_core::*` without changes.

#![forbid(unsafe_code)]

// --- IR layer (from loom-ir-core) ---
pub use loom_ir_core::error;
pub use loom_ir_core::l2_core;
pub use loom_ir_core::l2core_codec;
pub use loom_ir_core::full_verifier;
// Sidecar modules (Phase 50 — Plan 50-02):
pub use loom_ir_core::sidecar;
pub use loom_ir_core::sidecar_routing;

// --- Container layer (from loom-container) ---
pub use loom_container::fsst_params;
pub use loom_container::alp_params;
pub use loom_container::layout_codec;
pub use loom_container::table_codec;
pub use loom_container::container_codec;
pub use loom_container::kloom_harness;
pub use loom_container::native_lowering;
pub use loom_container::production_native_lowering;
pub use loom_container::decode_dialect;
pub use loom_container::arrow_buffer_lowering;
pub use loom_container::arrow_semantic;
pub use loom_container::arrow_semantic_codec;
pub use loom_container::arrow_semantic_verifier;
pub use loom_container::native_arrow_semantic;
pub use loom_container::verified_lineage;
pub use loom_container::artifact_verifier;
pub use loom_container::descriptor;
pub use loom_container::arrow_builder_output;
pub use loom_container::l1_model;
pub use loom_container::l2_kernel_registry;
pub use loom_container::verifier;
pub use loom_container::runtime_abi;

// --- Re-export key dependency crates that downstream code may use ---
pub use arrow;
pub use arrow_array;
pub use arrow_buffer;
pub use arrow_schema;
pub use arrow_data;
pub use arrow_ipc;
pub use fsst;
pub use ron;
pub use serde;
pub use fnv;
