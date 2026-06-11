//! `loom-sidecar-ffi` — Lean sidecar FFI staticlib.
//!
//! This crate is compiled as a `staticlib` (and `rlib` for in-process tests).
//! It exports sidecar extract/verify/routing/free functions through the C ABI,
//! depending only on `loom-ir-core` and `loom-parquet-ingress` — zero transitive
//! dependency on `loom-container` or `loom-core`.
//!
//! # Safety boundary
//!
//! All `unsafe` code lives in `ffi.rs` at the `extern "C"` entry points, wrapped
//! in `std::panic::catch_unwind(AssertUnwindSafe(...))` to prevent panics from
//! unwinding across the C ABI. The rest of the crate (lib.rs, build.rs) contains
//! no `unsafe`.

use std::alloc::System;

/// Global allocator declaration (PITFALLS P5, T-51-05).
///
/// Forces Rust to use the OS system allocator so that heap memory allocated on
/// the Rust side can be safely freed via `loom_sidecar_free_bytes` by the C++
/// consumer.  Without this, cross-allocator `free()` is undefined behavior.
#[global_allocator]
static GLOBAL: System = System;

/// FFI surface — `extern "C"` entry points for sidecar operations.
///
/// See [`ffi`] for the four public entry points:
/// `loom_sidecar_extract`, `loom_sidecar_verify`,
/// `loom_sidecar_route`, `loom_sidecar_free_bytes`.
pub mod ffi;
