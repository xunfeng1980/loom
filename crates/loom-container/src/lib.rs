//! `loom-container` — Loom distribution container layer.
//!
//! This crate owns all packaging, codec, and distribution logic.
//! It depends on `loom-ir-core` for the L2Core IR types, codec,
//! verifier, sidecar overlay, and runtime ABI.

#![forbid(unsafe_code)]

use arrow_schema::DataType;
use loom_ir_core::l2_core::L2DataType;

/// Convert a container-local [`L2DataType`] to a native Arrow [`DataType`].
pub fn l2_to_arrow(dt: &L2DataType) -> DataType {
    match dt {
        L2DataType::Boolean => DataType::Boolean,
        L2DataType::Int32 => DataType::Int32,
        L2DataType::Int64 => DataType::Int64,
        L2DataType::Float32 => DataType::Float32,
        L2DataType::Float64 => DataType::Float64,
        L2DataType::Utf8 => DataType::Utf8,
    }
}

/// Attempt to convert a native Arrow [`DataType`] to a container-local [`L2DataType`].
/// Returns `None` for unsupported types.
pub fn arrow_to_l2(dt: &DataType) -> Option<L2DataType> {
    match dt {
        DataType::Boolean => Some(L2DataType::Boolean),
        DataType::Int32 => Some(L2DataType::Int32),
        DataType::Int64 => Some(L2DataType::Int64),
        DataType::Float32 => Some(L2DataType::Float32),
        DataType::Float64 => Some(L2DataType::Float64),
        DataType::Utf8 => Some(L2DataType::Utf8),
        _ => None,
    }
}

pub mod fsst_params;
pub mod alp_params;
pub mod layout_codec;
pub mod table_codec;
pub mod container_codec;
pub mod kloom_harness;
pub mod native_lowering;
pub mod production_native_lowering;
pub mod decode_dialect;
pub mod arrow_buffer_lowering;
pub mod arrow_semantic;
pub mod arrow_semantic_codec;
pub mod arrow_semantic_verifier;
pub mod native_arrow_semantic;
pub mod verified_lineage;
pub mod artifact_verifier;
pub mod descriptor;
pub mod arrow_builder_output;
pub mod l1_model;
pub mod l2_kernel_registry;
pub mod verifier;
pub mod runtime_abi;
