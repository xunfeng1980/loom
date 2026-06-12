//! `loom-ffi` — Loom sidecar FFI staticlib.
//!
//! # Structure
//!
//! - `jit/`   — JIT acceleration (melior/LLVM), production runtime
//! - `ffi.rs` — C ABI entry points
//!
//! The interpreter (`loom-interp`) lives in `contrib/interp/` as an offline
//! verification tool. It provides the L1/L2 decoder, kloom harness, and Arrow
//! semantic execution — all used in CI for differential verification, not in
//! the production hot path.
//!
//! # Safety boundary
//!
//! All `unsafe` code lives in `ffi.rs` at the `extern "C"` entry points,
//! wrapped in `std::panic::catch_unwind(AssertUnwindSafe(...))`.

use std::alloc::System;

#[global_allocator]
static GLOBAL: System = System;

pub use loom_interp::l2_to_arrow;
pub use loom_interp::arrow_to_l2;

// --- Internal modules ---
pub mod jit;

// --- Re-export from loom-interp (offline verification infrastructure) ---
pub use loom_interp::alp_params;
pub use loom_interp::arrow_buffer_lowering;
pub use loom_interp::arrow_builder_output;
pub use loom_interp::arrow_semantic;
pub use loom_interp::arrow_semantic_codec;
pub use loom_interp::arrow_semantic_verifier;
pub use loom_interp::artifact_types;
pub use loom_interp::decode_dialect;
pub use loom_interp::fsst_params;
pub use loom_interp::kloom_harness;
pub use loom_interp::l1_model;
pub use loom_interp::l2_kernel_registry;
pub use loom_interp::native_arrow_semantic;
pub use loom_interp::native_lowering;
pub use loom_interp::production_native_lowering;
pub use loom_interp::runtime_abi;
pub use loom_interp::verify_layout_types;

// --- Re-export jit modules at crate root ---
pub use jit::backend;
pub use jit::builder;
pub use jit::decode_dialect_manifest;
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
pub use serde;