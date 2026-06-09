use std::path::Path;

use loom_iceberg_binding::IcebergBindingAcceptedArtifact;
use serde::{Deserialize, Serialize};

use crate::query_surface::{
    binding_identity, canonical_query_matrix, BindingIdentityEvidence, DualQuerySurfaceDiagnostic,
    QueryKind,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DuckDbQueryCase {
    pub name: String,
    pub query_kind: QueryKind,
    pub identity: BindingIdentityEvidence,
    pub sql: String,
    pub expected_csv: String,
}

pub fn duckdb_query_cases(
    artifact_path: &Path,
    accepted: &IcebergBindingAcceptedArtifact,
) -> Result<Vec<DuckDbQueryCase>, DualQuerySurfaceDiagnostic> {
    let identity = binding_identity(accepted)?;
    let escaped_path = escape_duckdb_string(&artifact_path.to_string_lossy());
    let mut cases = Vec::new();
    for result in canonical_query_matrix(accepted)? {
        let (name, sql, expected_csv) = match result.kind {
            QueryKind::OrderedRows => (
                "ordered_rows",
                format!("SELECT id FROM loom_scan('{escaped_path}') ORDER BY id"),
                csv_values(&result.values),
            ),
            QueryKind::Projection => (
                "projection_id",
                format!("SELECT id FROM loom_scan('{escaped_path}') ORDER BY id"),
                csv_values(&result.values),
            ),
            QueryKind::PredicateIdGteZero => (
                "predicate_id_gte_zero",
                format!("SELECT id FROM loom_scan('{escaped_path}') WHERE id >= 0 ORDER BY id"),
                csv_values(&result.values),
            ),
            QueryKind::Count => (
                "count",
                format!("SELECT COUNT(*) FROM loom_scan('{escaped_path}')"),
                result.scalar.unwrap_or_default().to_string(),
            ),
            QueryKind::Sum => (
                "sum",
                format!("SELECT SUM(id) FROM loom_scan('{escaped_path}')"),
                result.scalar.unwrap_or_default().to_string(),
            ),
        };
        cases.push(DuckDbQueryCase {
            name: name.to_string(),
            query_kind: result.kind,
            identity: identity.clone(),
            sql,
            expected_csv,
        });
    }
    Ok(cases)
}

fn csv_values(values: &[i64]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_duckdb_string(path: &str) -> String {
    path.replace('\'', "''")
}
