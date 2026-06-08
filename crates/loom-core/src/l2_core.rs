//! Tiny `L2Core` language model for the Phase 13 full-verifier foundation.
//!
//! This module is intentionally a data model first. The executable verifier in
//! later Phase 13 plans will type-check and abstract-interpret these values.

pub mod constraints;

use arrow_schema::DataType;

/// A future Loom total-function artifact in the tiny Phase 13 core language.
#[derive(Debug, Clone, PartialEq)]
pub struct L2CoreProgram {
    pub artifact_version: u16,
    pub required_features: Vec<String>,
    pub optional_features: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub resource_budget: ResourceBudget,
    pub body: Vec<L2CoreStmt>,
}

/// Explicit authority available to an `L2CoreProgram`.
#[derive(Debug, Clone, PartialEq)]
pub enum Capability {
    InputSlice(InputSliceCapability),
    Scratch(ScratchCapability),
    OutputBuilder(OutputBuilderCapability),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSliceCapability {
    pub id: String,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchCapability {
    pub id: String,
    pub max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputBuilderCapability {
    pub id: String,
    pub arrow_type: DataType,
    pub nullable: bool,
    pub max_events: u64,
}

/// Conservative per-artifact resource limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBudget {
    pub max_steps: u64,
    pub max_input_bytes_read: u64,
    pub max_scratch_bytes: u64,
    pub max_builder_events: u64,
    pub max_rows: u64,
    pub max_constraint_count: u64,
}

impl ResourceBudget {
    pub fn bounded_rows(rows: u64) -> Self {
        Self {
            max_steps: rows.saturating_mul(8).saturating_add(16),
            max_input_bytes_read: rows.saturating_mul(8),
            max_scratch_bytes: 0,
            max_builder_events: rows,
            max_rows: rows,
            max_constraint_count: 64,
        }
    }
}

/// Scalar types admitted by the tiny `L2Core` slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarType {
    Bool,
    Int32,
    Int64,
    UInt32,
    UInt64,
    Bytes,
    RowIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarValue {
    Bool(bool),
    Int32(i32),
    Int64(i64),
    UInt32(u32),
    UInt64(u64),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarExpr {
    Const(ScalarValue),
    Var(String),
    Add(Box<ScalarExpr>, Box<ScalarExpr>),
    Sub(Box<ScalarExpr>, Box<ScalarExpr>),
    Mul(Box<ScalarExpr>, Box<ScalarExpr>),
    Min(Box<ScalarExpr>, Box<ScalarExpr>),
    Max(Box<ScalarExpr>, Box<ScalarExpr>),
    Eq(Box<ScalarExpr>, Box<ScalarExpr>),
    Lt(Box<ScalarExpr>, Box<ScalarExpr>),
    Le(Box<ScalarExpr>, Box<ScalarExpr>),
}

impl ScalarExpr {
    pub fn var(name: impl Into<String>) -> Self {
        Self::Var(name.into())
    }

    pub fn u64(value: u64) -> Self {
        Self::Const(ScalarValue::UInt64(value))
    }
}

/// Statements admitted by the tiny `L2Core` slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L2CoreStmt {
    ForRange {
        index: String,
        start: ScalarExpr,
        end: ScalarExpr,
        body: Vec<L2CoreStmt>,
    },
    CursorLoop {
        cursor: String,
        limit: ScalarExpr,
        progress: ScalarExpr,
        body: Vec<L2CoreStmt>,
    },
    ReadInput {
        capability: String,
        offset: ScalarExpr,
        width: ScalarExpr,
        bind: String,
    },
    LetScalar {
        name: String,
        expr: ScalarExpr,
    },
    AppendValue {
        builder: String,
        value: ScalarExpr,
    },
    AppendNull {
        builder: String,
    },
    FailClosed {
        code: String,
    },
}

/// Event types emitted to typed Arrow builders.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrowEventType {
    AppendValue {
        builder_id: String,
        arrow_type: DataType,
    },
    AppendNull {
        builder_id: String,
        arrow_type: DataType,
    },
    Finish {
        builder_id: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputBuilderState {
    pub builder_id: String,
    pub arrow_type: DataType,
    pub nullable: bool,
    pub max_events: u64,
    pub emitted_events: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRangeFact {
    pub capability_id: String,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSchemaFact {
    pub builder_id: String,
    pub arrow_type: DataType,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopBoundFact {
    pub loop_id: String,
    pub max_iterations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityFact {
    InputSlice {
        id: String,
        offset: u64,
        length: u64,
    },
    Scratch {
        id: String,
        max_bytes: u64,
    },
    OutputBuilder {
        id: String,
        nullable: bool,
        max_events: u64,
    },
}

/// Stable verifier evidence emitted for later lowering preconditions.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedArtifactFacts {
    pub artifact_version: u16,
    pub required_features: Vec<String>,
    pub optional_features: Vec<String>,
    pub accepted_feature_set: Vec<String>,
    pub input_ranges: Vec<InputRangeFact>,
    pub output_schema: Vec<OutputSchemaFact>,
    pub row_count_bound: Option<u64>,
    pub loop_bounds: Vec<LoopBoundFact>,
    pub resource_bounds: ResourceBudget,
    pub builder_event_types: Vec<ArrowEventType>,
    pub capability_summary: Vec<CapabilityFact>,
    pub constraint_ids: Vec<String>,
    pub proof_obligation_ids: Vec<String>,
}

impl VerifiedArtifactFacts {
    pub fn for_program(
        program: &L2CoreProgram,
        constraint_ids: Vec<String>,
        proof_obligation_ids: Vec<String>,
    ) -> Self {
        let mut input_ranges = Vec::new();
        let mut output_schema = Vec::new();
        let mut capability_summary = Vec::new();
        let mut builder_event_types = Vec::new();

        for capability in &program.capabilities {
            match capability {
                Capability::InputSlice(input) => {
                    input_ranges.push(InputRangeFact {
                        capability_id: input.id.clone(),
                        offset: input.offset,
                        length: input.length,
                    });
                    capability_summary.push(CapabilityFact::InputSlice {
                        id: input.id.clone(),
                        offset: input.offset,
                        length: input.length,
                    });
                }
                Capability::Scratch(scratch) => {
                    capability_summary.push(CapabilityFact::Scratch {
                        id: scratch.id.clone(),
                        max_bytes: scratch.max_bytes,
                    });
                }
                Capability::OutputBuilder(builder) => {
                    output_schema.push(OutputSchemaFact {
                        builder_id: builder.id.clone(),
                        arrow_type: builder.arrow_type.clone(),
                        nullable: builder.nullable,
                    });
                    capability_summary.push(CapabilityFact::OutputBuilder {
                        id: builder.id.clone(),
                        nullable: builder.nullable,
                        max_events: builder.max_events,
                    });
                    builder_event_types.push(ArrowEventType::AppendValue {
                        builder_id: builder.id.clone(),
                        arrow_type: builder.arrow_type.clone(),
                    });
                    if builder.nullable {
                        builder_event_types.push(ArrowEventType::AppendNull {
                            builder_id: builder.id.clone(),
                            arrow_type: builder.arrow_type.clone(),
                        });
                    }
                    builder_event_types.push(ArrowEventType::Finish {
                        builder_id: builder.id.clone(),
                    });
                }
            }
        }

        Self {
            artifact_version: program.artifact_version,
            required_features: program.required_features.clone(),
            optional_features: program.optional_features.clone(),
            accepted_feature_set: program.required_features.clone(),
            input_ranges,
            output_schema,
            row_count_bound: Some(program.resource_budget.max_rows),
            loop_bounds: collect_loop_bounds(&program.body, program.resource_budget.max_rows),
            resource_bounds: program.resource_budget.clone(),
            builder_event_types,
            capability_summary,
            constraint_ids,
            proof_obligation_ids,
        }
    }
}

fn collect_loop_bounds(body: &[L2CoreStmt], fallback_bound: u64) -> Vec<LoopBoundFact> {
    let mut bounds = Vec::new();
    collect_loop_bounds_inner(body, fallback_bound, &mut bounds);
    bounds
}

fn collect_loop_bounds_inner(
    body: &[L2CoreStmt],
    fallback_bound: u64,
    out: &mut Vec<LoopBoundFact>,
) {
    for stmt in body {
        match stmt {
            L2CoreStmt::ForRange { index, body, .. } => {
                out.push(LoopBoundFact {
                    loop_id: index.clone(),
                    max_iterations: fallback_bound,
                });
                collect_loop_bounds_inner(body, fallback_bound, out);
            }
            L2CoreStmt::CursorLoop { cursor, body, .. } => {
                out.push(LoopBoundFact {
                    loop_id: cursor.clone(),
                    max_iterations: fallback_bound,
                });
                collect_loop_bounds_inner(body, fallback_bound, out);
            }
            L2CoreStmt::ReadInput { .. }
            | L2CoreStmt::LetScalar { .. }
            | L2CoreStmt::AppendValue { .. }
            | L2CoreStmt::AppendNull { .. }
            | L2CoreStmt::FailClosed { .. } => {}
        }
    }
}
