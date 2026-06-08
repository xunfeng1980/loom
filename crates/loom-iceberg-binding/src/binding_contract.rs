//! Loom-owned Iceberg binding contract placeholders.
//!
//! Plan 28-01 Task 2 fills in the report model and constructors.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IcebergBindingStatus {
    Accepted,
    Unsupported,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergTableRefIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingFacts;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingEvidence;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingReport;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IcebergBindingReportError {
    MissingFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IcebergBindingAcceptedArtifact;
