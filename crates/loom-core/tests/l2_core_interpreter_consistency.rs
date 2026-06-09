use std::collections::HashMap;

use arrow_schema::DataType;
use loom_core::full_verifier::verify_l2_core;
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, ScalarType, ScalarValue,
};
use loom_core::l2_core_reference_executor::execute_reference;

fn sample_program() -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input0".to_string(),
                offset: 0,
                length: 16,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "out0".to_string(),
                arrow_type: DataType::Int32,
                nullable: true,
                max_events: 4,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(4),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::u64(0),
            end: ScalarExpr::u64(4),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: "input0".to_string(),
                    offset: ScalarExpr::Add(
                        Box::new(ScalarExpr::var("i")),
                        Box::new(ScalarExpr::u64(0)),
                    ),
                    width: ScalarExpr::u64(4),
                    bind: "value".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: "out0".to_string(),
                    value: ScalarExpr::var("value"),
                },
            ],
        }],
    }
}

struct ConsistencyCase {
    id: &'static str,
    program: L2CoreProgram,
}

fn consistency_cases() -> Vec<ConsistencyCase> {
    let mut append_null = sample_program();
    append_null.body = vec![L2CoreStmt::AppendNull {
        builder: "out0".to_string(),
    }];

    let mut missing_input = sample_program();
    missing_input
        .capabilities
        .retain(|capability| !matches!(capability, Capability::InputSlice(_)));

    let mut missing_output = sample_program();
    missing_output.body = vec![L2CoreStmt::AppendNull {
        builder: "missing".to_string(),
    }];

    let mut invalid_loop = sample_program();
    invalid_loop.body = vec![L2CoreStmt::ForRange {
        index: "i".to_string(),
        start: ScalarExpr::u64(4),
        end: ScalarExpr::u64(0),
        body: vec![],
    }];

    let mut non_monotone = sample_program();
    non_monotone.body = vec![L2CoreStmt::CursorLoop {
        cursor: "cursor".to_string(),
        limit: ScalarExpr::u64(4),
        progress: ScalarExpr::var("cursor"),
        body: vec![],
    }];

    let mut fuzz_000 = sample_program();
    fuzz_000.body = vec![
        L2CoreStmt::LetScalar {
            name: "x".to_string(),
            expr: ScalarExpr::Const(ScalarValue::Int32(7)),
        },
        L2CoreStmt::LetScalar {
            name: "y".to_string(),
            expr: ScalarExpr::Add(
                Box::new(ScalarExpr::var("x")),
                Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            ),
        },
        L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: ScalarExpr::var("y"),
        },
    ];

    let fail_closed = {
        let mut program = sample_program();
        program.body = vec![L2CoreStmt::FailClosed {
            code: "test-fail-closed".to_string(),
        }];
        program
    };

    vec![
        ConsistencyCase {
            id: "matrix-accepted-copy",
            program: sample_program(),
        },
        ConsistencyCase {
            id: "matrix-append-null",
            program: append_null,
        },
        ConsistencyCase {
            id: "matrix-missing-input-capability",
            program: missing_input,
        },
        ConsistencyCase {
            id: "matrix-missing-output-builder",
            program: missing_output,
        },
        ConsistencyCase {
            id: "matrix-invalid-loop-bounds",
            program: invalid_loop,
        },
        ConsistencyCase {
            id: "matrix-non-monotone-cursor-loop",
            program: non_monotone,
        },
        ConsistencyCase {
            id: "matrix-fail-closed",
            program: fail_closed,
        },
        ConsistencyCase {
            id: "fuzz-000-let-add-int32",
            program: fuzz_000,
        },
    ]
}

/// Observer-only production trace subject under test.
///
/// The current repository does not expose a separate user-facing L2Core runtime
/// interpreter for this modeled slice. This helper defines the narrow
/// interpreter surface under test for Phase 39: production verifier
/// classification plus an independent event walk for accepted programs. It does
/// not call reference executor code.
fn production_trace_subject(program: &L2CoreProgram) -> Vec<String> {
    if let Some(first_error) = verify_l2_core(program).first_error() {
        return vec![format!("fail-closed:{}", first_error.code.as_str())];
    }

    let mut subject = ProductionTraceSubject::new(program);
    subject.exec_body(&program.body);
    if !subject.failed {
        subject.trace.push("terminal:finished".to_string());
    }
    subject.trace
}

struct ProductionTraceSubject {
    input_capabilities: HashMap<String, InputSliceCapability>,
    output_builders: HashMap<String, (ScalarType, bool)>,
    scalar_types: HashMap<String, ScalarType>,
    trace: Vec<String>,
    failed: bool,
}

impl ProductionTraceSubject {
    fn new(program: &L2CoreProgram) -> Self {
        let mut input_capabilities = HashMap::new();
        let mut output_builders = HashMap::new();
        for capability in &program.capabilities {
            match capability {
                Capability::InputSlice(input) => {
                    input_capabilities.insert(input.id.clone(), input.clone());
                }
                Capability::OutputBuilder(builder) => {
                    if let Some(scalar_type) = scalar_type_for_arrow(&builder.arrow_type) {
                        output_builders.insert(builder.id.clone(), (scalar_type, builder.nullable));
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
            failed: false,
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
                let Some(input) = self.input_capabilities.get(capability) else {
                    self.fail_closed("missing-input-capability");
                    return;
                };
                let in_bounds = match (const_u64(offset), const_u64(width)) {
                    (Some(offset), Some(width)) => {
                        offset >= input.offset
                            && offset.saturating_add(width) <= input.offset + input.length
                    }
                    _ => true,
                };
                self.trace.push(format!(
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
                let Some(ty) = self.type_of_expr(expr) else {
                    self.fail_closed("unknown-variable");
                    return;
                };
                self.scalar_types.insert(name.clone(), ty);
            }
            L2CoreStmt::AppendValue { builder, value } => {
                let Some(actual) = self.type_of_expr(value) else {
                    self.fail_closed("unknown-variable");
                    return;
                };
                let Some((expected, _)) = self.output_builders.get(builder) else {
                    self.fail_closed("missing-output-builder");
                    return;
                };
                if expected != &actual {
                    self.fail_closed("output-type-mismatch");
                    return;
                }
                self.trace
                    .push(format!("append-value:{builder}:{}", scalar_type_name(&actual)));
            }
            L2CoreStmt::AppendNull { builder } => {
                let Some((ty, nullable)) = self.output_builders.get(builder) else {
                    self.fail_closed("missing-output-builder");
                    return;
                };
                if !nullable {
                    self.fail_closed("output-nullability-mismatch");
                    return;
                }
                self.trace
                    .push(format!("append-null:{builder}:{}", scalar_type_name(ty)));
            }
            L2CoreStmt::ForRange { index, body, .. } => {
                let before = self.scalar_types.clone();
                self.scalar_types
                    .insert(index.clone(), ScalarType::RowIndex);
                self.exec_body(body);
                self.scalar_types = before;
            }
            L2CoreStmt::CursorLoop { cursor, body, .. } => {
                let before = self.scalar_types.clone();
                self.scalar_types
                    .insert(cursor.clone(), ScalarType::RowIndex);
                self.exec_body(body);
                self.scalar_types = before;
            }
            L2CoreStmt::FailClosed { code } => self.fail_closed(code),
        }
    }

    fn fail_closed(&mut self, code: &str) {
        self.trace.push(format!("fail-closed:{code}"));
        self.failed = true;
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

#[test]
fn reference_and_production_trace_subjects_match() {
    for case in consistency_cases() {
        let reference_trace = execute_reference(&case.program).trace_lines();
        let production_trace = production_trace_subject(&case.program);
        assert_eq!(
            production_trace, reference_trace,
            "trace divergence for {}",
            case.id
        );
    }
}

fn scalar_type_for_arrow(data_type: &DataType) -> Option<ScalarType> {
    match data_type {
        DataType::Boolean => Some(ScalarType::Bool),
        DataType::Int32 => Some(ScalarType::Int32),
        DataType::Int64 => Some(ScalarType::Int64),
        DataType::Float32 => Some(ScalarType::Float32),
        DataType::Float64 => Some(ScalarType::Float64),
        DataType::Utf8 => Some(ScalarType::Bytes),
        _ => None,
    }
}

fn scalar_type_for_read_width(width: &ScalarExpr) -> ScalarType {
    match const_u64(width) {
        Some(4) => ScalarType::Int32,
        Some(8) => ScalarType::Int64,
        _ => ScalarType::Bytes,
    }
}

fn type_of_const(value: &ScalarValue) -> ScalarType {
    match value {
        ScalarValue::Bool(_) => ScalarType::Bool,
        ScalarValue::Int32(_) => ScalarType::Int32,
        ScalarValue::Int64(_) => ScalarType::Int64,
        ScalarValue::Float32Bits(_) => ScalarType::Float32,
        ScalarValue::Float64Bits(_) => ScalarType::Float64,
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

fn scalar_type_name(scalar_type: &ScalarType) -> &'static str {
    match scalar_type {
        ScalarType::Bool => "bool",
        ScalarType::Int32 => "int32",
        ScalarType::Int64 => "int64",
        ScalarType::Float32 => "float32",
        ScalarType::Float64 => "float64",
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
