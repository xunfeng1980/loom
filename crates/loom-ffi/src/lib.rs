//! `loom-ffi` — Loom sidecar FFI staticlib.
//!
//! This crate combines the production-core types and codecs (formerly `loom-common`)
//! with the sidecar extract/verify/routing C ABI (formerly `loom-sidecar-ffi`).
//!
//! # Safety boundary
//!
//! All `unsafe` code lives in `ffi.rs` at the `extern "C"` entry points, wrapped
//! in `std::panic::catch_unwind(AssertUnwindSafe(...))` to prevent panics from
//! unwinding across the C ABI. The `ffi` module uses `#![allow(unsafe_code)]`;
//! all other modules are free of `unsafe` code.

use std::alloc::System;

#[global_allocator]
static GLOBAL: System = System;

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

// --- Production-core modules (from loom-common) ---
pub mod artifact_types;
pub mod verify_layout_types;
pub mod fsst_params;
pub mod alp_params;
pub mod arrow_builder_output;
pub mod arrow_semantic;
pub mod arrow_semantic_codec;
pub mod arrow_semantic_verifier;
pub mod native_lowering;
pub mod production_native_lowering;
pub mod decode_dialect;
pub mod arrow_buffer_lowering;
pub mod runtime_abi;
pub mod native_arrow_semantic;
pub mod l1_model;
pub mod l2_kernel_registry;
pub mod kloom_harness;

// --- FFI surface (from loom-sidecar-ffi) ---
pub mod ffi;

// --- Re-export loom-ir-core modules for convenience ---
pub use loom_ir_core::error;
pub use loom_ir_core::full_verifier;
pub use loom_ir_core::l2_core;
pub use loom_ir_core::l2core_codec;
pub use loom_ir_core::sidecar;
pub use loom_ir_core::sidecar_routing;

// --- Re-export key dependencies ---
pub use arrow;
pub use arrow_array;
pub use arrow_buffer;
pub use arrow_schema;
pub use arrow_data;
pub use arrow_ipc;
pub use ron;
pub use serde;
pub use fnv;

// --- Optional melior/LLVM/JIT backend (feature = "melior") ---
#[cfg(feature = "melior")]
pub mod backend;
#[cfg(feature = "melior")]
pub mod decode_dialect_manifest;
#[cfg(feature = "melior")]
pub mod report;
#[cfg(feature = "melior")]
pub mod toolchain;
#[cfg(feature = "melior")]
pub mod builder;
#[cfg(feature = "melior")]
pub mod jit;
#[cfg(feature = "melior")]
pub mod pipeline;