//! Verifier scaffold for Arrow semantic artifacts.

use super::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowSemanticVerificationStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowSemanticDiagnostic {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowSemanticVerificationReport {
    status: ArrowSemanticVerificationStatus,
    diagnostics: Vec<ArrowSemanticDiagnostic>,
}

impl ArrowSemanticVerificationReport {
    pub fn accepted() -> Self {
        Self {
            status: ArrowSemanticVerificationStatus::Accepted,
            diagnostics: Vec::new(),
        }
    }

    pub fn rejected(diagnostics: Vec<ArrowSemanticDiagnostic>) -> Self {
        Self {
            status: ArrowSemanticVerificationStatus::Rejected,
            diagnostics,
        }
    }

    pub fn status(&self) -> ArrowSemanticVerificationStatus {
        self.status
    }

    pub fn is_ok(&self) -> bool {
        self.status == ArrowSemanticVerificationStatus::Accepted
    }

    pub fn diagnostics(&self) -> &[ArrowSemanticDiagnostic] {
        &self.diagnostics
    }
}

pub fn verify_arrow_semantic_batch(batch: &ArrowSemanticBatch) -> ArrowSemanticVerificationReport {
    let mut diagnostics = Vec::new();

    if batch.schema().fields().len() != batch.columns().len() {
        diagnostics.push(ArrowSemanticDiagnostic {
            path: "$.schema.fields".to_string(),
            message: "schema field count does not match column count".to_string(),
        });
    }

    for (idx, column) in batch.columns().iter().enumerate() {
        if column.len() != batch.row_count() {
            diagnostics.push(ArrowSemanticDiagnostic {
                path: format!("$.columns[{idx}].length"),
                message: format!(
                    "column length {} does not match batch row count {}",
                    column.len(),
                    batch.row_count()
                ),
            });
        }

        if let Err(error) = column.validate_full() {
            diagnostics.push(ArrowSemanticDiagnostic {
                path: format!("$.columns[{idx}]"),
                message: error.to_string(),
            });
        }
    }

    if diagnostics.is_empty() {
        ArrowSemanticVerificationReport::accepted()
    } else {
        ArrowSemanticVerificationReport::rejected(diagnostics)
    }
}

pub fn verify_arrow_semantic_payload(
    payload: &ArrowSemanticPayload,
) -> ArrowSemanticVerificationReport {
    let mut diagnostics = Vec::new();
    for (batch_idx, batch) in payload.batches().iter().enumerate() {
        if batch.schema().as_ref() != payload.schema().as_ref() {
            diagnostics.push(ArrowSemanticDiagnostic {
                path: format!("$.batches[{batch_idx}].schema"),
                message: "batch schema does not match payload schema".to_string(),
            });
        }

        let report = verify_arrow_semantic_batch(batch);
        diagnostics.extend(report.diagnostics().iter().cloned().map(|mut diagnostic| {
            diagnostic.path = format!(
                "$.batches[{batch_idx}]{}",
                diagnostic.path.trim_start_matches('$')
            );
            diagnostic
        }));
    }

    if diagnostics.is_empty() {
        ArrowSemanticVerificationReport::accepted()
    } else {
        ArrowSemanticVerificationReport::rejected(diagnostics)
    }
}
