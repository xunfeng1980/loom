//! `loom-container-legacy` — Legacy Loom container format.
//!
//! This crate owns the legacy container packaging layer: LMC1/LMP1/LMT1
//! codecs, human-readable descriptor, container-specific verifier functions,
//! artifact-verifier dispatch, and verified-lineage records.
//! It depends on `loom-common` for production-core types and `loom-ir-core`
//! for the L2Core IR.
//!
//! Plan 52-02: created from the legacy modules of `crates/loom-container`.

#![forbid(unsafe_code)]

// Re-export types extracted to loom-common (so old `use loom_container::*` paths still work)
pub use loom_common::{
    alp_params, arrow_builder_output, arrow_buffer_lowering, arrow_semantic,
    arrow_semantic_codec, arrow_semantic_verifier, artifact_types, decode_dialect, fsst_params,
    kloom_harness, l1_model, l2_kernel_registry, native_arrow_semantic, native_lowering,
    production_native_lowering, runtime_abi, verify_layout_types,
};

// Legacy container-layer modules that live here
pub mod container_codec;
pub mod layout_codec;
pub mod table_codec;
pub mod descriptor;
// verifier.rs keeps container-dependent verify_table and verify_container
pub mod verifier;
// artifact_verifier.rs keeps container-dependent verify_artifact (LMC1 path)
pub mod artifact_verifier;
pub mod verified_lineage;
