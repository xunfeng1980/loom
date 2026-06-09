//! Reference-only modeled executor for Phase 39 differential validation.
//!
//! This module is a Rust transcription of the Phase 38 Lean modeled executor.
//! It is a differential oracle, not the production interpreter and not a
//! fallback path. Production execution must not call this module to make
//! behavior "match" the model.

use std::collections::HashMap;

use arrow_schema::DataType;

use crate::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, ScalarExpr, ScalarType,
    ScalarValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceStatus {
    Finished,
    FailClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTrace {
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceReport {
    pub status: ReferenceStatus,
    pub trace: Vec<ReferenceTrace>,
}

impl ReferenceReport {
    pub fn trace_lines(&self) -> Vec<String> {
        self.trace.iter().map(|row| row.line.clone()).collect()
    }
}

#[derive(Debug, Clone)]
struct OutputInfo {
    scalar_type: ScalarType,
    nullable: bool,
}

#[derive(Debug, Clone)]
struct ReferenceExecutor<'a> {
    input_capabilities: HashMap<String, &'a InputSliceCapability>,
    output_builders: HashMap<String, OutputInfo>,
    scalar_types: HashMap<String, ScalarType>,
    trace: Vec<ReferenceTrace>,
    rows_used: u64,
    max_rows: u64,
    failed: bool,
}

/// Execute the Rust reference oracle for the modeled L2Core semantics.
pub fn execute_reference(program: &L2CoreProgram) -> ReferenceReport {
    ReferenceExecutor::new(program).execute(program)
}

impl<'a> ReferenceExecutor<'a> {
    fn new(program: &'a L2CoreProgram) -> Self {
        let mut input_capabilities = HashMap::new();
        let mut output_builders = HashMap::new();

        for capability in &program.capabilities {
            match capability {
                Capability::InputSlice(input) => {
                    input_capabilities.insert(input.id.clone(), input);
                }
                Capability::OutputBuilder(builder) => {
                    if let Some(scalar_type) = scalar_type_for_arrow(&builder.arrow_type) {
                        output_builders.insert(
                            builder.id.clone(),
                            OutputInfo {
                                scalar_type,
                                nullable: builder.nullable,
                            },
                        );
                    }
                }
                Capability::Scratch(_) => {}
            }
        }

        Self {
            input_capabilities,
            output_builders,
            scalar_types: HashMap::new(),
            trace: Vec::new(),
            rows_used: 0,
            max_rows: program.resource_budget.max_rows,
            failed: false,
        }
    }

    fn execute(mut self, program: &L2CoreProgram) -> ReferenceReport {
        self.exec_body(&program.body);
        if self.failed {
            ReferenceReport {
                status: ReferenceStatus::FailClosed,
                trace: self.trace,
            }
        } else {
            self.trace("terminal:finished");
            ReferenceReport {
                status: ReferenceStatus::Finished,
                trace: self.trace,
            }
        }
    }

    fn trace(&mut self, line: impl Into<String>) {
        self.trace.push(ReferenceTrace { line: line.into() });
    }

    fn fail_closed(&mut self, code: &str) {
        if !self.failed {
            self.trace(format!("fail-closed:{code}"));
            self.failed = true;
        }
    }

    fn exec_body(&mut self, body: &[L2CoreStmt]) {
        for stmt in body {
            if self.failed {
                return;
            }
            self.exec_stmt(stmt);
        }
    }

    fn exec_stmt(&mut self, stmt: &L2CoreStmt) {
        match stmt {
            L2CoreStmt::ReadInput {
                capability,
                offset,
                width,
                bind,
            } => {
                if self.type_of_expr(offset).is_none() || self.type_of_expr(width).is_none() {
                    self.fail_closed("unknown-variable");
                    return;
                }
                let Some(input) = self.input_capabilities.get(capability) else {
                    self.fail_closed("missing-input-capability");
                    return;
                };
                let in_bounds = concrete_read_in_range(input, offset, width);
                self.trace(format!(
                    "read:{capability}:offset={}:width={}:in-bounds={in_bounds}",
                    expr_label(offset),
                    expr_label(width)
                ));
                if !in_bounds {
                    self.fail_closed("read-out-of-bounds");
                    return;
                }
                self.scalar_types
                    .insert(bind.clone(), scalar_type_for_read_width(width));
            }
            L2CoreStmt::LetScalar { name, expr } => {
                let Some(scalar_type) = self.type_of_expr(expr) else {
                    self.fail_closed("unknown-variable");
                    return;
                };
                self.scalar_types.insert(name.clone(), scalar_type);
            }
            L2CoreStmt::AppendValue { builder, value } => {
                let Some(actual) = self.type_of_expr(value) else {
                    self.fail_closed("unknown-variable");
                    return;
                };
                let Some(output) = self.output_builders.get(builder) else {
                    self.fail_closed("missing-output-builder");
                    return;
                };
                if output.scalar_type != actual {
                    self.fail_closed("output-type-mismatch");
                    return;
                }
                self.trace(format!("append-value:{builder}:{}", scalar_type_name(&actual)));
            }
            L2CoreStmt::AppendNull { builder } => {
                let Some(output) = self.output_builders.get(builder) else {
                    self.fail_closed("missing-output-builder");
                    return;
                };
                if !output.nullable {
                    self.fail_closed("output-nullability-mismatch");
                    return;
                }
                self.trace(format!(
                    "append-null:{builder}:{}",
                    scalar_type_name(&output.scalar_type)
                ));
            }
            L2CoreStmt::ForRange {
                index,
                start,
                end,
                body,
            } => {
                let Some(start) = const_u64(start) else {
                    self.fail_closed("invalid-loop-bounds");
                    return;
                };
                let Some(end) = const_u64(end) else {
                    self.fail_closed("invalid-loop-bounds");
                    return;
                };
                if end < start {
                    self.fail_closed("invalid-loop-bounds");
                    return;
                }
                let count = end - start;
                if count > self.max_rows {
                    self.fail_closed("resource-budget-exceeded");
                    return;
                }
                self.rows_used = self.rows_used.saturating_add(count);
                let before = self.scalar_types.clone();
                self.scalar_types
                    .insert(index.clone(), ScalarType::RowIndex);
                self.exec_body(body);
                self.scalar_types = before;
            }
            L2CoreStmt::CursorLoop {
                cursor,
                limit,
                progress,
                body,
            } => {
                if !is_monotone_progress(cursor, progress) {
                    self.fail_closed("non-monotone-cursor-loop");
                    return;
                }
                let Some(limit) = const_u64(limit) else {
                    self.fail_closed("invalid-loop-bounds");
                    return;
                };
                if limit > self.max_rows {
                    self.fail_closed("resource-budget-exceeded");
                    return;
                }
                self.rows_used = self.rows_used.saturating_add(limit);
                let before = self.scalar_types.clone();
                self.scalar_types
                    .insert(cursor.clone(), ScalarType::RowIndex);
                self.exec_body(body);
                self.scalar_types = before;
            }
            L2CoreStmt::FailClosed { code } => self.fail_closed(code),
        }
    }

    fn type_of_expr(&self, expr: &ScalarExpr) -> Option<ScalarType> {
        match expr {
            ScalarExpr::Const(value) => Some(type_of_const(value)),
            ScalarExpr::Var(name) => self.scalar_types.get(name).cloned(),
            ScalarExpr::Add(lhs, rhs)
            | ScalarExpr::Sub(lhs, rhs)
            | ScalarExpr::Mul(lhs, rhs)
            | ScalarExpr::Min(lhs, rhs)
            | ScalarExpr::Max(lhs, rhs) => {
                let lhs_ty = self.type_of_expr(lhs)?;
                self.type_of_expr(rhs)?;
                Some(lhs_ty)
            }
            ScalarExpr::Eq(lhs, rhs) | ScalarExpr::Lt(lhs, rhs) | ScalarExpr::Le(lhs, rhs) => {
                self.type_of_expr(lhs)?;
                self.type_of_expr(rhs)?;
                Some(ScalarType::Bool)
            }
        }
    }
}

fn concrete_read_in_range(
    input: &InputSliceCapability,
    offset: &ScalarExpr,
    width: &ScalarExpr,
) -> bool {
    match (const_u64(offset), const_u64(width)) {
        (Some(offset), Some(width)) => {
            offset >= input.offset && offset.saturating_add(width) <= input.offset + input.length
        }
        _ => true,
    }
}

fn scalar_type_for_read_width(width: &ScalarExpr) -> ScalarType {
    match const_u64(width) {
        Some(4) => ScalarType::Int32,
        Some(8) => ScalarType::Int64,
        _ => ScalarType::Bytes,
    }
}

fn scalar_type_for_arrow(data_type: &DataType) -> Option<ScalarType> {
    match data_type {
        DataType::Boolean => Some(ScalarType::Bool),
        DataType::Int32 => Some(ScalarType::Int32),
        DataType::Int64 => Some(ScalarType::Int64),
        DataType::Utf8 => Some(ScalarType::Bytes),
        _ => None,
    }
}

fn type_of_const(value: &ScalarValue) -> ScalarType {
    match value {
        ScalarValue::Bool(_) => ScalarType::Bool,
        ScalarValue::Int32(_) => ScalarType::Int32,
        ScalarValue::Int64(_) => ScalarType::Int64,
        ScalarValue::UInt32(_) => ScalarType::UInt32,
        ScalarValue::UInt64(_) => ScalarType::UInt64,
        ScalarValue::Bytes(_) => ScalarType::Bytes,
    }
}

fn const_u64(expr: &ScalarExpr) -> Option<u64> {
    match expr {
        ScalarExpr::Const(ScalarValue::UInt64(value)) => Some(*value),
        ScalarExpr::Const(ScalarValue::UInt32(value)) => Some((*value).into()),
        ScalarExpr::Const(ScalarValue::Int32(value)) if *value >= 0 => Some(*value as u64),
        ScalarExpr::Const(ScalarValue::Int64(value)) if *value >= 0 => Some(*value as u64),
        _ => None,
    }
}

fn is_monotone_progress(cursor: &str, expr: &ScalarExpr) -> bool {
    match expr {
        ScalarExpr::Add(lhs, rhs) => {
            matches!(lhs.as_ref(), ScalarExpr::Var(name) if name == cursor)
                && const_u64(rhs).is_some_and(|value| value > 0)
        }
        _ => false,
    }
}

fn scalar_type_name(scalar_type: &ScalarType) -> &'static str {
    match scalar_type {
        ScalarType::Bool => "bool",
        ScalarType::Int32 => "int32",
        ScalarType::Int64 => "int64",
        ScalarType::UInt32 => "uint32",
        ScalarType::UInt64 => "uint64",
        ScalarType::Bytes => "bytes",
        ScalarType::RowIndex => "row-index",
    }
}

fn expr_label(expr: &ScalarExpr) -> String {
    match const_u64(expr) {
        Some(value) => value.to_string(),
        None => "expr".to_string(),
    }
}
