//! Isolated local-file Parquet ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on
//! `parquet`. It will translate local Parquet file facts and Arrow reader
//! batches into Loom-owned source-ingress reports without exposing
//! Parquet SDK readers, file handles, credentials, object-store state, public
//! SQL routes, DuckDB APIs, or FFI surfaces.
//!
//! Phase 50.1: Degraded to thin host adapter — mount + extract facts + sidecar stubs.
//! Arrow materialization and LMC2/LMA1 emission removed from public API.

pub mod decode_ir_gen;
pub mod source_contract;
pub mod sidecar_parquet;

pub use decode_ir_gen::generate_decode_ir_from_parquet;
pub use source_contract::{
    bind_content_hash_to_parquet_data,
    extract_sidecar_bytes_from_parquet_path,
    parquet_source_facts_from_path,
};
