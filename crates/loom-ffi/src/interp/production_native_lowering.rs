//! Production native-lowering support gate for Phase 20+.
//!
//! This module is deliberately separate from `native_lowering`, which preserves
//! the Phase 14 bounded-copy spike. Production lowering consumes the unified
//! artifact verifier report and requires discharged solver facts before any
//! dialect or MLIR artifact can be emitted.

use std::fmt;

use arrow_schema::DataType;

use super::artifact_types::{
    ArtifactVerificationReport, ArtifactVerificationStatus,
};
use super::decode_dialect::{emit_decode_dialect_text, DecodeDialectTextArtifact};
use super::l1_model::{LayoutDescription, LayoutNode};
use loom_ir_core::l2_core::OutputSchemaFact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionLoweringBackend {
    LoomDecodeDialect,
}

impl ProductionLoweringBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoomDecodeDialect => "loom-decode-dialect",
        }
    }
}

impl fmt::Display for ProductionLoweringBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionLoweringDiagnosticCode {
    VerifierRejected,
    MissingArtifactFacts,
    MissingL2Facts,
    MissingRowCountBound,
    ConstraintsNotDischarged,
    UnsupportedPayload,
    UnsupportedType,
    UnsupportedNullability,
    UnsupportedKernel,
    UnsupportedMultiColumnShape,
    UnsupportedFeature,
    UnsupportedShape,
}

impl ProductionLoweringDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifierRejected => "verifier-rejected",
            Self::MissingArtifactFacts => "missing-artifact-facts",
            Self::MissingL2Facts => "missing-l2-facts",
            Self::MissingRowCountBound => "missing-row-count-bound",
            Self::ConstraintsNotDischarged => "constraints-not-discharged",
            Self::UnsupportedPayload => "unsupported-payload",
            Self::UnsupportedType => "unsupported-type",
            Self::UnsupportedNullability => "unsupported-nullability",
            Self::UnsupportedKernel => "unsupported-kernel",
            Self::UnsupportedMultiColumnShape => "unsupported-multi-column-shape",
            Self::UnsupportedFeature => "unsupported-feature",
            Self::UnsupportedShape => "unsupported-shape",
        }
    }
}

impl fmt::Display for ProductionLoweringDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionLoweringDiagnostic {
    pub code: ProductionLoweringDiagnosticCode,
    pub path: String,
    pub message: String,
}

use loom_ir_core::l2_core::L2DataType;

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionColumnShape {
    pub builder_id: String,
    pub arrow_type: L2DataType,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProductionLoweringShape {
    SingleColumnPrimitive {
        row_count: u64,
        column: ProductionColumnShape,
    },
    PrimitiveTable {
        row_count: u64,
        columns: Vec<ProductionColumnShape>,
    },
}

impl ProductionLoweringShape {
    pub fn row_count(&self) -> u64 {
        match self {
            Self::SingleColumnPrimitive { row_count, .. }
            | Self::PrimitiveTable { row_count, .. } => *row_count,
        }
    }

    pub fn columns(&self) -> &[ProductionColumnShape] {
        match self {
            Self::SingleColumnPrimitive { column, .. } => std::slice::from_ref(column),
            Self::PrimitiveTable { columns, .. } => columns,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionLoweringFacts {
    pub backend: ProductionLoweringBackend,
    pub artifact_kind: String,
    pub payload_kind: String,
    pub constraints_discharged: bool,
    pub shape: ProductionLoweringShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionNativeKernel {
    BitpackPrimitiveUnpack,
    FrameOfReferencePrimitiveDecode,
}

impl ProductionNativeKernel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BitpackPrimitiveUnpack => "bitpack-primitive-unpack",
            Self::FrameOfReferencePrimitiveDecode => "frame-of-reference-primitive-decode",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProductionLoweringSupportReport {
    diagnostics: Vec<ProductionLoweringDiagnostic>,
    facts: Option<ProductionLoweringFacts>,
}

impl ProductionLoweringSupportReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && self.facts.is_some()
    }

    pub fn diagnostics(&self) -> &[ProductionLoweringDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&ProductionLoweringDiagnostic> {
        self.diagnostics.first()
    }

    pub fn facts(&self) -> Option<&ProductionLoweringFacts> {
        self.facts.as_ref()
    }

    fn push(
        &mut self,
        code: ProductionLoweringDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(ProductionLoweringDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

pub fn check_production_lowering_support(
    report: &ArtifactVerificationReport,
) -> ProductionLoweringSupportReport {
    let mut support = ProductionLoweringSupportReport::default();

    if report.status() != ArtifactVerificationStatus::Accepted || !report.is_ok() {
        support.push(
            ProductionLoweringDiagnosticCode::VerifierRejected,
            "$.verification",
            "production native lowering requires an accepted artifact verifier report",
        );
        return support;
    }

    let Some(facts) = report.facts() else {
        support.push(
            ProductionLoweringDiagnosticCode::MissingArtifactFacts,
            "$.facts",
            "accepted artifact report did not expose artifact facts",
        );
        return support;
    };

    let row_count = facts.row_count_bound;
    if row_count.is_none() {
        support.push(
            ProductionLoweringDiagnosticCode::MissingRowCountBound,
            "$.facts.row_count_bound",
            "production native lowering requires a finite row-count bound",
        );
    }

    let Some(payload_kind) = facts.payload_kind.as_deref() else {
        support.push(
            ProductionLoweringDiagnosticCode::UnsupportedPayload,
            "$.facts.payload_kind",
            "production native lowering requires a supported payload kind",
        );
        return support;
    };

    if !matches!(payload_kind, "LMP1 layout" | "LMT1 table") {
        support.push(
            ProductionLoweringDiagnosticCode::UnsupportedPayload,
            "$.facts.payload_kind",
            format!("unsupported production payload kind '{payload_kind}'"),
        );
    }

    let Some(l2_facts) = facts.l2_core.as_ref() else {
        support.push(
            ProductionLoweringDiagnosticCode::MissingL2Facts,
            "$.facts.l2_core",
            "production native lowering requires associated L2/native facts",
        );
        return support;
    };

    let columns = supported_columns(&l2_facts.output_schema, &mut support);
    let Some(row_count) = row_count else {
        return support;
    };
    if columns.is_empty() {
        return support;
    }

    let shape = match payload_kind {
        "LMP1 layout" if columns.len() == 1 => ProductionLoweringShape::SingleColumnPrimitive {
            row_count,
            column: columns[0].clone(),
        },
        "LMP1 layout" => {
            support.push(
                ProductionLoweringDiagnosticCode::UnsupportedMultiColumnShape,
                "$.facts.l2_core.output_schema",
                "single-column layout payload must expose exactly one output column",
            );
            return support;
        }
        "LMT1 table" => ProductionLoweringShape::PrimitiveTable { row_count, columns },
        _ => return support,
    };

    if support.diagnostics.is_empty() {
        support.facts = Some(ProductionLoweringFacts {
            backend: ProductionLoweringBackend::LoomDecodeDialect,
            artifact_kind: facts.artifact_kind.clone(),
            payload_kind: payload_kind.to_string(),
            // Phase A–C: no in-TCB constraint discharge.
            constraints_discharged: false,
            shape,
        });
    }

    support
}

pub fn lower_to_decode_dialect_text(
    report: &ArtifactVerificationReport,
) -> Result<DecodeDialectTextArtifact, ProductionLoweringSupportReport> {
    let support = check_production_lowering_support(report);
    if !support.is_supported() {
        return Err(support);
    }
    let facts = support
        .facts()
        .expect("supported report must expose production lowering facts");
    Ok(emit_decode_dialect_text(facts))
}

fn supported_columns(
    output_schema: &[OutputSchemaFact],
    support: &mut ProductionLoweringSupportReport,
) -> Vec<ProductionColumnShape> {
    if output_schema.is_empty() {
        support.push(
            ProductionLoweringDiagnosticCode::UnsupportedShape,
            "$.facts.l2_core.output_schema",
            "production native lowering requires at least one output column",
        );
        return Vec::new();
    }

    let mut columns = Vec::with_capacity(output_schema.len());
    for (idx, column) in output_schema.iter().enumerate() {
        let path = format!("$.facts.l2_core.output_schema[{idx}]");
        if !is_supported_primitive(&column.arrow_type) {
            support.push(
                ProductionLoweringDiagnosticCode::UnsupportedType,
                path.clone(),
                format!(
                    "unsupported production output type {:?}; expected Int32, Int64, Float32, or Float64",
                    column.arrow_type
                ),
            );
            continue;
        }
        if column.nullable {
            support.push(
                ProductionLoweringDiagnosticCode::UnsupportedNullability,
                path,
                "Phase 20 production lowering initially supports non-null primitive output only",
            );
            continue;
        }
        columns.push(ProductionColumnShape {
            builder_id: column.builder_id.clone(),
            arrow_type: column.arrow_type.clone(),
            nullable: column.nullable,
        });
    }
    columns
}

pub fn is_supported_primitive(data_type: &L2DataType) -> bool {
    matches!(
        data_type,
        L2DataType::Int32 | L2DataType::Int64 | L2DataType::Float32 | L2DataType::Float64
    )
}

pub fn check_layout_kernel_support(
    layout: &LayoutDescription,
) -> Result<ProductionNativeKernel, ProductionLoweringDiagnostic> {
    let Some(expected_width) = primitive_byte_width(&layout.data_type) else {
        return Err(ProductionLoweringDiagnostic {
            code: ProductionLoweringDiagnosticCode::UnsupportedType,
            path: "$.data_type".to_string(),
            message: format!(
                "unsupported native kernel output type {:?}; expected Int32, Int64, Float32, or Float64",
                layout.data_type
            ),
        });
    };

    match &layout.root {
        LayoutNode::Raw {
            elem_size, count, ..
        } => {
            if u64::from(*elem_size) != expected_width {
                return Err(ProductionLoweringDiagnostic {
                    code: ProductionLoweringDiagnosticCode::UnsupportedShape,
                    path: "$.root.elem_size".to_string(),
                    message: format!(
                        "raw primitive elem_size {} does not match {:?} byte width {}",
                        elem_size, layout.data_type, expected_width
                    ),
                });
            }
            if *count != layout.row_count {
                return Err(ProductionLoweringDiagnostic {
                    code: ProductionLoweringDiagnosticCode::UnsupportedShape,
                    path: "$.root.count".to_string(),
                    message: format!(
                        "raw primitive count {} does not match row_count {}",
                        count, layout.row_count
                    ),
                });
            }
            Err(ProductionLoweringDiagnostic {
                code: ProductionLoweringDiagnosticCode::UnsupportedKernel,
                path: "$.root.Raw".to_string(),
                message: "raw-copy primitive kernel removed from production path pending Phase 40 validation".to_string(),
            })
        }
        LayoutNode::BitPack { .. } => Err(ProductionLoweringDiagnostic {
            code: ProductionLoweringDiagnosticCode::UnsupportedKernel,
            path: "$.root.BitPack".to_string(),
            message: "bitpack native lowering is deferred until Phase 21 pairs encoding coverage with discharged bit-offset lowering facts".to_string(),
        }),
        LayoutNode::FrameOfReference { .. } => Err(ProductionLoweringDiagnostic {
            code: ProductionLoweringDiagnosticCode::UnsupportedKernel,
            path: "$.root.FrameOfReference".to_string(),
            message: "frame-of-reference native lowering is deferred until overflow/range facts are paired with the encoding expansion".to_string(),
        }),
        LayoutNode::Dictionary { .. }
        | LayoutNode::RunEnd { .. }
        | LayoutNode::KernelEscape { .. } => Err(ProductionLoweringDiagnostic {
            code: ProductionLoweringDiagnosticCode::UnsupportedKernel,
            path: "$.root".to_string(),
            message: "dictionary, RLE, and L2 kernel native lowering are deferred beyond the Phase 20 primitive matrix".to_string(),
        }),
    }
}

fn primitive_byte_width(data_type: &DataType) -> Option<u64> {
    match data_type {
        DataType::Int32 | DataType::Float32 => Some(4),
        DataType::Int64 | DataType::Float64 => Some(8),
        _ => None,
    }
}
