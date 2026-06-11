//! Isolated local-file Lance dataset ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on `lance`.
//! It will translate local Lance dataset facts and Arrow scanner batches
//! into Loom-owned source-ingress reports without exposing Lance SDK handles,
//! readers, credentials, object-store state, public SQL routes, DuckDB APIs, or
//! FFI surfaces.
//!
//! Phase 50.1: Degraded to thin host adapter — mount + extract facts + sidecar stubs.
//! Arrow materialization and LMC2/LMA1 emission removed from public API.

mod source_contract;

pub use source_contract::{
    bind_content_hash_to_lance_data,
    extract_sidecar_bytes_from_lance_path,
    lance_source_facts_from_path,
};
