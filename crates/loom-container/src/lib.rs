//! `loom-container` — Loom distribution container layer.
//!
//! This crate owns all packaging, codec, and distribution logic.
//! It depends on `loom-ir-core` for the L2Core IR types, codec,
//! verifier, sidecar overlay, and runtime ABI.
//! Plan 52-01: production-core modules are now in `loom-common`; this
//! crate re-exports them for backward compatibility.

#![forbid(unsafe_code)]

// Re-export production-core utility functions from loom-common
pub use loom_common::{arrow_to_l2, l2_to_arrow};

// Re-export the 17 production-core modules from loom-common
pub use loom_common::{
    alp_params, arrow_builder_output, arrow_buffer_lowering, arrow_semantic,
    arrow_semantic_codec, arrow_semantic_verifier, artifact_types, decode_dialect, fsst_params,
    kloom_harness, l1_model, l2_kernel_registry, native_arrow_semantic, native_lowering,
    production_native_lowering, runtime_abi, verify_layout_types,
};

// Legacy container-layer modules still resident here
pub mod layout_codec;
pub mod table_codec;
pub mod container_codec;
pub mod verified_lineage;
pub mod descriptor;
// verifier.rs keeps container-dependent verify_table and verify_container
pub mod verifier;
// artifact_verifier.rs keeps container-dependent verify_artifact (LMC1 path)
pub mod artifact_verifier;
