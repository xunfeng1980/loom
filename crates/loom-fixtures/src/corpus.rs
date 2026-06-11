//! Equivalence-class corpus generator for L2Core programs (Phase 48 P4).
//!
//! Generates a large, deterministic set of L2Core programs that vary across
//! multiple dimensions (schema shape, expression complexity, statement mix).
//! The corpus is intended for stress-testing the native-route pre-check and
//! disable-registry logic, as well as serving as a seed pool for differential
//! fuzzing against the K oracle.
//!
//! # Usage
//!
//! ```rust,no_run
//! use loom_fixtures::corpus::CorpusBuilder;
//!
//! let programs = CorpusBuilder::default()
//!     .schema_variants(10)
//!     .expr_depths(0..=3)
//!     .build();
//! ```

use loom_core::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, L2DataType, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, ScalarType, ScalarValue,
};

/// Builder for an equivalence-class corpus.
#[derive(Debug, Clone)]
pub struct CorpusBuilder {
    /// How many distinct schema shapes to generate.
    pub schema_variants: usize,
    /// Range of expression nesting depths to explore.
    pub expr_depth_range: std::ops::RangeInclusive<usize>,
    /// Whether to include nullable columns.
    pub include_nullable: bool,
    /// Whether to include Min/Max expressions (Phase 48 P1).
    pub include_min_max: bool,
    /// Whether to include ForRange loops.
    pub include_for_range: bool,
    /// Whether to include CursorLoop.
    pub include_cursor_loop: bool,
    /// Whether to include ReadInput statements.
    pub include_read_input: bool,
}

impl Default for CorpusBuilder {
    fn default() -> Self {
        Self {
            schema_variants: 20,
            expr_depth_range: 0..=2,
            include_nullable: true,
            include_min_max: true,
            include_for_range: true,
            include_cursor_loop: true,
            include_read_input: false, // ReadInput requires input capabilities
        }
    }
}

impl CorpusBuilder {
    pub fn schema_variants(mut self, n: usize) -> Self {
        self.schema_variants = n;
        self
    }

    pub fn expr_depths(mut self, range: std::ops::RangeInclusive<usize>) -> Self {
        self.expr_depth_range = range;
        self
    }

    pub fn with_nullable(mut self, yes: bool) -> Self {
        self.include_nullable = yes;
        self
    }

    pub fn with_min_max(mut self, yes: bool) -> Self {
        self.include_min_max = yes;
        self
    }

    pub fn with_for_range(mut self, yes: bool) -> Self {
        self.include_for_range = yes;
        self
    }

    pub fn with_cursor_loop(mut self, yes: bool) -> Self {
        self.include_cursor_loop = yes;
        self
    }

    pub fn with_read_input(mut self, yes: bool) -> Self {
        self.include_read_input = yes;
        self
    }

    /// Build the full corpus.
    pub fn build(&self) -> Vec<L2CoreProgram> {
        let mut corpus = Vec::new();
        for schema_idx in 0..self.schema_variants {
            let schema = mk_schema(schema_idx, self.include_nullable);
            for depth in self.expr_depth_range.clone() {
                // Simple append-value program.
                corpus.push(mk_append_program(&schema, depth, self));

                if self.include_for_range {
                    corpus.push(mk_for_range_program(&schema, depth, self));
                }

                if self.include_cursor_loop {
                    corpus.push(mk_cursor_loop_program(&schema, depth, self));
                }
            }
        }
        corpus
    }
}

// ---------------------------------------------------------------------------
// Schema generation
// ---------------------------------------------------------------------------

fn mk_schema(idx: usize, nullable: bool) -> Vec<(String, ScalarType, bool)> {
    let base_types: Vec<ScalarType> = vec![
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::UInt32,
        ScalarType::UInt64,
        ScalarType::Float32,
        ScalarType::Float64,
    ];

    let mut cols = Vec::new();
    let col_count = 1 + (idx % 4); // 1..=4 columns
    for c in 0..col_count {
        let ty = base_types[(idx + c) % base_types.len()].clone();
        let is_nullable = nullable && ((idx + c) % 2 == 0);
        cols.push((format!("col{}", c), ty, is_nullable));
    }
    cols
}

// ---------------------------------------------------------------------------
// Expression generation
// ---------------------------------------------------------------------------

fn mk_expr(depth: usize, ty: &ScalarType, builder: &CorpusBuilder) -> ScalarExpr {
    if depth == 0 {
        return mk_leaf_expr(ty.clone());
    }

    let mut ops: Vec<fn(Box<ScalarExpr>, Box<ScalarExpr>) -> ScalarExpr> = vec![
        ScalarExpr::Add,
        ScalarExpr::Sub,
        ScalarExpr::Mul,
    ];
    if builder.include_min_max {
        ops.push(ScalarExpr::Min);
        ops.push(ScalarExpr::Max);
    }

    let op_idx = (depth * 7 + ty.discriminant()) % ops.len();
    let lhs = mk_expr(depth - 1, ty, builder);
    let rhs = mk_expr(depth - 1, ty, builder);
    ops[op_idx](Box::new(lhs), Box::new(rhs))
}

fn mk_leaf_expr(ty: ScalarType) -> ScalarExpr {
    ScalarExpr::Const(match ty {
        ScalarType::Int32 => ScalarValue::Int32(42),
        ScalarType::Int64 => ScalarValue::Int64(42),
        ScalarType::UInt32 => ScalarValue::UInt32(42),
        ScalarType::UInt64 => ScalarValue::UInt64(42),
        ScalarType::Float32 => ScalarValue::Float32Bits(3.14f32.to_bits()),
        ScalarType::Float64 => ScalarValue::Float64Bits(3.14f64.to_bits()),
        ScalarType::Bool => ScalarValue::Bool(true),
        ScalarType::Bytes => ScalarValue::Bytes(vec![0xAB, 0xCD]),
        ScalarType::RowIndex => ScalarValue::UInt64(0),
    })
}

// ---------------------------------------------------------------------------
// Program templates
// ---------------------------------------------------------------------------

fn mk_append_program(
    schema: &[(String, ScalarType, bool)],
    depth: usize,
    builder: &CorpusBuilder,
) -> L2CoreProgram {
    let (_, first_ty, _) = schema.first().cloned().unwrap_or(("out".to_string(), ScalarType::Int32, false));
    let body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: mk_expr(depth, &first_ty, builder),
    }];
    mk_program(schema, body)
}

fn mk_for_range_program(
    schema: &[(String, ScalarType, bool)],
    depth: usize,
    builder: &CorpusBuilder,
) -> L2CoreProgram {
    let (_, first_ty, _) = schema.first().cloned().unwrap_or(("out".to_string(), ScalarType::Int32, false));
    let body = vec![L2CoreStmt::ForRange {
        index: "i".to_string(),
        start: ScalarExpr::Const(ScalarValue::UInt64(0)),
        end: ScalarExpr::Const(ScalarValue::UInt64(4)),
        body: vec![L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: mk_expr(depth, &first_ty, builder),
        }],
    }];
    mk_program(schema, body)
}

fn mk_cursor_loop_program(
    schema: &[(String, ScalarType, bool)],
    depth: usize,
    builder: &CorpusBuilder,
) -> L2CoreProgram {
    let (_, first_ty, _) = schema.first().cloned().unwrap_or(("out".to_string(), ScalarType::Int32, false));
    let body = vec![L2CoreStmt::CursorLoop {
        cursor: "c".to_string(),
        limit: ScalarExpr::Const(ScalarValue::UInt64(4)),
        progress: ScalarExpr::Add(
            Box::new(ScalarExpr::Var("c".to_string())),
            Box::new(ScalarExpr::Const(ScalarValue::UInt64(1))),
        ),
        body: vec![L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: mk_expr(depth, &first_ty, builder),
        }],
    }];
    mk_program(schema, body)
}

fn mk_program(schema: &[(String, ScalarType, bool)], body: Vec<L2CoreStmt>) -> L2CoreProgram {
    let max_events = body_max_events(&body);
    let capabilities: Vec<Capability> = schema
        .iter()
        .enumerate()
        .map(|(idx, (_name, ty, nullable))| {
            Capability::OutputBuilder(OutputBuilderCapability {
                id: format!("out{}", idx),
                arrow_type: scalar_type_to_l2(ty),
                nullable: *nullable,
                max_events,
            })
        })
        .collect();

    L2CoreProgram {
        artifact_version: 1,
        required_features: vec![],
        optional_features: vec![],
        capabilities,
        resource_budget: ResourceBudget {
            max_steps: 1000,
            max_input_bytes_read: 0,
            max_scratch_bytes: 0,
            max_builder_events: max_events,
            max_rows: max_events,
            max_constraint_count: 0,
        },
        body,
    }
}

fn body_max_events(stmts: &[L2CoreStmt]) -> u64 {
    stmts
        .iter()
        .map(|s| match s {
            L2CoreStmt::AppendValue { .. } => 1,
            L2CoreStmt::ForRange { end, body, .. } => {
                let iterations = const_u64_or(end, 1);
                iterations * body_max_events(body)
            }
            L2CoreStmt::CursorLoop { limit, body, .. } => {
                let iterations = const_u64_or(limit, 1);
                iterations * body_max_events(body)
            }
            L2CoreStmt::ReadInput { .. } => 0,
            L2CoreStmt::LetScalar { .. } => 0,
            L2CoreStmt::AppendNull { .. } => 1,
            L2CoreStmt::FailClosed { .. } => 0,
        })
        .sum()
}

fn const_u64_or(expr: &ScalarExpr, default: u64) -> u64 {
    match expr {
        ScalarExpr::Const(ScalarValue::UInt64(v)) => *v,
        ScalarExpr::Const(ScalarValue::Int64(v)) => *v as u64,
        _ => default,
    }
}

fn scalar_type_to_arrow(ty: &ScalarType) -> arrow_schema::DataType {
    match ty {
        ScalarType::Int32 => arrow_schema::DataType::Int32,
        ScalarType::Int64 => arrow_schema::DataType::Int64,
        ScalarType::UInt32 => arrow_schema::DataType::UInt32,
        ScalarType::UInt64 => arrow_schema::DataType::UInt64,
        ScalarType::Float32 => arrow_schema::DataType::Float32,
        ScalarType::Float64 => arrow_schema::DataType::Float64,
        ScalarType::Bool => arrow_schema::DataType::Boolean,
        ScalarType::Bytes => arrow_schema::DataType::Binary,
        ScalarType::RowIndex => arrow_schema::DataType::UInt64,
    }
}

fn scalar_type_to_l2(ty: &ScalarType) -> L2DataType {
    match ty {
        ScalarType::Int32 => L2DataType::Int32,
        ScalarType::Int64 => L2DataType::Int64,
        ScalarType::UInt32 => L2DataType::Int32,
        ScalarType::UInt64 => L2DataType::Int64,
        ScalarType::Float32 => L2DataType::Float32,
        ScalarType::Float64 => L2DataType::Float64,
        ScalarType::Bool => L2DataType::Boolean,
        ScalarType::Bytes => L2DataType::Utf8,
        ScalarType::RowIndex => L2DataType::Int64,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

trait Discriminant {
    fn discriminant(&self) -> usize;
}

impl Discriminant for ScalarType {
    fn discriminant(&self) -> usize {
        match self {
            ScalarType::Int32 => 0,
            ScalarType::Int64 => 1,
            ScalarType::UInt32 => 2,
            ScalarType::UInt64 => 3,
            ScalarType::Float32 => 4,
            ScalarType::Float64 => 5,
            ScalarType::Bool => 6,
            ScalarType::Bytes => 7,
            ScalarType::RowIndex => 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_produces_non_empty_corpus() {
        let corpus = CorpusBuilder::default().build();
        assert!(!corpus.is_empty(), "default corpus must not be empty");
    }

    #[test]
    fn corpus_programs_have_valid_budgets() {
        let corpus = CorpusBuilder::default().build();
        for (idx, prog) in corpus.iter().enumerate() {
            assert!(
                prog.resource_budget.max_builder_events > 0,
                "program {idx} must have positive max_builder_events"
            );
            assert!(
                prog.resource_budget.max_rows > 0,
                "program {idx} must have positive max_rows"
            );
        }
    }

    #[test]
    fn corpus_includes_min_max_when_enabled() {
        let corpus = CorpusBuilder::default()
            .with_min_max(true)
            .expr_depths(1..=1)
            .build();
        let has_min = corpus.iter().any(|p| expr_uses_op(&p.body, is_min));
        let has_max = corpus.iter().any(|p| expr_uses_op(&p.body, is_max));
        assert!(has_min, "corpus with min_max enabled must include Min");
        assert!(has_max, "corpus with min_max enabled must include Max");
    }

    #[test]
    fn corpus_excludes_min_max_when_disabled() {
        let corpus = CorpusBuilder::default()
            .with_min_max(false)
            .expr_depths(1..=2)
            .build();
        let has_min = corpus.iter().any(|p| expr_uses_op(&p.body, is_min));
        let has_max = corpus.iter().any(|p| expr_uses_op(&p.body, is_max));
        assert!(!has_min, "corpus with min_max disabled must not include Min");
        assert!(!has_max, "corpus with min_max disabled must not include Max");
    }

    fn expr_uses_op(stmts: &[L2CoreStmt], pred: fn(&ScalarExpr) -> bool) -> bool {
        stmts.iter().any(|s| stmt_uses_op(s, pred))
    }

    fn stmt_uses_op(stmt: &L2CoreStmt, pred: fn(&ScalarExpr) -> bool) -> bool {
        match stmt {
            L2CoreStmt::AppendValue { value, .. } => expr_has_op(value, pred),
            L2CoreStmt::ForRange { body, .. } => expr_uses_op(body, pred),
            L2CoreStmt::CursorLoop { body, .. } => expr_uses_op(body, pred),
            L2CoreStmt::LetScalar { expr, .. } => expr_has_op(expr, pred),
            L2CoreStmt::ReadInput { .. } => false,
            L2CoreStmt::AppendNull { .. } => false,
            L2CoreStmt::FailClosed { .. } => false,
        }
    }

    fn expr_has_op(expr: &ScalarExpr, pred: fn(&ScalarExpr) -> bool) -> bool {
        if pred(expr) {
            return true;
        }
        match expr {
            ScalarExpr::Const(_) | ScalarExpr::Var(_) => false,
            ScalarExpr::Add(l, r) | ScalarExpr::Sub(l, r) | ScalarExpr::Mul(l, r)
            | ScalarExpr::Min(l, r) | ScalarExpr::Max(l, r)
            | ScalarExpr::Eq(l, r) | ScalarExpr::Lt(l, r) | ScalarExpr::Le(l, r) => {
                expr_has_op(l, pred) || expr_has_op(r, pred)
            }
        }
    }

    fn is_min(expr: &ScalarExpr) -> bool {
        matches!(expr, ScalarExpr::Min(_, _))
    }

    fn is_max(expr: &ScalarExpr) -> bool {
        matches!(expr, ScalarExpr::Max(_, _))
    }
}
