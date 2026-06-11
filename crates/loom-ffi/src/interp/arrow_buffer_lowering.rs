//! Primitive Arrow/raw-buffer builder plans for production native lowering.
//!
//! This module models fixed-size primitive output buffers only. It is
//! engine-independent and deliberately does not define the host runtime ABI.

use std::fmt;

use arrow_schema::DataType;

use super::decode_dialect::arrow_type_name;
use super::production_native_lowering::{ProductionColumnShape, ProductionLoweringFacts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveArrowType {
    Int32,
    Int64,
    Float32,
    Float64,
}

impl PrimitiveArrowType {
    pub fn from_data_type(data_type: &DataType) -> Option<Self> {
        match data_type {
            DataType::Int32 => Some(Self::Int32),
            DataType::Int64 => Some(Self::Int64),
            DataType::Float32 => Some(Self::Float32),
            DataType::Float64 => Some(Self::Float64),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
        }
    }

    pub fn byte_width(self) -> u64 {
        match self {
            Self::Int32 | Self::Float32 => 4,
            Self::Int64 | Self::Float64 => 8,
        }
    }

    pub fn mlir_type(self) -> &'static str {
        match self {
            Self::Int32 => "i32",
            Self::Int64 => "i64",
            Self::Float32 => "f32",
            Self::Float64 => "f64",
        }
    }

    fn zero_constant(self) -> &'static str {
        match self {
            Self::Int32 => "0 : i32",
            Self::Int64 => "0 : i64",
            Self::Float32 => "0.000000e+00 : f32",
            Self::Float64 => "0.000000e+00 : f64",
        }
    }
}

impl fmt::Display for PrimitiveArrowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrowValidityPlan {
    AllValid,
    CopyBitmap { source: String },
}

impl ArrowValidityPlan {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllValid => "all-valid",
            Self::CopyBitmap { .. } => "copy-bitmap",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowPrimitiveBufferPlan {
    pub primitive_type: PrimitiveArrowType,
    pub row_count: u64,
    pub value_buffer_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowColumnBufferPlan {
    pub builder_id: String,
    pub primitive: ArrowPrimitiveBufferPlan,
    pub validity: ArrowValidityPlan,
    pub null_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowTableBufferPlan {
    pub row_count: u64,
    pub columns: Vec<ArrowColumnBufferPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowBufferLoweringDiagnosticCode {
    UnsupportedType,
    UnsupportedNullability,
    UnsupportedShape,
}

impl ArrowBufferLoweringDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedType => "unsupported-type",
            Self::UnsupportedNullability => "unsupported-nullability",
            Self::UnsupportedShape => "unsupported-shape",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowBufferLoweringDiagnostic {
    pub code: ArrowBufferLoweringDiagnosticCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArrowBufferLoweringReport {
    diagnostics: Vec<ArrowBufferLoweringDiagnostic>,
    table: Option<ArrowTableBufferPlan>,
}

impl ArrowBufferLoweringReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && self.table.is_some()
    }

    pub fn diagnostics(&self) -> &[ArrowBufferLoweringDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&ArrowBufferLoweringDiagnostic> {
        self.diagnostics.first()
    }

    pub fn table(&self) -> Option<&ArrowTableBufferPlan> {
        self.table.as_ref()
    }

    fn push(
        &mut self,
        code: ArrowBufferLoweringDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(ArrowBufferLoweringDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

pub fn plan_arrow_buffers_from_decode_dialect(
    facts: &ProductionLoweringFacts,
) -> ArrowBufferLoweringReport {
    let mut report = ArrowBufferLoweringReport::default();
    let row_count = facts.shape.row_count();
    let mut columns = Vec::new();

    for (idx, column) in facts.shape.columns().iter().enumerate() {
        match plan_column(row_count, column) {
            Ok(plan) => columns.push(plan),
            Err(mut diagnostic) => {
                diagnostic.path = format!("$.columns[{idx}]{}", diagnostic.path);
                report.diagnostics.push(diagnostic);
            }
        }
    }

    if columns.is_empty() {
        report.push(
            ArrowBufferLoweringDiagnosticCode::UnsupportedShape,
            "$.columns",
            "Arrow buffer lowering requires at least one supported primitive column",
        );
    }

    if report.diagnostics.is_empty() {
        report.table = Some(ArrowTableBufferPlan { row_count, columns });
    }

    report
}

pub fn lower_arrow_buffers_to_standard_mlir(
    plan: &ArrowTableBufferPlan,
) -> Result<String, ArrowBufferLoweringReport> {
    if plan.columns.is_empty() {
        let mut report = ArrowBufferLoweringReport::default();
        report.push(
            ArrowBufferLoweringDiagnosticCode::UnsupportedShape,
            "$.columns",
            "standard MLIR lowering requires at least one output buffer",
        );
        return Err(report);
    }

    let args = plan
        .columns
        .iter()
        .map(|column| {
            format!(
                "%{}: memref<?x{}>",
                sanitize_symbol(&column.builder_id),
                column.primitive.primitive_type.mlir_type()
            )
        })
        .chain(std::iter::once("%rows: index".to_string()))
        .collect::<Vec<_>>()
        .join(", ");

    let mut text = String::new();
    text.push_str("module {\n");
    text.push_str(&format!(
        "  func.func @loom_decode_build_buffers({args}) {{\n"
    ));
    text.push_str("    %c0 = arith.constant 0 : index\n");
    text.push_str("    %c1 = arith.constant 1 : index\n");
    for (idx, column) in plan.columns.iter().enumerate() {
        text.push_str(&format!(
            "    %z{idx} = arith.constant {}\n",
            column.primitive.primitive_type.zero_constant()
        ));
    }
    text.push_str("    scf.for %row = %c0 to %rows step %c1 {\n");
    for (idx, column) in plan.columns.iter().enumerate() {
        text.push_str(&format!(
            "      memref.store %z{idx}, %{}[%row] : memref<?x{}>\n",
            sanitize_symbol(&column.builder_id),
            column.primitive.primitive_type.mlir_type()
        ));
    }
    text.push_str("    }\n");
    text.push_str("    return\n");
    text.push_str("  }\n");
    text.push_str("}\n");
    Ok(text)
}

/// Deprecated: retained only for `loom-ffi` internal use until Phase 40
/// validation replaces the raw-copy MLIR lowering path.
#[doc(hidden)]
pub fn lower_arrow_raw_copy_to_standard_mlir(
    plan: &ArrowTableBufferPlan,
) -> Result<String, ArrowBufferLoweringReport> {
    if plan.columns.is_empty() {
        let mut report = ArrowBufferLoweringReport::default();
        report.push(
            ArrowBufferLoweringDiagnosticCode::UnsupportedShape,
            "$.columns",
            "standard MLIR raw-copy lowering requires at least one primitive column",
        );
        return Err(report);
    }

    let input_args = plan.columns.iter().map(|column| {
        format!(
            "%{}_in: memref<?x{}>",
            sanitize_symbol(&column.builder_id),
            column.primitive.primitive_type.mlir_type()
        )
    });
    let output_args = plan.columns.iter().map(|column| {
        format!(
            "%{}_out: memref<?x{}>",
            sanitize_symbol(&column.builder_id),
            column.primitive.primitive_type.mlir_type()
        )
    });
    let args = input_args
        .chain(output_args)
        .chain(std::iter::once("%rows: index".to_string()))
        .collect::<Vec<_>>()
        .join(", ");

    let mut text = String::new();
    text.push_str("module {\n");
    text.push_str(&format!(
        "  func.func @loom_decode_build_buffers({args}) attributes {{ llvm.emit_c_interface }} {{\n"
    ));
    text.push_str("    %c0 = arith.constant 0 : index\n");
    text.push_str("    %c1 = arith.constant 1 : index\n");
    text.push_str("    scf.for %row = %c0 to %rows step %c1 {\n");
    for column in plan.columns.iter() {
        let id = sanitize_symbol(&column.builder_id);
        let ty = column.primitive.primitive_type.mlir_type();
        text.push_str(&format!(
            "      %value_{id} = memref.load %{id}_in[%row] : memref<?x{ty}>\n"
        ));
        text.push_str(&format!(
            "      memref.store %value_{id}, %{id}_out[%row] : memref<?x{ty}>\n"
        ));
    }
    text.push_str("    }\n");
    text.push_str("    return\n");
    text.push_str("  }\n");
    text.push_str("}\n");
    Ok(text)
}

/// Deprecated zeroed-reference helper removed from production path.
#[doc(hidden)]
pub fn reference_zeroed_value_bytes(plan: &ArrowColumnBufferPlan) -> Vec<u8> {
    vec![0u8; plan.primitive.value_buffer_bytes as usize]
}

fn plan_column(
    row_count: u64,
    column: &ProductionColumnShape,
) -> Result<ArrowColumnBufferPlan, ArrowBufferLoweringDiagnostic> {
    if column.nullable {
        return Err(ArrowBufferLoweringDiagnostic {
            code: ArrowBufferLoweringDiagnosticCode::UnsupportedNullability,
            path: ".nullable".to_string(),
            message: "primitive Arrow buffer lowering currently supports all-valid columns only"
                .to_string(),
        });
    }

    let Some(primitive_type) = PrimitiveArrowType::from_data_type(&crate::l2_to_arrow(&column.arrow_type)) else {
        return Err(ArrowBufferLoweringDiagnostic {
            code: ArrowBufferLoweringDiagnosticCode::UnsupportedType,
            path: ".arrow_type".to_string(),
            message: format!(
                "unsupported Arrow output type {:?}; expected Int32, Int64, Float32, or Float64",
                column.arrow_type
            ),
        });
    };

    let value_buffer_bytes = row_count.saturating_mul(primitive_type.byte_width());
    Ok(ArrowColumnBufferPlan {
        builder_id: column.builder_id.clone(),
        primitive: ArrowPrimitiveBufferPlan {
            primitive_type,
            row_count,
            value_buffer_bytes,
        },
        validity: ArrowValidityPlan::AllValid,
        null_count: Some(0),
    })
}

pub fn describe_column(plan: &ArrowColumnBufferPlan) -> String {
    format!(
        "{}:{}:{}bytes:{}",
        plan.builder_id,
        plan.primitive.primitive_type,
        plan.primitive.value_buffer_bytes,
        plan.validity.as_str()
    )
}

pub fn data_type_name(data_type: &DataType) -> &'static str {
    arrow_type_name(data_type)
}

fn sanitize_symbol(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
