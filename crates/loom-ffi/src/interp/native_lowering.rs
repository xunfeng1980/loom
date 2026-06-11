//! Verifier-gated native-lowering support checks.
//!
//! Phase 14 starts with a deliberately tiny support predicate. It accepts only
//! the Phase 13 bounded Int32 copy slice after `verify_l2_core` has accepted the
//! program and emitted `VerifiedArtifactFacts`. Textual MLIR emission is added
//! later; unsupported programs fail closed here before any lowering artifact can
//! exist.

use std::fmt;

use loom_ir_core::l2_core::L2DataType;

use loom_ir_core::full_verifier::FullVerificationReport;
use loom_ir_core::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability, ScalarExpr, ScalarValue,
};

const SUPPORTED_FEATURE: &str = "l2core.copy.v0";
const ENTRY_SYMBOL: &str = "loom_l2core_copy_i32";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringDiagnosticCode {
    VerifierRejected,
    MissingVerifierFacts,
    UnsupportedFeature,
    UnsupportedStatement,
    UnsupportedType,
    UnsupportedNullability,
    UnsupportedLoopShape,
    UnsupportedCapabilityShape,
    UnsupportedExpressionShape,
}

impl LoweringDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            LoweringDiagnosticCode::VerifierRejected => "verifier-rejected",
            LoweringDiagnosticCode::MissingVerifierFacts => "missing-verifier-facts",
            LoweringDiagnosticCode::UnsupportedFeature => "unsupported-feature",
            LoweringDiagnosticCode::UnsupportedStatement => "unsupported-statement",
            LoweringDiagnosticCode::UnsupportedType => "unsupported-type",
            LoweringDiagnosticCode::UnsupportedNullability => "unsupported-nullability",
            LoweringDiagnosticCode::UnsupportedLoopShape => "unsupported-loop-shape",
            LoweringDiagnosticCode::UnsupportedCapabilityShape => "unsupported-capability-shape",
            LoweringDiagnosticCode::UnsupportedExpressionShape => "unsupported-expression-shape",
        }
    }
}

impl fmt::Display for LoweringDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringDiagnostic {
    pub code: LoweringDiagnosticCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupportedCopySlice {
    pub input_id: String,
    pub output_builder_id: String,
    pub row_count: u64,
    pub loop_index: String,
    pub read_bind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoweringSupportReport {
    diagnostics: Vec<LoweringDiagnostic>,
    supported_copy: Option<SupportedCopySlice>,
}

impl LoweringSupportReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && self.supported_copy.is_some()
    }

    pub fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<&LoweringDiagnostic> {
        self.diagnostics.first()
    }

    pub fn supported_copy(&self) -> Option<&SupportedCopySlice> {
        self.supported_copy.as_ref()
    }

    fn push(
        &mut self,
        code: LoweringDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(LoweringDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringBackend {
    TextualMlir,
}

impl LoweringBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            LoweringBackend::TextualMlir => "textual-mlir",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringArtifact {
    pub backend: LoweringBackend,
    pub entry_symbol: String,
    pub mlir_text: String,
    pub facts_linkage: String,
    pub row_count: u64,
}

pub fn lower_to_textual_mlir(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
) -> Result<LoweringArtifact, LoweringSupportReport> {
    let support = check_lowering_support(program, report);
    if !support.is_supported() {
        return Err(support);
    }

    let copy = support
        .supported_copy()
        .expect("support report is supported only when the copy slice exists");
    let facts = report
        .facts()
        .expect("support checking requires facts before emission");

    Ok(LoweringArtifact {
        backend: LoweringBackend::TextualMlir,
        entry_symbol: ENTRY_SYMBOL.to_string(),
        mlir_text: textual_mlir(copy),
        facts_linkage: format!(
            "artifact_version={};features={};constraints={};proofs={};rows={}",
            facts.artifact_version,
            facts.accepted_feature_set.join(","),
            facts.constraint_ids.len(),
            facts.proof_obligation_ids.join(","),
            copy.row_count
        ),
        row_count: copy.row_count,
    })
}

/// Execute the tiny supported copy slice as typed primitive regression evidence.
///
/// This helper is intentionally not a general `L2Core` interpreter. It exists so
/// Phase 14 can compare the verifier-gated textual lowering shape against the
/// same bounded Int32 copy semantics without generating Arrow buffers directly.
pub fn execute_supported_copy_i32(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
    input: &[i32],
) -> Result<Vec<i32>, LoweringSupportReport> {
    let support = check_lowering_support(program, report);
    if !support.is_supported() {
        return Err(support);
    }

    let copy = support
        .supported_copy()
        .expect("support report is supported only when the copy slice exists");
    let row_count_bound = report
        .facts()
        .and_then(|facts| facts.row_count_bound)
        .expect("support checking requires row_count_bound before execution");
    let row_count = row_count_bound as usize;

    if input.len() < row_count {
        let mut rejected = LoweringSupportReport::default();
        rejected.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.input",
            format!(
                "input has {} Int32 values but row_count_bound requires {}",
                input.len(),
                row_count
            ),
        );
        return Err(rejected);
    }

    Ok(input[..copy.row_count as usize].to_vec())
}

pub fn check_lowering_support(
    program: &L2CoreProgram,
    report: &FullVerificationReport,
) -> LoweringSupportReport {
    let mut support = LoweringSupportReport::default();

    if !report.is_ok() {
        support.push(
            LoweringDiagnosticCode::VerifierRejected,
            "$.verification",
            "L2Core verifier rejected the program",
        );
        return support;
    }

    let Some(facts) = report.facts() else {
        support.push(
            LoweringDiagnosticCode::MissingVerifierFacts,
            "$.verification.facts",
            "accepted lowering requires verifier facts from the same report",
        );
        return support;
    };

    if program.required_features != [SUPPORTED_FEATURE] {
        support.push(
            LoweringDiagnosticCode::UnsupportedFeature,
            "$.required_features",
            format!(
                "Phase 14 only lowers required feature '{}'",
                SUPPORTED_FEATURE
            ),
        );
    }
    if !program.optional_features.is_empty() {
        support.push(
            LoweringDiagnosticCode::UnsupportedFeature,
            "$.optional_features",
            "Phase 14 textual lowering accepts no optional features",
        );
    }
    if facts.accepted_feature_set != [SUPPORTED_FEATURE] {
        support.push(
            LoweringDiagnosticCode::UnsupportedFeature,
            "$.facts.accepted_feature_set",
            "verifier facts do not match the Phase 14 supported feature",
        );
    }

    let Some(row_count) = facts.row_count_bound else {
        support.push(
            LoweringDiagnosticCode::UnsupportedLoopShape,
            "$.facts.row_count_bound",
            "Phase 14 lowering requires a finite row-count bound",
        );
        return support;
    };

    let Some((input_id, output_builder)) = check_capabilities(program, &mut support) else {
        return support;
    };
    check_facts_match(&input_id, output_builder, row_count, facts, &mut support);

    let supported_copy = check_body(
        program,
        &input_id,
        &output_builder.id,
        row_count,
        &mut support,
    );

    if support.diagnostics.is_empty() {
        support.supported_copy = supported_copy;
    }

    support
}

fn check_capabilities<'a>(
    program: &'a L2CoreProgram,
    support: &mut LoweringSupportReport,
) -> Option<(String, &'a OutputBuilderCapability)> {
    let mut input_id = None;
    let mut output_builder = None;

    for (idx, capability) in program.capabilities.iter().enumerate() {
        match capability {
            Capability::InputSlice(input) => {
                if input_id.replace(input.id.clone()).is_some() {
                    support.push(
                        LoweringDiagnosticCode::UnsupportedCapabilityShape,
                        format!("$.capabilities[{idx}]"),
                        "Phase 14 lowering accepts exactly one input slice",
                    );
                }
            }
            Capability::OutputBuilder(builder) => {
                if output_builder.replace(builder).is_some() {
                    support.push(
                        LoweringDiagnosticCode::UnsupportedCapabilityShape,
                        format!("$.capabilities[{idx}]"),
                        "Phase 14 lowering accepts exactly one output builder",
                    );
                }
            }
            Capability::Scratch(_) => support.push(
                LoweringDiagnosticCode::UnsupportedCapabilityShape,
                format!("$.capabilities[{idx}]"),
                "Phase 14 lowering does not support scratch capabilities",
            ),
        }
    }

    let Some(input_id) = input_id else {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.capabilities",
            "Phase 14 lowering requires one input slice",
        );
        return None;
    };
    let Some(output_builder) = output_builder else {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.capabilities",
            "Phase 14 lowering requires one output builder",
        );
        return None;
    };

    if output_builder.arrow_type != L2DataType::Int32 {
        support.push(
            LoweringDiagnosticCode::UnsupportedType,
            "$.capabilities.output_builder.arrow_type",
            format!(
                "Phase 14 lowering only supports Int32 output, got {:?}",
                output_builder.arrow_type
            ),
        );
    }

    Some((input_id, output_builder))
}

fn check_facts_match(
    input_id: &str,
    output_builder: &OutputBuilderCapability,
    row_count: u64,
    facts: &loom_ir_core::l2_core::VerifiedArtifactFacts,
    support: &mut LoweringSupportReport,
) {
    if facts.input_ranges.len() != 1 || facts.input_ranges[0].capability_id != input_id {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.facts.input_ranges",
            "verifier facts must describe exactly the supported input slice",
        );
    }
    if facts.output_schema.len() != 1
        || facts.output_schema[0].builder_id != output_builder.id
        || facts.output_schema[0].arrow_type != L2DataType::Int32
    {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.facts.output_schema",
            "verifier facts must describe exactly the supported Int32 output builder",
        );
    }
    if output_builder.max_events != row_count {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.capabilities.output_builder.max_events",
            format!(
                "output builder max_events {} must equal row bound {}",
                output_builder.max_events, row_count
            ),
        );
    }
}

fn check_body(
    program: &L2CoreProgram,
    input_id: &str,
    output_builder_id: &str,
    row_count: u64,
    support: &mut LoweringSupportReport,
) -> Option<SupportedCopySlice> {
    if program.body.len() != 1 {
        support.push(
            LoweringDiagnosticCode::UnsupportedStatement,
            "$.body",
            "Phase 14 lowering expects exactly one top-level ForRange",
        );
        return None;
    }

    let L2CoreStmt::ForRange {
        index,
        start,
        end,
        body,
    } = &program.body[0]
    else {
        let code = match &program.body[0] {
            L2CoreStmt::CursorLoop { .. } => LoweringDiagnosticCode::UnsupportedLoopShape,
            L2CoreStmt::AppendNull { .. } => LoweringDiagnosticCode::UnsupportedNullability,
            _ => LoweringDiagnosticCode::UnsupportedStatement,
        };
        support.push(
            code,
            "$.body[0]",
            "Phase 14 lowering expects one top-level ForRange",
        );
        return None;
    };

    if const_u64(start) != Some(0) || const_u64(end) != Some(row_count) {
        support.push(
            LoweringDiagnosticCode::UnsupportedLoopShape,
            "$.body[0]",
            "ForRange bounds must be constant 0..row_count_bound",
        );
    }

    if body.len() != 2 {
        support.push(
            LoweringDiagnosticCode::UnsupportedStatement,
            "$.body[0].body",
            "ForRange body must be exactly ReadInput followed by AppendValue",
        );
        return None;
    }

    let L2CoreStmt::ReadInput {
        capability,
        offset,
        width,
        bind,
    } = &body[0]
    else {
        support.push(
            statement_code(&body[0]),
            "$.body[0].body[0]",
            "first lowered statement must be ReadInput",
        );
        return None;
    };

    if capability != input_id {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.body[0].body[0].capability",
            "ReadInput must use the supported input capability",
        );
    }
    if const_u64(width) != Some(4) {
        support.push(
            LoweringDiagnosticCode::UnsupportedExpressionShape,
            "$.body[0].body[0].width",
            "ReadInput width must be the supported Int32 width of 4 bytes",
        );
    }
    if !is_index_plus_zero(offset, index) {
        support.push(
            LoweringDiagnosticCode::UnsupportedExpressionShape,
            "$.body[0].body[0].offset",
            "ReadInput offset must be index + 0",
        );
    }

    let L2CoreStmt::AppendValue { builder, value } = &body[1] else {
        support.push(
            statement_code(&body[1]),
            "$.body[0].body[1]",
            "second lowered statement must be AppendValue",
        );
        return None;
    };

    if builder != output_builder_id {
        support.push(
            LoweringDiagnosticCode::UnsupportedCapabilityShape,
            "$.body[0].body[1].builder",
            "AppendValue must use the supported output builder",
        );
    }
    if !matches!(value, ScalarExpr::Var(name) if name == bind) {
        support.push(
            LoweringDiagnosticCode::UnsupportedExpressionShape,
            "$.body[0].body[1].value",
            "AppendValue must append the value bound by ReadInput",
        );
    }

    Some(SupportedCopySlice {
        input_id: input_id.to_string(),
        output_builder_id: output_builder_id.to_string(),
        row_count,
        loop_index: index.clone(),
        read_bind: bind.clone(),
    })
}

fn statement_code(stmt: &L2CoreStmt) -> LoweringDiagnosticCode {
    match stmt {
        L2CoreStmt::CursorLoop { .. } => LoweringDiagnosticCode::UnsupportedLoopShape,
        L2CoreStmt::AppendNull { .. } => LoweringDiagnosticCode::UnsupportedNullability,
        _ => LoweringDiagnosticCode::UnsupportedStatement,
    }
}

fn const_u64(expr: &ScalarExpr) -> Option<u64> {
    match expr {
        ScalarExpr::Const(ScalarValue::UInt64(value)) => Some(*value),
        _ => None,
    }
}

fn is_index_plus_zero(expr: &ScalarExpr, index: &str) -> bool {
    match expr {
        ScalarExpr::Var(name) => name == index,
        ScalarExpr::Add(lhs, rhs) => {
            matches!(lhs.as_ref(), ScalarExpr::Var(name) if name == index)
                && const_u64(rhs.as_ref()) == Some(0)
        }
        _ => false,
    }
}

fn textual_mlir(_copy: &SupportedCopySlice) -> String {
    format!(
        "module {{\n  func.func @{ENTRY_SYMBOL}(%input: memref<?xi32>, %output: memref<?xi32>, %rows: index) {{\n    %c0 = arith.constant 0 : index\n    %c1 = arith.constant 1 : index\n    scf.for %i = %c0 to %rows step %c1 {{\n      %v = memref.load %input[%i] : memref<?xi32>\n      memref.store %v, %output[%i] : memref<?xi32>\n    }}\n    return\n  }}\n}}\n"
    )
}
