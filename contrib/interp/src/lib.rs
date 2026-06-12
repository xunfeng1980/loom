use arrow_schema::DataType;
use loom_ir_core::l2_core::L2DataType;

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

pub mod alp_params;
pub mod arrow_buffer_lowering;
pub mod arrow_builder_output;
pub mod arrow_semantic;
pub mod arrow_semantic_codec;
pub mod arrow_semantic_verifier;
pub mod artifact_types;
pub mod decode_dialect;
pub mod fsst_params;
pub mod kloom_harness;
pub mod l1_model;
pub mod l2_kernel_registry;
pub mod native_arrow_semantic;
pub mod native_lowering;
pub mod production_native_lowering;
pub mod runtime_abi;
pub mod verify_layout_types;