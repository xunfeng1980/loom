//! Isolated local-file Parquet ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on
//! `parquet`. It will translate local Parquet file facts and supported
//! primitive batches into Loom-owned source-ingress reports without exposing
//! Parquet SDK readers, file handles, credentials, object-store state, public
//! SQL routes, DuckDB APIs, or FFI surfaces.

