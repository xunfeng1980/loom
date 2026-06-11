//! Executable Rust verifier MVP for the Phase 13 `L2Core` slice.

use std::collections::HashMap;
use std::fmt;

use arrow_schema::DataType;

use crate::l2_core::constraints::{ConstraintSet, ConstraintTerm, IntegerType, LoomConstraint};
use crate::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderState,
    ResourceBudget, ScalarExpr, ScalarType, ScalarValue, VerifiedArtifactFacts,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullVerificationCode {
    MissingInputCapability,
    MissingOutputBuilder,
    UnknownVariable,
    OutputTypeMismatch,
    OutputNullabilityMismatch,
    InvalidLoopBounds,
    NonMonotoneCursorLoop,
    ResourceBudgetExceeded,
    ConstraintBudgetExceeded,
    ExplicitFailClosed,
}

impl FullVerificationCode {
    pub fn as_str(self) -> &'static str {
        match self {
            FullVerificationCode::MissingInputCapability => "missing-input-capability",
            FullVerificationCode::MissingOutputBuilder => "missing-output-builder",
            FullVerificationCode::UnknownVariable => "unknown-variable",
            FullVerificationCode::OutputTypeMismatch => "output-type-mismatch",
            FullVerificationCode::OutputNullabilityMismatch => "output-nullability-mismatch",
            FullVerificationCode::InvalidLoopBounds => "invalid-loop-bounds",
            FullVerificationCode::NonMonotoneCursorLoop => "non-monotone-cursor-loop",
            FullVerificationCode::ResourceBudgetExceeded => "resource-budget-exceeded",
            FullVerificationCode::ConstraintBudgetExceeded => "constraint-budget-exceeded",
            FullVerificationCode::ExplicitFailClosed => "explicit-fail-closed",
        }
    }
}

impl fmt::Display for FullVerificationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullVerificationDiagnostic {
    pub code: FullVerificationCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligationTrace {
    pub id: String,
    pub layer: String,
    pub summary: String,
    pub constraint_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbstractState {
    pub input_capabilities: HashMap<String, InputSliceCapability>,
    pub output_builders: HashMap<String, OutputBuilderState>,
    pub scalar_types: HashMap<String, ScalarType>,
    pub resource_budget: ResourceBudget,
    pub steps_used: u64,
    pub builder_events_used: u64,
    pub loop_bounds: Vec<(String, u64)>,
    pub constraints: ConstraintSet,
}

impl AbstractState {
    fn from_program(program: &L2CoreProgram) -> Self {
        let mut input_capabilities = HashMap::new();
        let mut output_builders = HashMap::new();

        for capability in &program.capabilities {
            match capability {
                Capability::InputSlice(input) => {
                    input_capabilities.insert(input.id.clone(), input.clone());
                }
                Capability::OutputBuilder(builder) => {
                    output_builders.insert(
                        builder.id.clone(),
                        OutputBuilderState {
                            builder_id: builder.id.clone(),
                            arrow_type: builder.arrow_type.clone(),
                            nullable: builder.nullable,
                            max_events: builder.max_events,
                            emitted_events: 0,
                        },
                    );
                }
                Capability::Scratch(_) => {}
            }
        }

        Self {
            input_capabilities,
            output_builders,
            scalar_types: HashMap::new(),
            resource_budget: program.resource_budget.clone(),
            steps_used: 0,
            builder_events_used: 0,
            loop_bounds: Vec::new(),
            constraints: ConstraintSet::new(),
        }
    }

    fn constraint_ids(&self) -> Vec<String> {
        self.constraints
            .iter()
            .map(|constraint| constraint.id().to_string())
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FullVerificationReport {
    diagnostics: Vec<FullVerificationDiagnostic>,
    proof_obligations: Vec<ProofObligationTrace>,
    facts: Option<VerifiedArtifactFacts>,
    constraints: ConstraintSet,
    constraint_comments: String,
}

impl FullVerificationReport {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[FullVerificationDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&FullVerificationDiagnostic> {
        self.diagnostics.first()
    }

    pub fn proof_obligations(&self) -> &[ProofObligationTrace] {
        &self.proof_obligations
    }

    pub fn facts(&self) -> Option<&VerifiedArtifactFacts> {
        self.facts.as_ref()
    }

    pub fn constraints(&self) -> &ConstraintSet {
        &self.constraints
    }

    pub fn constraint_comments(&self) -> &str {
        &self.constraint_comments
    }

    fn push(
        &mut self,
        code: FullVerificationCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(FullVerificationDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

pub fn verify_l2_core(program: &L2CoreProgram) -> FullVerificationReport {
    let mut report = FullVerificationReport::default();
    let mut state = AbstractState::from_program(program);

    verify_statements(&program.body, "$.body", &mut state, &mut report);

    if state.constraints.iter().count() as u64 > state.resource_budget.max_constraint_count {
        report.push(
            FullVerificationCode::ConstraintBudgetExceeded,
            "$.resource_budget.max_constraint_count",
            format!(
                "constraint count {} exceeds budget {}",
                state.constraints.iter().count(),
                state.resource_budget.max_constraint_count
            ),
        );
    }

    report.constraints = state.constraints.clone();
    report.constraint_comments = state.constraints.to_smtlib_comments();
    report.proof_obligations = proof_obligations(&state);

    if report.is_ok() {
        let constraint_ids = state.constraint_ids();
        let proof_ids = report
            .proof_obligations
            .iter()
            .map(|obligation| obligation.id.clone())
            .collect();
        // Phase A–C: production verify stays oracle-free.
        // kloom differential evidence lives in CI harnesses (kloom-diff.sh,
        // native_arrow_semantic model-validation tests), not in the
        // production verifier path.
        report.facts = Some(VerifiedArtifactFacts::for_program(
            program,
            constraint_ids,
            proof_ids,
            false,
        ));
    }

    report
}

fn verify_statements(
    body: &[L2CoreStmt],
    path: &str,
    state: &mut AbstractState,
    report: &mut FullVerificationReport,
) {
    for (idx, stmt) in body.iter().enumerate() {
        let stmt_path = format!("{path}[{idx}]");
        state.steps_used = state.steps_used.saturating_add(1);
        if state.steps_used > state.resource_budget.max_steps {
            report.push(
                FullVerificationCode::ResourceBudgetExceeded,
                format!("{stmt_path}.steps"),
                "statement step budget exceeded",
            );
        }

        match stmt {
            L2CoreStmt::ForRange {
                index,
                start,
                end,
                body,
            } => {
                let iterations = const_u64(end)
                    .zip(const_u64(start))
                    .and_then(|(end, start)| {
                        if end >= start {
                            Some(end - start)
                        } else {
                            None
                        }
                    });

                match iterations {
                    Some(count) if count <= state.resource_budget.max_rows => {
                        state.loop_bounds.push((index.clone(), count));
                        state
                            .scalar_types
                            .insert(index.clone(), ScalarType::RowIndex);
                        verify_statements(body, &format!("{stmt_path}.body"), state, report);
                        state.scalar_types.remove(index);
                    }
                    Some(count) => report.push(
                        FullVerificationCode::ResourceBudgetExceeded,
                        format!("{stmt_path}.end"),
                        format!(
                            "ForRange iterations {count} exceed row budget {}",
                            state.resource_budget.max_rows
                        ),
                    ),
                    None => report.push(
                        FullVerificationCode::InvalidLoopBounds,
                        format!("{stmt_path}.end"),
                        "ForRange bounds must be finite constants with end >= start",
                    ),
                }
            }
            L2CoreStmt::CursorLoop {
                cursor,
                limit,
                progress,
                body,
            } => {
                let limit_bound = const_u64(limit);
                if !is_monotone_progress(cursor, progress) {
                    report.push(
                        FullVerificationCode::NonMonotoneCursorLoop,
                        format!("{stmt_path}.progress"),
                        "CursorLoop progress must advance the cursor by a positive constant",
                    );
                } else {
                    push_decreases_constraint(cursor, &stmt_path, state);
                }

                if let Some(limit) = limit_bound {
                    if limit > state.resource_budget.max_rows {
                        report.push(
                            FullVerificationCode::ResourceBudgetExceeded,
                            format!("{stmt_path}.limit"),
                            format!(
                                "CursorLoop limit {limit} exceeds row budget {}",
                                state.resource_budget.max_rows
                            ),
                        );
                    }
                    state.loop_bounds.push((cursor.clone(), limit));
                } else {
                    report.push(
                        FullVerificationCode::InvalidLoopBounds,
                        format!("{stmt_path}.limit"),
                        "CursorLoop limit must be a finite constant",
                    );
                }

                state
                    .scalar_types
                    .insert(cursor.clone(), ScalarType::RowIndex);
                verify_statements(body, &format!("{stmt_path}.body"), state, report);
                state.scalar_types.remove(cursor);
            }
            L2CoreStmt::ReadInput {
                capability,
                offset,
                width,
                bind,
            } => {
                verify_expr(offset, &format!("{stmt_path}.offset"), state, report);
                verify_expr(width, &format!("{stmt_path}.width"), state, report);
                let Some(input) = state.input_capabilities.get(capability).cloned() else {
                    report.push(
                        FullVerificationCode::MissingInputCapability,
                        format!("{stmt_path}.capability"),
                        format!("input capability '{capability}' is not declared"),
                    );
                    continue;
                };
                if let (Some(offset), Some(width)) = (const_u64(offset), const_u64(width)) {
                    if offset < input.offset
                        || offset.saturating_add(width) > input.offset.saturating_add(input.length)
                    {
                        report.push(
                            FullVerificationCode::MissingInputCapability,
                            format!("{stmt_path}.range"),
                            format!(
                                "read offset {offset} width {width} is outside input capability '{capability}' range {}..{}",
                                input.offset,
                                input.offset.saturating_add(input.length)
                            ),
                        );
                        continue;
                    }
                };

                push_read_constraints(capability, &input, offset, width, &stmt_path, state);
                state
                    .scalar_types
                    .insert(bind.clone(), scalar_type_for_read_width(width));
            }
            L2CoreStmt::LetScalar { name, expr } => {
                if let Some(ty) = verify_expr(expr, &format!("{stmt_path}.expr"), state, report) {
                    state.scalar_types.insert(name.clone(), ty);
                }
            }
            L2CoreStmt::AppendValue { builder, value } => {
                let value_type = verify_expr(value, &format!("{stmt_path}.value"), state, report);
                let Some(builder_state) = state.output_builders.get_mut(builder) else {
                    report.push(
                        FullVerificationCode::MissingOutputBuilder,
                        format!("{stmt_path}.builder"),
                        format!("output builder '{builder}' is not declared"),
                    );
                    continue;
                };

                if let Some(value_type) = value_type {
                    let expected = scalar_type_for_arrow(&builder_state.arrow_type);
                    if expected.as_ref() != Some(&value_type) {
                        report.push(
                            FullVerificationCode::OutputTypeMismatch,
                            format!("{stmt_path}.value"),
                            format!(
                                "value type {:?} does not match builder {} type {:?}",
                                value_type, builder, builder_state.arrow_type
                            ),
                        );
                    }
                }

                builder_state.emitted_events = builder_state.emitted_events.saturating_add(1);
                state.builder_events_used = state.builder_events_used.saturating_add(1);
                check_builder_budget(builder, &stmt_path, state, report);
            }
            L2CoreStmt::AppendNull { builder } => {
                let Some(builder_state) = state.output_builders.get_mut(builder) else {
                    report.push(
                        FullVerificationCode::MissingOutputBuilder,
                        format!("{stmt_path}.builder"),
                        format!("output builder '{builder}' is not declared"),
                    );
                    continue;
                };

                if !builder_state.nullable {
                    report.push(
                        FullVerificationCode::OutputNullabilityMismatch,
                        format!("{stmt_path}.builder"),
                        format!("builder '{builder}' is not nullable"),
                    );
                }

                builder_state.emitted_events = builder_state.emitted_events.saturating_add(1);
                state.builder_events_used = state.builder_events_used.saturating_add(1);
                check_builder_budget(builder, &stmt_path, state, report);
            }
            L2CoreStmt::FailClosed { .. } => {
                report.push(
                    FullVerificationCode::ExplicitFailClosed,
                    stmt_path,
                    "explicit FailClosed statements are runtime diagnostics, not accepted verifier programs",
                );
            }
        }
    }
}

fn verify_expr(
    expr: &ScalarExpr,
    path: &str,
    state: &mut AbstractState,
    report: &mut FullVerificationReport,
) -> Option<ScalarType> {
    match expr {
        ScalarExpr::Const(value) => Some(type_of_const(value)),
        ScalarExpr::Var(name) => match state.scalar_types.get(name) {
            Some(ty) => Some(ty.clone()),
            None => {
                report.push(
                    FullVerificationCode::UnknownVariable,
                    path,
                    format!("variable '{name}' is not defined"),
                );
                None
            }
        },
        ScalarExpr::Add(lhs, rhs) | ScalarExpr::Sub(lhs, rhs) | ScalarExpr::Mul(lhs, rhs) => {
            let lhs_ty = verify_expr(lhs, &format!("{path}.lhs"), state, report);
            let rhs_ty = verify_expr(rhs, &format!("{path}.rhs"), state, report);
            if matches!(expr, ScalarExpr::Add(_, _) | ScalarExpr::Mul(_, _)) {
                push_overflow_constraint(expr, path, state);
            }
            lhs_ty.or(rhs_ty)
        }
        ScalarExpr::Min(lhs, rhs) | ScalarExpr::Max(lhs, rhs) => {
            let lhs_ty = verify_expr(lhs, &format!("{path}.lhs"), state, report);
            let rhs_ty = verify_expr(rhs, &format!("{path}.rhs"), state, report);
            lhs_ty.or(rhs_ty)
        }
        ScalarExpr::Eq(lhs, rhs) | ScalarExpr::Lt(lhs, rhs) | ScalarExpr::Le(lhs, rhs) => {
            verify_expr(lhs, &format!("{path}.lhs"), state, report);
            verify_expr(rhs, &format!("{path}.rhs"), state, report);
            Some(ScalarType::Bool)
        }
    }
}

fn check_builder_budget(
    builder: &str,
    path: &str,
    state: &mut AbstractState,
    report: &mut FullVerificationReport,
) {
    if state.builder_events_used > state.resource_budget.max_builder_events {
        report.push(
            FullVerificationCode::ResourceBudgetExceeded,
            format!("{path}.builder"),
            format!(
                "builder events {} exceed resource budget {}",
                state.builder_events_used, state.resource_budget.max_builder_events
            ),
        );
    }
    if let Some(builder_state) = state.output_builders.get(builder) {
        if builder_state.emitted_events > builder_state.max_events {
            report.push(
                FullVerificationCode::ResourceBudgetExceeded,
                format!("{path}.builder"),
                format!(
                    "builder '{builder}' events {} exceed builder budget {}",
                    builder_state.emitted_events, builder_state.max_events
                ),
            );
        }
    }
}

fn push_read_constraints(
    capability_id: &str,
    input: &InputSliceCapability,
    offset: &ScalarExpr,
    width: &ScalarExpr,
    path: &str,
    state: &mut AbstractState,
) {
    let offset_term = constraint_term(offset);
    let width_term = constraint_term(width);
    let end_term = ConstraintTerm::Add(Box::new(offset_term.clone()), Box::new(width_term.clone()));
    let constraint_prefix = sanitize_path(path);

    state.constraints.push(LoomConstraint::AddNoOverflow {
        id: format!("{constraint_prefix}.read-add-no-overflow"),
        left: offset_term.clone(),
        right: width_term,
        ty: IntegerType::UInt64,
    });
    state.constraints.push(LoomConstraint::InRange {
        id: format!("{constraint_prefix}.read-in-range"),
        value: end_term,
        lower: ConstraintTerm::int(input.offset as i128),
        upper_exclusive: ConstraintTerm::int((input.offset + input.length + 1) as i128),
    });
    state.constraints.push(LoomConstraint::FeatureImplies {
        id: format!("{constraint_prefix}.capability-declared"),
        feature: format!("input-capability:{capability_id}"),
        obligation_id: "VERIFIER-04".to_string(),
    });
}

fn push_decreases_constraint(cursor: &str, path: &str, state: &mut AbstractState) {
    let prefix = sanitize_path(path);
    state.constraints.push(LoomConstraint::Decreases {
        id: format!("{prefix}.cursor-decreases"),
        previous: ConstraintTerm::var(format!("{cursor}.remaining_before")),
        next: ConstraintTerm::var(format!("{cursor}.remaining_after")),
    });
}

fn push_overflow_constraint(expr: &ScalarExpr, path: &str, state: &mut AbstractState) {
    let prefix = sanitize_path(path);
    match expr {
        ScalarExpr::Add(lhs, rhs) => state.constraints.push(LoomConstraint::AddNoOverflow {
            id: format!("{prefix}.add-no-overflow"),
            left: constraint_term(lhs),
            right: constraint_term(rhs),
            ty: IntegerType::UInt64,
        }),
        ScalarExpr::Mul(lhs, rhs) => state.constraints.push(LoomConstraint::MulNoOverflow {
            id: format!("{prefix}.mul-no-overflow"),
            left: constraint_term(lhs),
            right: constraint_term(rhs),
            ty: IntegerType::UInt64,
        }),
        _ => {}
    }
}

fn proof_obligations(state: &AbstractState) -> Vec<ProofObligationTrace> {
    let constraint_ids = state.constraint_ids();
    vec![
        ProofObligationTrace {
            id: "VERIFIER-04".to_string(),
            layer: "Rust".to_string(),
            summary: "capability and resource checks executed by verify_l2_core".to_string(),
            constraint_ids: constraint_ids.clone(),
        },
        ProofObligationTrace {
            id: "VERIFIER-06".to_string(),
            layer: "Rust".to_string(),
            summary: "type/effect and abstract-state walk completed".to_string(),
            constraint_ids: constraint_ids.clone(),
        },
        ProofObligationTrace {
            id: "VERIFIER-07".to_string(),
            layer: "SMT".to_string(),
            summary: "local arithmetic/range/progress obligations emitted".to_string(),
            constraint_ids: constraint_ids.clone(),
        },
        ProofObligationTrace {
            id: "VERIFIER-08".to_string(),
            layer: "Rust".to_string(),
            summary: "stable diagnostics and proof-obligation traces emitted".to_string(),
            constraint_ids: constraint_ids.clone(),
        },
        ProofObligationTrace {
            id: "VERIFIER-10".to_string(),
            layer: "Gate".to_string(),
            summary: "accepted programs can emit VerifiedArtifactFacts".to_string(),
            constraint_ids,
        },
    ]
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
        DataType::Float32 => Some(ScalarType::Float32),
        DataType::Float64 => Some(ScalarType::Float64),
        DataType::Utf8 => Some(ScalarType::Bytes),
        _ => None,
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

fn is_monotone_progress(cursor: &str, expr: &ScalarExpr) -> bool {
    match expr {
        ScalarExpr::Add(lhs, rhs) => {
            matches!(lhs.as_ref(), ScalarExpr::Var(name) if name == cursor)
                && const_u64(rhs).is_some_and(|value| value > 0)
        }
        _ => false,
    }
}

fn constraint_term(expr: &ScalarExpr) -> ConstraintTerm {
    match expr {
        ScalarExpr::Const(ScalarValue::UInt64(value)) => ConstraintTerm::int((*value).into()),
        ScalarExpr::Const(ScalarValue::UInt32(value)) => ConstraintTerm::int((*value).into()),
        ScalarExpr::Const(ScalarValue::Int64(value)) => ConstraintTerm::int((*value).into()),
        ScalarExpr::Const(ScalarValue::Int32(value)) => ConstraintTerm::int((*value).into()),
        ScalarExpr::Const(ScalarValue::Bool(value)) => {
            ConstraintTerm::int(if *value { 1 } else { 0 })
        }
        ScalarExpr::Const(ScalarValue::Float32Bits(value)) => ConstraintTerm::int((*value).into()),
        ScalarExpr::Const(ScalarValue::Float64Bits(value)) => ConstraintTerm::int((*value).into()),
        ScalarExpr::Const(ScalarValue::Bytes(_)) => ConstraintTerm::var("<bytes>"),
        ScalarExpr::Var(name) => ConstraintTerm::var(name),
        ScalarExpr::Add(lhs, rhs) => ConstraintTerm::Add(
            Box::new(constraint_term(lhs)),
            Box::new(constraint_term(rhs)),
        ),
        ScalarExpr::Sub(lhs, rhs) => ConstraintTerm::Sub(
            Box::new(constraint_term(lhs)),
            Box::new(constraint_term(rhs)),
        ),
        ScalarExpr::Mul(lhs, rhs) => ConstraintTerm::Mul(
            Box::new(constraint_term(lhs)),
            Box::new(constraint_term(rhs)),
        ),
        ScalarExpr::Min(_, _)
        | ScalarExpr::Max(_, _)
        | ScalarExpr::Eq(_, _)
        | ScalarExpr::Lt(_, _)
        | ScalarExpr::Le(_, _) => ConstraintTerm::var("<expr>"),
    }
}

fn sanitize_path(path: &str) -> String {
    path.chars()
        .map(|ch| match ch {
            '$' => "root".to_string(),
            '[' | ']' | '.' => "_".to_string(),
            other if other.is_ascii_alphanumeric() || other == '_' || other == '-' => {
                other.to_string()
            }
            _ => "_".to_string(),
        })
        .collect()
}
