//! Arrow semantic artifact model for Phase 31.
//!
//! This module is the source-compatibility substrate for `LMA1`/`LMC2`.
//! It intentionally models Arrow `ArrayData` trees instead of adding one
//! Loom-specific layout node per Arrow type.
//!
//! **Phase 50.1: LMA1/LMC2 magic constants are out-of-TCB — retained for
//! backward-compatible decode of existing test fixtures only.**

use std::sync::Arc;

use arrow_array::{make_array, RecordBatch};
use arrow_data::ArrayData;
use arrow_schema::{Schema, SchemaRef};

use loom_ir_core::error::LoomDecodeError;

/// Loom container magic for Arrow semantic artifacts.
pub const LMC2_MAGIC: &[u8; 4] = b"LMC2";

/// Loom Arrow semantic payload magic.
pub const LMA1_MAGIC: &[u8; 4] = b"LMA1";

/// A verifier-visible Arrow record batch payload.
#[derive(Debug, Clone)]
pub struct ArrowSemanticBatch {
    schema: SchemaRef,
    columns: Vec<ArrayData>,
    row_count: usize,
}

impl ArrowSemanticBatch {
    /// Construct a semantic batch after checking field/column count and row
    /// count consistency. Full Arrow tree validation is performed by
    /// `arrow_semantic_verifier`.
    pub fn try_new(schema: SchemaRef, columns: Vec<ArrayData>) -> Result<Self, LoomDecodeError> {
        if schema.fields().len() != columns.len() {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "arrow semantic field/column count mismatch",
            ));
        }

        let row_count = columns.first().map(ArrayData::len).unwrap_or(0);
        if columns.iter().any(|column| column.len() != row_count) {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "arrow semantic column row count mismatch",
            ));
        }

        Ok(Self {
            schema,
            columns,
            row_count,
        })
    }

    pub fn empty(schema: Schema) -> Result<Self, LoomDecodeError> {
        Self::try_new(Arc::new(schema), Vec::new())
    }

    pub fn from_record_batch(batch: &RecordBatch) -> Result<Self, LoomDecodeError> {
        Self::try_new(
            batch.schema(),
            batch
                .columns()
                .iter()
                .map(|column| column.to_data())
                .collect(),
        )
    }

    pub fn to_record_batch(&self) -> Result<RecordBatch, LoomDecodeError> {
        let arrays = self
            .columns
            .iter()
            .cloned()
            .map(make_array)
            .collect::<Vec<_>>();
        RecordBatch::try_new(self.schema.clone(), arrays).map_err(|_| {
            LoomDecodeError::MalformedLayoutPayload("arrow semantic record batch reconstruction")
        })
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn columns(&self) -> &[ArrayData] {
        &self.columns
    }

    pub fn row_count(&self) -> usize {
        self.row_count
    }
}

/// A multi-batch Arrow semantic payload.
#[derive(Debug, Clone)]
pub struct ArrowSemanticPayload {
    schema: SchemaRef,
    batches: Vec<ArrowSemanticBatch>,
}

impl ArrowSemanticPayload {
    pub fn try_new(
        schema: SchemaRef,
        batches: Vec<ArrowSemanticBatch>,
    ) -> Result<Self, LoomDecodeError> {
        if batches
            .iter()
            .any(|batch| batch.schema().as_ref() != schema.as_ref())
        {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "arrow semantic batch schema mismatch",
            ));
        }
        Ok(Self { schema, batches })
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn batches(&self) -> &[ArrowSemanticBatch] {
        &self.batches
    }

    pub fn row_count(&self) -> usize {
        self.batches.iter().map(ArrowSemanticBatch::row_count).sum()
    }

    pub fn from_record_batches(batches: &[RecordBatch]) -> Result<Self, LoomDecodeError> {
        let first = batches
            .first()
            .ok_or(LoomDecodeError::MalformedLayoutPayload(
                "arrow semantic payload has no batches",
            ))?;
        let schema = first.schema();
        let batches = batches
            .iter()
            .map(ArrowSemanticBatch::from_record_batch)
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new(schema, batches)
    }

    pub fn to_record_batches(&self) -> Result<Vec<RecordBatch>, LoomDecodeError> {
        self.batches
            .iter()
            .map(ArrowSemanticBatch::to_record_batch)
            .collect()
    }
}
