//! DEPRECATED — transitional shim.
//!
//! All modules have moved to `loom-common` (production-core types)
//! and `loom-container-legacy` (legacy container format).
//! Use those crates directly. This crate exists for backward
//! compatibility only.
//!
//! Plan 52-02: replaced with re-exports from loom-container-legacy
//! (which already re-exports all production-core modules from loom-common).

#![forbid(unsafe_code)]

// Re-export everything from the legacy container crate.
// Since loom-container-legacy's lib.rs already re-exports all
// loom-common modules, this covers both common and legacy types.
pub use loom_container_legacy::*;

// Re-export utility functions from loom-common's lib.rs
// (not covered by module re-exports above).
pub use loom_common::{arrow_to_l2, l2_to_arrow};
