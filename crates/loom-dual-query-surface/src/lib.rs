//! Adapter-local Phase 29 query-surface evidence.
//!
//! This crate consumes Phase 29 accepted Iceberg binding evidence, keeps
//! executable DuckDB checks on existing `loom_scan(path)`, and emits offline
//! StarRocks-compatible descriptors. It is not a StarRocks connector, a DuckDB
//! public SQL expansion, or a generic query-engine framework.

pub mod duckdb_evidence;
pub mod fixture_bundle;
pub mod query_surface;

pub use duckdb_evidence::{duckdb_query_cases, DuckDbQueryCase};
pub use fixture_bundle::{
    accepted_fixture_bundle, write_accepted_fixture_bundle, AcceptedFixtureBundle,
};
pub use query_surface::{
    canonical_query_matrix, missing_starrocks_runtime_evidence, plan_unsupported_query_feature,
    starrocks_descriptors, unsupported_starrocks_runtime_evidence, validate_starrocks_descriptor,
    validate_starrocks_runtime_output, BindingIdentityEvidence, CanonicalQueryResult,
    DualQuerySurfaceDiagnostic, QueryKind, QuerySurfaceStatus, StarRocksQueryDescriptor,
    StarRocksRuntimeEvidence, StarRocksRuntimeStatus, UnsupportedQueryFeature,
};
