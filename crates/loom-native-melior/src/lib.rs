//! Optional verifier-gated `melior`/LLVM/JIT backend boundary.
//!
//! This crate is intentionally separate from `loom-core` and `loom-ffi` so the
//! default Loom workspace can build and verify without a mandatory MLIR/LLVM
//! installation. Feature-enabled backend evidence must stay fail-closed.

pub mod report;
pub mod toolchain;
