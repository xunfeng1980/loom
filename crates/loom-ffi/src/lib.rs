//! `loom-ffi` — FFI shim and global allocator.
//!
//! This crate is compiled as a `staticlib` (and `rlib` for in-process tests).
//! It carries two categories of content:
//!
//! 1. **Global allocator** (this plan): installs the system allocator so that
//!    Rust heap memory and DuckDB's C++ heap share the same underlying allocator.
//!    Without this, cross-allocator `free()` calls produce undefined behaviour
//!    (PITFALLS P5, T-01-01, CORE-02).
//!
//! 2. **`extern "C"` surface** (Plan 02): `loom_decode` and supporting functions
//!    that form the ABI boundary between the Rust decoder and the C++ DuckDB
//!    extension. cbindgen will generate `loom.h` from those symbols in Plan 02.
//!
//! All `unsafe` code in the workspace lives here; `loom-core` carries
//! `#![forbid(unsafe_code)]` to enforce that boundary.

use std::alloc::System;

/// Global allocator declaration (CORE-02, PITFALLS P5, T-01-01).
///
/// Forces Rust to use the OS system allocator for all heap allocations.
/// DuckDB's C++ runtime also uses the system allocator, so any pointer that
/// crosses the Rust↔C++ boundary can be safely freed on either side without
/// allocator mismatch.
///
/// This declaration must exist before the first `extern "C"` function is added
/// (Plan 02) so the invariant is present from the first moment the staticlib is
/// linked into a C++ consumer.
#[global_allocator]
static GLOBAL: System = System;

/// FFI surface — `extern "C"` entry point and supporting types.
///
/// See [`ffi::loom_decode`] for the locked FFI contract.
pub mod ffi;

/// Internal DuckDB runtime planning bridge over the host-neutral runtime ABI.
pub mod duckdb_runtime;

/// Re-export the primary FFI entry point at the crate root for discoverability.
pub use ffi::loom_decode;
