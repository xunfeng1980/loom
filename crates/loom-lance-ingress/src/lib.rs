//! Isolated local-file Lance dataset ingress boundary.
//!
//! This crate is the only workspace crate that may depend directly on `lance`.
//! It will translate local Lance dataset facts and Arrow scanner batches
//! into Loom-owned source-ingress reports without exposing Lance SDK handles,
//! readers, credentials, object-store state, public SQL routes, DuckDB APIs, or
//! FFI surfaces.

mod source_contract;

pub use source_contract::{
    emit_source_ingress_lma1_from_lance_path, lance_native_oracle_batches_from_path,
    lance_source_facts_from_path, source_ingress_report_from_lance_path,
};
