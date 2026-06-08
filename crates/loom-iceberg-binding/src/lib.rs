//! Adapter-local local-file Iceberg table/ref binding proof.
//!
//! This crate owns Phase 28 Iceberg binding vocabulary and keeps it out of
//! `loom-core`, `loom-ffi`, `loom-source-ingress`, DuckDB host code, CLI public
//! routes, and public headers. It does not add public SQL, C ABI, DuckDB,
//! StarRocks, remote catalog, table commit, branch/tag mutation, warehouse, or
//! object-store credential surfaces.
//!
//! The default implementation is a local metadata/sidecar binding proof. It
//! intentionally does not depend on the official `iceberg` SDK because Phase 28
//! keeps SDK and Arrow/Parquet-version churn out of the workspace default graph.

pub mod binding_contract;

pub use binding_contract::{
    bind_iceberg_ref_from_paths, iceberg_binding_facts_from_paths,
    source_ingress_report_from_iceberg_metadata_path, IcebergBindingAcceptedArtifact,
    IcebergBindingEvidence, IcebergBindingFacts, IcebergBindingReport, IcebergBindingReportError,
    IcebergBindingStatus, IcebergTableRefIdentity,
};
