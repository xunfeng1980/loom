//! `loom-interp` — Loom offline verification interpreter.
//!
//! This crate is a thin re-export of the interpreter modules in `loom-ffi`.
//! The source code lives in `crates/loom-ffi/src/interp/`; this crate exists
//! so the interpreter can be referenced as a standalone dependency.
//!
//! Architecture:
//!   - `loom-ffi` owns the interpreter source (shared types + execution)
//!   - `loom-interp` re-exports from `loom-ffi` for standalone use
//!   - Production path: `loom-ffi` only (JIT)
//!   - Offline verification: `loom-interp` (or `loom-ffi::interp`)

pub use loom_ffi::interp::alp_params;
pub use loom_ffi::interp::arrow_buffer_lowering;
pub use loom_ffi::interp::arrow_builder_output;
pub use loom_ffi::interp::arrow_semantic;
pub use loom_ffi::interp::arrow_semantic_codec;
pub use loom_ffi::interp::arrow_semantic_verifier;
pub use loom_ffi::interp::artifact_types;
pub use loom_ffi::interp::decode_dialect;
pub use loom_ffi::interp::fsst_params;
pub use loom_ffi::interp::kloom_harness;
pub use loom_ffi::interp::l1_model;
pub use loom_ffi::interp::l2_kernel_registry;
pub use loom_ffi::interp::native_arrow_semantic;
pub use loom_ffi::interp::native_lowering;
pub use loom_ffi::interp::production_native_lowering;
pub use loom_ffi::interp::runtime_abi;
pub use loom_ffi::interp::verify_layout_types;

pub use loom_ffi::l2_to_arrow;
pub use loom_ffi::arrow_to_l2;
