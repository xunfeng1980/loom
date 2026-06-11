//! `loom-core` — re-export shim.
//!
//! This crate is a thin wrapper that re-exports `loom-ir-core`,
//! `loom-common`, and `loom-container`. All modules live in those
//! crates. Existing consumers continue to `use loom_core::*` without changes.

#![forbid(unsafe_code)]

// --- IR layer (from loom-ir-core) ---
pub use loom_ir_core::error;
pub use loom_ir_core::l2_core;
pub use loom_ir_core::l2core_codec;
pub use loom_ir_core::full_verifier;
// Sidecar modules (Phase 50 — Plan 50-02):
pub use loom_ir_core::sidecar;
pub use loom_ir_core::sidecar_routing;

// --- Production-core layer (from loom-common) ---
pub use loom_common::alp_params;
pub use loom_common::arrow_builder_output;
pub use loom_common::arrow_buffer_lowering;
pub use loom_common::arrow_semantic;
pub use loom_common::arrow_semantic_codec;
pub use loom_common::arrow_semantic_verifier;
pub use loom_common::artifact_types;
pub use loom_common::decode_dialect;
pub use loom_common::fsst_params;
pub use loom_common::kloom_harness;
pub use loom_common::l1_model;
pub use loom_common::l2_kernel_registry;
pub use loom_common::native_arrow_semantic;
pub use loom_common::native_lowering;
pub use loom_common::production_native_lowering;
pub use loom_common::runtime_abi;
pub use loom_common::verify_layout_types;

// --- Container layer (from loom-container) ---
pub use loom_container::container_codec;
pub use loom_container::descriptor;
pub use loom_container::layout_codec;
pub use loom_container::table_codec;
pub use loom_container::verified_lineage;
// artifact_verifier and verifier modules contain container-dependent
// functions (verify_artifact LMC1 path, verify_container, artifact_verifier
// pipeline) — re-exported from loom-container.
pub use loom_container::artifact_verifier;
pub use loom_container::verifier;

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
