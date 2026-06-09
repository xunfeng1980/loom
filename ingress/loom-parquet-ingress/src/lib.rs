//! Isolated local-file Parquet ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on
//! `parquet`. It will translate local Parquet file facts and Arrow reader
//! batches into Loom-owned source-ingress reports without exposing
//! Parquet SDK readers, file handles, credentials, object-store state, public
//! SQL routes, DuckDB APIs, or FFI surfaces.

pub mod source_contract;

pub use source_contract::{
    emit_source_ingress_lma1_from_parquet_path, emit_source_ingress_lmc2_from_parquet_path,
    parquet_arrow_oracle_batches_from_path, parquet_source_facts_from_path,
    source_ingress_report_from_parquet_path,
};
