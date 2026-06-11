//! `loom-ffi` — Loom sidecar FFI staticlib.
//!
//! # Structure
//!
//! - `interp/` — Rust interpreter (ground truth, verified by kloom offline)
//! - `jit/`   — JIT acceleration (melior/LLVM, verified against interpreter online)
//! - `ffi.rs` — C ABI entry points
//!
//! # Safety boundary
//!
//! All `unsafe` code lives in `ffi.rs` at the `extern "C"` entry points,
//! wrapped in `std::panic::catch_unwind(AssertUnwindSafe(...))`.

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

// --- Internal modules ---
pub mod interp;
pub mod jit;

// --- Re-export interp modules at crate root (backward-compatible) ---
pub use interp::alp_params;
pub use interp::arrow_buffer_lowering;
pub use interp::arrow_builder_output;
pub use interp::arrow_semantic;
pub use interp::arrow_semantic_codec;
pub use interp::arrow_semantic_verifier;
pub use interp::artifact_types;
pub use interp::decode_dialect;
pub use interp::fsst_params;
pub use interp::kloom_harness;
pub use interp::l1_model;
pub use interp::l2_kernel_registry;
pub use interp::native_arrow_semantic;
pub use interp::native_lowering;
pub use interp::production_native_lowering;
pub use interp::runtime_abi;
pub use interp::verify_layout_types;

// --- Re-export jit modules at crate root (backward-compatible) ---
pub use jit::backend;
pub use jit::builder;
pub use jit::decode_dialect_manifest;
pub use jit::jit as jit_mod;
pub use jit::pipeline;
pub use jit::report;
pub use jit::toolchain;

// --- C ABI surface ---
pub mod ffi;

// --- Re-export loom-ir-core modules ---
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