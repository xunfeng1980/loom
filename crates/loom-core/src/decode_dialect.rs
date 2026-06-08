//! Loom-owned textual `loom.decode` dialect surface.
//!
//! This is a deterministic post-verification contract surface, not a registered
//! MLIR dialect dependency. A compiled C++/ODS dialect can be added behind
//! optional tooling once the op surface is stable.

use std::fmt;

use arrow_schema::DataType;

use crate::production_native_lowering::{
    ProductionColumnShape, ProductionLoweringFacts, ProductionLoweringShape,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeDialectOp {
    Module,
    Kernel,
    InputSlice,
    Column,
    Builder,
    Finish,
    ForRows,
    RawCopy,
    BitUnpack,
    ForDelta,
    ValidityAllValid,
    ValidityCopy,
}

impl DecodeDialectOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Module => "loom.decode.module",
            Self::Kernel => "loom.decode.kernel",
            Self::InputSlice => "loom.decode.input_slice",
            Self::Column => "loom.decode.column",
            Self::Builder => "loom.decode.builder",
            Self::Finish => "loom.decode.finish",
            Self::ForRows => "loom.decode.for_rows",
            Self::RawCopy => "loom.decode.raw_copy",
            Self::BitUnpack => "loom.decode.bit_unpack",
            Self::ForDelta => "loom.decode.for_delta",
            Self::ValidityAllValid => "loom.decode.validity_all_valid",
            Self::ValidityCopy => "loom.decode.validity_copy",
        }
    }
}

impl fmt::Display for DecodeDialectOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodeDialectModule {
    pub artifact_kind: String,
    pub payload_kind: String,
    pub row_count: u64,
    pub constraint_status: String,
    pub columns: Vec<ProductionColumnShape>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeDialectTextArtifact {
    pub backend: String,
    pub module_name: String,
    pub text: String,
    pub facts_linkage: String,
    pub row_count: u64,
    pub column_count: usize,
}

pub fn emit_decode_dialect_text(facts: &ProductionLoweringFacts) -> DecodeDialectTextArtifact {
    let module = DecodeDialectModule::from(facts);
    let mut text = String::new();
    text.push_str("module {\n");
    text.push_str(&format!(
        "  {} @loom_artifact {{artifact_kind = \"{}\", payload_kind = \"{}\", rows = {}, constraint_status = \"{}\", backend = \"{}\", columns = {}}}\n",
        DecodeDialectOp::Module,
        escape(&module.artifact_kind),
        escape(&module.payload_kind),
        module.row_count,
        module.constraint_status,
        facts.backend.as_str(),
        module.columns.len()
    ));
    text.push_str(&format!(
        "  {} @decode {{rows = {}}} {{\n",
        DecodeDialectOp::Kernel,
        module.row_count
    ));
    for column in &module.columns {
        text.push_str(&format!(
            "    {} @{} {{arrow_type = \"{}\", nullable = {}}}\n",
            DecodeDialectOp::Column,
            sanitize_symbol(&column.builder_id),
            arrow_type_name(&column.arrow_type),
            column.nullable
        ));
        text.push_str(&format!(
            "    {} @{} {{arrow_type = \"{}\", validity = \"all_valid\"}}\n",
            DecodeDialectOp::Builder,
            sanitize_symbol(&column.builder_id),
            arrow_type_name(&column.arrow_type)
        ));
    }
    text.push_str(&format!(
        "    {} %row = 0 to {} {{\n",
        DecodeDialectOp::ForRows,
        module.row_count
    ));
    for column in &module.columns {
        let symbol = sanitize_symbol(&column.builder_id);
        text.push_str(&format!(
            "      {} @{}[%row]\n",
            DecodeDialectOp::RawCopy,
            symbol
        ));
        text.push_str(&format!(
            "      {} @{}[%row]\n",
            DecodeDialectOp::ValidityAllValid,
            symbol
        ));
    }
    text.push_str("    }\n");
    for column in &module.columns {
        text.push_str(&format!(
            "    {} @{}\n",
            DecodeDialectOp::Finish,
            sanitize_symbol(&column.builder_id)
        ));
    }
    text.push_str("  }\n");
    text.push_str("}\n");

    DecodeDialectTextArtifact {
        backend: facts.backend.as_str().to_string(),
        module_name: "loom_artifact".to_string(),
        text,
        facts_linkage: format!(
            "artifact_kind={};payload_kind={};constraint_status={};rows={};columns={}",
            module.artifact_kind,
            module.payload_kind,
            module.constraint_status,
            module.row_count,
            module.columns.len()
        ),
        row_count: module.row_count,
        column_count: module.columns.len(),
    }
}

impl From<&ProductionLoweringFacts> for DecodeDialectModule {
    fn from(facts: &ProductionLoweringFacts) -> Self {
        Self {
            artifact_kind: facts.artifact_kind.clone(),
            payload_kind: facts.payload_kind.clone(),
            row_count: facts.shape.row_count(),
            constraint_status: facts.constraint_status.as_str().to_string(),
            columns: facts.shape.columns().to_vec(),
        }
    }
}

pub fn arrow_type_name(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Int32 => "int32",
        DataType::Int64 => "int64",
        DataType::Float32 => "float32",
        DataType::Float64 => "float64",
        _ => "unsupported",
    }
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

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[allow(dead_code)]
fn _shape_name(shape: &ProductionLoweringShape) -> &'static str {
    match shape {
        ProductionLoweringShape::SingleColumnPrimitive { .. } => "single-column-primitive",
        ProductionLoweringShape::PrimitiveTable { .. } => "primitive-table",
    }
}
