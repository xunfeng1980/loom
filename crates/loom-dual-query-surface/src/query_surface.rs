use arrow_array::{Array, Int32Array};
use loom_core::container_codec::decode_table_payload_maybe_container;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::table_codec::decode_table_to_array_data;
use loom_iceberg_binding::IcebergBindingAcceptedArtifact;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuerySurfaceStatus {
    Accepted,
    Unsupported,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryKind {
    OrderedRows,
    Projection,
    PredicateIdGteZero,
    Count,
    Sum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnsupportedQueryFeature {
    Join,
    FreeformSql,
    ExternalTableDdl,
    RemoteCatalog,
    Credentials,
    NestedField,
    NullableExpansion,
    DistributedExecution,
    PredicatePushdown,
}

impl UnsupportedQueryFeature {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Join => "join",
            Self::FreeformSql => "freeform-sql",
            Self::ExternalTableDdl => "external-table-ddl",
            Self::RemoteCatalog => "remote-catalog",
            Self::Credentials => "credentials",
            Self::NestedField => "nested-field",
            Self::NullableExpansion => "nullable-expansion",
            Self::DistributedExecution => "distributed-execution",
            Self::PredicatePushdown => "predicate-pushdown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BindingIdentityEvidence {
    pub table_uuid: String,
    pub table_name: String,
    pub schema_id: i32,
    pub snapshot_id: i64,
    pub artifact_sha256: String,
    pub row_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanonicalQueryResult {
    pub kind: QueryKind,
    pub projection: Vec<String>,
    pub values: Vec<i64>,
    pub scalar: Option<i64>,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StarRocksQueryDescriptor {
    pub status: QuerySurfaceStatus,
    pub identity: BindingIdentityEvidence,
    pub query_kind: QueryKind,
    pub projection: Vec<String>,
    pub sql: String,
    pub expected_result_digest: String,
    pub expected_values: Vec<i64>,
    pub expected_scalar: Option<i64>,
    pub diagnostics: Vec<DualQuerySurfaceDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DualQuerySurfaceDiagnostic {
    pub code: String,
    pub message: String,
}

impl DualQuerySurfaceDiagnostic {
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            code: "unsupported-query-surface".to_string(),
            message: message.into(),
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            code: "rejected-query-surface".to_string(),
            message: message.into(),
        }
    }
}

pub fn canonical_query_matrix(
    accepted: &IcebergBindingAcceptedArtifact,
) -> Result<Vec<CanonicalQueryResult>, DualQuerySurfaceDiagnostic> {
    let values = decode_single_i32_id_column(accepted)?;
    let mut ordered = values.clone();
    ordered.sort_unstable();
    let predicate = ordered
        .iter()
        .copied()
        .filter(|value| *value >= 0)
        .collect::<Vec<_>>();
    let sum = values.iter().copied().map(i64::from).sum::<i64>();

    Ok(vec![
        canonical_values(QueryKind::OrderedRows, ordered.clone()),
        canonical_values(QueryKind::Projection, ordered),
        canonical_values(QueryKind::PredicateIdGteZero, predicate),
        canonical_scalar(QueryKind::Count, values.len() as i64),
        canonical_scalar(QueryKind::Sum, sum),
    ])
}

pub fn binding_identity(
    accepted: &IcebergBindingAcceptedArtifact,
) -> Result<BindingIdentityEvidence, DualQuerySurfaceDiagnostic> {
    let facts = accepted.report.facts.as_ref().ok_or_else(|| {
        DualQuerySurfaceDiagnostic::rejected("accepted binding report did not expose facts")
    })?;
    let row_count = decode_single_i32_id_column(accepted)?.len() as u64;
    Ok(BindingIdentityEvidence {
        table_uuid: facts.identity.table_uuid.clone(),
        table_name: facts.identity.table_name.clone(),
        schema_id: facts.identity.schema_id,
        snapshot_id: facts.identity.snapshot_id,
        artifact_sha256: facts.artifact_sha256.clone(),
        row_count,
    })
}

pub fn starrocks_descriptors(
    accepted: &IcebergBindingAcceptedArtifact,
) -> Result<Vec<StarRocksQueryDescriptor>, DualQuerySurfaceDiagnostic> {
    let identity = binding_identity(accepted)?;
    canonical_query_matrix(accepted)?
        .into_iter()
        .map(|result| descriptor_for_result(identity.clone(), result))
        .collect()
}

pub fn validate_starrocks_descriptor(
    accepted: &IcebergBindingAcceptedArtifact,
    descriptor: &StarRocksQueryDescriptor,
) -> Result<(), DualQuerySurfaceDiagnostic> {
    let expected = binding_identity(accepted)?;
    if descriptor.status != QuerySurfaceStatus::Accepted {
        return Err(DualQuerySurfaceDiagnostic::rejected(
            "descriptor is not in accepted state",
        ));
    }
    if descriptor.identity != expected {
        return Err(DualQuerySurfaceDiagnostic::rejected(
            "descriptor identity does not match accepted Phase 29 binding",
        ));
    }
    if descriptor.projection != ["id"] {
        return Err(DualQuerySurfaceDiagnostic::unsupported(
            "only projection id is supported in Phase 29",
        ));
    }
    let expected_result = canonical_query_matrix(accepted)?
        .into_iter()
        .find(|result| result.kind == descriptor.query_kind)
        .ok_or_else(|| {
            DualQuerySurfaceDiagnostic::unsupported(
                "descriptor query kind is outside the Phase 30 query matrix",
            )
        })?;
    if descriptor.expected_result_digest != expected_result.digest
        || descriptor.expected_values != expected_result.values
        || descriptor.expected_scalar != expected_result.scalar
    {
        return Err(DualQuerySurfaceDiagnostic::rejected(
            "descriptor expected result evidence does not match accepted Loom artifact",
        ));
    }
    Ok(())
}

pub fn plan_unsupported_query_feature(
    feature: UnsupportedQueryFeature,
) -> Result<StarRocksQueryDescriptor, DualQuerySurfaceDiagnostic> {
    Err(DualQuerySurfaceDiagnostic::unsupported(format!(
        "unsupported Phase 30 query feature: {}",
        feature.as_str()
    )))
}

fn descriptor_for_result(
    identity: BindingIdentityEvidence,
    result: CanonicalQueryResult,
) -> Result<StarRocksQueryDescriptor, DualQuerySurfaceDiagnostic> {
    let sql = match result.kind {
        QueryKind::OrderedRows | QueryKind::Projection => {
            format!(
                "SELECT id FROM {} ORDER BY id",
                starrocks_table_name(&identity.table_name)?
            )
        }
        QueryKind::PredicateIdGteZero => format!(
            "SELECT id FROM {} WHERE id >= 0 ORDER BY id",
            starrocks_table_name(&identity.table_name)?
        ),
        QueryKind::Count => {
            format!(
                "SELECT COUNT(*) FROM {}",
                starrocks_table_name(&identity.table_name)?
            )
        }
        QueryKind::Sum => format!(
            "SELECT SUM(id) FROM {}",
            starrocks_table_name(&identity.table_name)?
        ),
    };
    Ok(StarRocksQueryDescriptor {
        status: QuerySurfaceStatus::Accepted,
        identity,
        query_kind: result.kind,
        projection: result.projection,
        sql,
        expected_result_digest: result.digest,
        expected_values: result.values,
        expected_scalar: result.scalar,
        diagnostics: Vec::new(),
    })
}

fn starrocks_table_name(name: &str) -> Result<String, DualQuerySurfaceDiagnostic> {
    let mut parts = name.split('.');
    let database = parts
        .next()
        .ok_or_else(|| DualQuerySurfaceDiagnostic::unsupported("table name has no database"))?;
    let table = parts
        .next()
        .ok_or_else(|| DualQuerySurfaceDiagnostic::unsupported("table name has no table"))?;
    if parts.next().is_some() {
        return Err(DualQuerySurfaceDiagnostic::unsupported(
            "only database.table names are supported",
        ));
    }
    Ok(format!("`{database}`.`{table}`"))
}

fn decode_single_i32_id_column(
    accepted: &IcebergBindingAcceptedArtifact,
) -> Result<Vec<i32>, DualQuerySurfaceDiagnostic> {
    let table = decode_table_payload_maybe_container(&accepted.bytes).map_err(|error| {
        DualQuerySurfaceDiagnostic::unsupported(format!(
            "accepted artifact is not a supported LMT1 table: {error}"
        ))
    })?;
    if table.columns.len() != 1 || table.columns[0].name != "id" {
        return Err(DualQuerySurfaceDiagnostic::unsupported(
            "Phase 30 DuckDB evidence supports one Int32 column named id",
        ));
    }
    let registry = L2KernelRegistry::default_for_mvp0();
    let arrays = decode_table_to_array_data(&table, &registry).map_err(|error| {
        DualQuerySurfaceDiagnostic::unsupported(format!(
            "accepted table bytes could not be decoded: {error}"
        ))
    })?;
    let ids = Int32Array::from(arrays[0].clone());
    if ids.null_count() != 0 {
        return Err(DualQuerySurfaceDiagnostic::unsupported(
            "Phase 30 DuckDB evidence supports non-null Int32 id values only",
        ));
    }
    Ok((0..ids.len()).map(|idx| ids.value(idx)).collect())
}

fn canonical_values(kind: QueryKind, values: Vec<i32>) -> CanonicalQueryResult {
    let values = values.into_iter().map(i64::from).collect::<Vec<_>>();
    let digest = stable_digest(&format!("{kind:?}|values|{values:?}"));
    CanonicalQueryResult {
        kind,
        projection: vec!["id".to_string()],
        values,
        scalar: None,
        digest,
    }
}

fn canonical_scalar(kind: QueryKind, scalar: i64) -> CanonicalQueryResult {
    let digest = stable_digest(&format!("{kind:?}|scalar|{scalar}"));
    CanonicalQueryResult {
        kind,
        projection: vec!["id".to_string()],
        values: Vec::new(),
        scalar: Some(scalar),
        digest,
    }
}

fn stable_digest(text: &str) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001B3;
    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("fnv1a64:{hash:016x}")
}
