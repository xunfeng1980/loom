//! Isolated local-file Parquet ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on
//! `parquet`. It will translate local Parquet file facts and supported
//! primitive batches into Loom-owned source-ingress reports without exposing
//! Parquet SDK readers, file handles, credentials, object-store state, public
//! SQL routes, DuckDB APIs, or FFI surfaces.

pub mod source_contract;

pub use source_contract::{
    parquet_source_facts_from_path, source_ingress_report_from_parquet_path,
};
