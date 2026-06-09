//! Engine-neutral native execution for Arrow semantic artifacts.
//!
//! Phase 35 deliberately keeps this backend out of DuckDB and FFI code. The
//! executor verifies `LMC2(LMA1)` or explicit direct `LMA1` bytes, decodes the
//! Arrow semantic payload, copies supported fixed-width primitive columns
//! through typed Arrow builders, and can compare the result with the decoded
//! reference batch.

use std::sync::Arc;

use arrow_array::{
    types::{Float32Type, Float64Type, Int32Type, Int64Type},
    Array, ArrayRef, BooleanArray, PrimitiveArray, RecordBatch,
};
use arrow_schema::DataType;

use crate::arrow_builder_output::OutputBuilder;
use crate::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, decode_arrow_semantic_payload,
    is_arrow_semantic_container, is_arrow_semantic_payload,
};
use crate::artifact_verifier::{
    verify_artifact, ArtifactVerificationOptions, ArtifactVerificationReport,
    ArtifactVerificationStatus,
};
use crate::l2_kernel_registry::L2KernelRegistry;

pub const NATIVE_ARROW_SEMANTIC_BACKEND: &str = "loom-native-arrow-semantic";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeArrowSemanticDiagnosticCode {
    VerifierRejected,
    UnsupportedArtifact,
    UnsupportedPayload,
    UnsupportedBatchShape,
    UnsupportedType,
    NativeOutputMismatch,
}

impl NativeArrowSemanticDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifierRejected => "verifier-rejected",
            Self::UnsupportedArtifact => "unsupported-artifact",
            Self::UnsupportedPayload => "unsupported-payload",
            Self::UnsupportedBatchShape => "unsupported-batch-shape",
            Self::UnsupportedType => "unsupported-type",
            Self::NativeOutputMismatch => "native-output-mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArrowSemanticDiagnostic {
    pub code: NativeArrowSemanticDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl NativeArrowSemanticDiagnostic {
    fn new(
        code: NativeArrowSemanticDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NativeArrowSemanticExecutionReport {
    pub backend: String,
    pub artifact_kind: String,
    pub payload_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    output: Option<RecordBatch>,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticExecutionReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && self.output.is_some()
    }

    pub fn output(&self) -> Option<&RecordBatch> {
        self.output.as_ref()
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&NativeArrowSemanticDiagnostic> {
        self.diagnostics.first()
    }

    fn rejected(diagnostic: NativeArrowSemanticDiagnostic) -> Self {
        Self {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: String::new(),
            payload_kind: String::new(),
            row_count: 0,
            column_count: 0,
            output: None,
            diagnostics: vec![diagnostic],
        }
    }

}

#[derive(Debug, Clone)]
pub struct NativeArrowSemanticEquivalenceReport {
    pub backend: String,
    pub artifact_kind: String,
    pub row_count: u64,
    pub column_count: usize,
    pub equivalent: bool,
    diagnostics: Vec<NativeArrowSemanticDiagnostic>,
}

impl NativeArrowSemanticEquivalenceReport {
    pub fn is_equivalent(&self) -> bool {
        self.equivalent && self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[NativeArrowSemanticDiagnostic] {
        &self.diagnostics
    }
}

pub fn execute_native_arrow_semantic(
    bytes: &[u8],
) -> NativeArrowSemanticExecutionReport {
    execute_native_arrow_semantic_with_options(bytes, &ArtifactVerificationOptions::default())
}

pub fn execute_native_arrow_semantic_with_options(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> NativeArrowSemanticExecutionReport {
    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(bytes, &registry, options);
    execute_verified_native_arrow_semantic(bytes, &verification)
}

pub fn execute_verified_native_arrow_semantic(
    bytes: &[u8],
    verification: &ArtifactVerificationReport,
) -> NativeArrowSemanticExecutionReport {
    if verification.status() != ArtifactVerificationStatus::Accepted || !verification.is_ok() {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::VerifierRejected,
            "$.verification",
            "native Arrow semantic execution requires an accepted artifact verifier report",
        ));
    }

    let Some(facts) = verification.facts() else {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::VerifierRejected,
            "$.facts",
            "accepted artifact verifier report did not expose facts",
        ));
    };

    if !matches!(facts.artifact_kind.as_str(), "LMC2" | "LMA1") {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedArtifact,
            "$.facts.artifact_kind",
            format!(
                "unsupported artifact kind '{}'; expected LMC2 or LMA1",
                facts.artifact_kind
            ),
        ));
    }

    if facts.payload_kind.as_deref() != Some("Arrow semantic payload") {
        return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedPayload,
            "$.facts.payload_kind",
            "native Arrow semantic execution requires an Arrow semantic payload",
        ));
    }

    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => return NativeArrowSemanticExecutionReport::rejected(diagnostic),
    };

    let mut copied_columns = Vec::with_capacity(reference.num_columns());
    for (idx, column) in reference.columns().iter().enumerate() {
        match copy_supported_column(column.as_ref(), idx) {
            Ok(array) => copied_columns.push(array),
            Err(diagnostic) => return NativeArrowSemanticExecutionReport::rejected(diagnostic),
        }
    }

    let row_count = reference.num_rows() as u64;
    let column_count = reference.num_columns();
    let output = match RecordBatch::try_new(reference.schema(), copied_columns) {
        Ok(batch) => batch,
        Err(_) => {
            return NativeArrowSemanticExecutionReport::rejected(NativeArrowSemanticDiagnostic::new(
                NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
                "$.native.output",
                "native Arrow semantic output batch construction failed",
            ));
        }
    };

    NativeArrowSemanticExecutionReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind: facts.artifact_kind.clone(),
        payload_kind: facts.payload_kind.clone().unwrap_or_default(),
        row_count,
        column_count,
        output: Some(output),
        diagnostics: Vec::new(),
    }
}

pub fn verify_native_arrow_semantic_equivalence(
    bytes: &[u8],
) -> NativeArrowSemanticEquivalenceReport {
    let execution = execute_native_arrow_semantic(bytes);
    verify_native_arrow_semantic_equivalence_from_execution(bytes, &execution)
}

pub fn verify_native_arrow_semantic_equivalence_from_execution(
    bytes: &[u8],
    execution: &NativeArrowSemanticExecutionReport,
) -> NativeArrowSemanticEquivalenceReport {
    if !execution.is_supported() {
        return NativeArrowSemanticEquivalenceReport {
            backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
            artifact_kind: execution.artifact_kind.clone(),
            row_count: execution.row_count,
            column_count: execution.column_count,
            equivalent: false,
            diagnostics: execution.diagnostics.clone(),
        };
    }

    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticEquivalenceReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind: execution.artifact_kind.clone(),
                row_count: execution.row_count,
                column_count: execution.column_count,
                equivalent: false,
                diagnostics: vec![diagnostic],
            };
        }
    };

    let output = execution
        .output()
        .expect("supported execution report must expose output");
    let equivalent = output == &reference;
    let diagnostics = if equivalent {
        Vec::new()
    } else {
        vec![NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.native.output",
            "native Arrow semantic output does not match decoded reference batch",
        )]
    };

    NativeArrowSemanticEquivalenceReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind: execution.artifact_kind.clone(),
        row_count: execution.row_count,
        column_count: execution.column_count,
        equivalent,
        diagnostics,
    }
}

pub fn verify_native_arrow_semantic_output_equivalence(
    bytes: &[u8],
    artifact_kind: impl Into<String>,
    output: &RecordBatch,
) -> NativeArrowSemanticEquivalenceReport {
    verify_native_arrow_semantic_equivalence_for_output(bytes, artifact_kind.into(), output)
}

fn verify_native_arrow_semantic_equivalence_for_output(
    bytes: &[u8],
    artifact_kind: String,
    output: &RecordBatch,
) -> NativeArrowSemanticEquivalenceReport {
    let reference = match decode_reference_batch(bytes) {
        Ok(batch) => batch,
        Err(diagnostic) => {
            return NativeArrowSemanticEquivalenceReport {
                backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
                artifact_kind,
                row_count: output.num_rows() as u64,
                column_count: output.num_columns(),
                equivalent: false,
                diagnostics: vec![diagnostic],
            };
        }
    };

    let equivalent = output == &reference;
    let diagnostics = if equivalent {
        Vec::new()
    } else {
        vec![NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::NativeOutputMismatch,
            "$.native.output",
            "native Arrow semantic output does not match decoded reference batch",
        )]
    };

    NativeArrowSemanticEquivalenceReport {
        backend: NATIVE_ARROW_SEMANTIC_BACKEND.to_string(),
        artifact_kind,
        row_count: output.num_rows() as u64,
        column_count: output.num_columns(),
        equivalent,
        diagnostics,
    }
}

fn decode_reference_batch(bytes: &[u8]) -> Result<RecordBatch, NativeArrowSemanticDiagnostic> {
    let payload = if is_arrow_semantic_container(bytes) {
        decode_arrow_semantic_container_payload(bytes)
    } else if is_arrow_semantic_payload(bytes) {
        decode_arrow_semantic_payload(bytes)
    } else {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedArtifact,
            "$.artifact",
            "native Arrow semantic execution requires LMC2(LMA1) or direct LMA1 bytes",
        ));
    }
    .map_err(|err| {
        NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::VerifierRejected,
            "$.payload",
            err.to_string(),
        )
    })?;

    let batches = payload.to_record_batches().map_err(|err| {
        NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.payload.batches",
            err.to_string(),
        )
    })?;
    if batches.len() != 1 {
        return Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedBatchShape,
            "$.payload.batches",
            format!(
                "native Arrow semantic execution requires exactly one record batch, got {}",
                batches.len()
            ),
        ));
    }
    Ok(batches.into_iter().next().expect("one batch checked"))
}

fn copy_supported_column(
    column: &dyn Array,
    column_index: usize,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic> {
    match column.data_type() {
        DataType::Boolean => copy_boolean_column(column, column_index),
        DataType::Int32 => copy_primitive_column::<Int32Type>(column, column_index),
        DataType::Int64 => copy_primitive_column::<Int64Type>(column, column_index),
        DataType::Float32 => copy_primitive_column::<Float32Type>(column, column_index),
        DataType::Float64 => copy_primitive_column::<Float64Type>(column, column_index),
        other => Err(NativeArrowSemanticDiagnostic::new(
            NativeArrowSemanticDiagnosticCode::UnsupportedType,
            format!("$.schema.fields[{column_index}].type"),
            format!(
                "unsupported native Arrow semantic type {other:?}; expected Boolean, Int32, Int64, Float32, or Float64"
            ),
        )),
    }
}

fn copy_boolean_column(
    column: &dyn Array,
    column_index: usize,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic> {
    let Some(values) = column.as_any().downcast_ref::<BooleanArray>() else {
        return Err(downcast_diagnostic(column, column_index));
    };
    let mut builder = OutputBuilder::new(&DataType::Boolean);
    for row in 0..values.len() {
        if values.is_null(row) {
            builder.append_null();
        } else {
            builder.append_bool(values.value(row));
        }
    }
    Ok(arrow_array::make_array(builder.finish()))
}

fn copy_primitive_column<T>(
    column: &dyn Array,
    column_index: usize,
) -> Result<ArrayRef, NativeArrowSemanticDiagnostic>
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let Some(values) = column.as_any().downcast_ref::<PrimitiveArray<T>>() else {
        return Err(downcast_diagnostic(column, column_index));
    };
    let mut builder = OutputBuilder::new(column.data_type());
    for row in 0..values.len() {
        if values.is_null(row) {
            builder.append_null();
        } else {
            append_primitive_value::<T>(&mut builder, values.value(row));
        }
    }
    Ok(arrow_array::make_array(builder.finish()))
}

fn append_primitive_value<T>(builder: &mut OutputBuilder, value: T::Native)
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    match builder {
        OutputBuilder::Int32(_) => builder.append_i32(value_to_i32::<T>(value)),
        OutputBuilder::Int64(_) => builder.append_i64(value_to_i64::<T>(value)),
        OutputBuilder::Float32(_) => builder.append_f32(value_to_f32::<T>(value)),
        OutputBuilder::Float64(_) => builder.append_f64(value_to_f64::<T>(value)),
        OutputBuilder::Boolean(_) | OutputBuilder::Utf8(_) => {
            panic!("primitive native copy called with non-primitive builder")
        }
    }
}

fn value_to_i32<T>(value: T::Native) -> i32
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<i32>()
        .expect("Int32 builder must receive i32 values")
}

fn value_to_i64<T>(value: T::Native) -> i64
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<i64>()
        .expect("Int64 builder must receive i64 values")
}

fn value_to_f32<T>(value: T::Native) -> f32
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<f32>()
        .expect("Float32 builder must receive f32 values")
}

fn value_to_f64<T>(value: T::Native) -> f64
where
    T: arrow_array::types::ArrowPrimitiveType,
{
    let any = &value as &dyn std::any::Any;
    *any.downcast_ref::<f64>()
        .expect("Float64 builder must receive f64 values")
}

fn downcast_diagnostic(column: &dyn Array, column_index: usize) -> NativeArrowSemanticDiagnostic {
    NativeArrowSemanticDiagnostic::new(
        NativeArrowSemanticDiagnosticCode::UnsupportedType,
        format!("$.columns[{column_index}]"),
        format!(
            "Arrow array data type {:?} did not match expected concrete array",
            column.data_type()
        ),
    )
}

#[allow(dead_code)]
fn _assert_record_batch_is_owned(batch: &RecordBatch) -> Vec<ArrayRef> {
    batch
        .columns()
        .iter()
        .map(|column| Arc::clone(column))
        .collect()
}
