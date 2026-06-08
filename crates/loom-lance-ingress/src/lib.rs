//! Isolated local-file Lance dataset ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on `lance`.
//! It will translate local Lance dataset facts and supported primitive batches
//! into Loom-owned source-ingress reports without exposing Lance SDK handles,
//! readers, credentials, object-store state, public SQL routes, DuckDB APIs, or
//! FFI surfaces.

