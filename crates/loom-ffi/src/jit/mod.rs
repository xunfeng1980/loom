pub mod backend;
pub mod builder;
pub mod decode_dialect_manifest;
pub mod jit;
pub mod pipeline;
pub mod report;
pub mod toolchain;

// Re-export inner jit module items at the `jit` level
// so that loom_ffi::jit::execute_... works directly.
pub use self::jit::*;