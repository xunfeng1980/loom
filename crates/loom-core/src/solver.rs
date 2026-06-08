//! Solver-neutral obligation and discharge report model.
//!
//! `loom-core` owns the report vocabulary and deterministic SMT-LIB contract
//! metadata. Solver process execution lives outside this crate.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::l2_core::constraints::{ConstraintSet, ConstraintTerm, IntegerType, LoomConstraint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverBackendKind {
    Z3,
    Cvc5,
    Bitwuzla,
}

impl SolverBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Z3 => "z3",
            Self::Cvc5 => "cvc5",
            Self::Bitwuzla => "bitwuzla",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverTheory {
    QfBv,
    QfLia,
}

impl SolverTheory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QfBv => "QF_BV",
            Self::QfLia => "QF_LIA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverBitWidthPolicy {
    Fixed(u16),
    Offset64,
    Native32,
    Native64,
}

impl SolverBitWidthPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed(_) => "fixed",
            Self::Offset64 => "offset64",
            Self::Native32 => "native32",
            Self::Native64 => "native64",
        }
    }

    pub fn bits(self) -> u16 {
        match self {
            Self::Fixed(bits) => bits,
            Self::Offset64 | Self::Native64 => 64,
            Self::Native32 => 32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverObligationKind {
    Bounds,
    RowResource,
    ArithmeticRange,
    FeatureImplication,
    NativeExactness,
}

impl SolverObligationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bounds => "bounds",
            Self::RowResource => "row-resource",
            Self::ArithmeticRange => "arithmetic-range",
            Self::FeatureImplication => "feature-implication",
            Self::NativeExactness => "native-exactness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverQuerySemantics {
    BadStateUnsat,
}

impl SolverQuerySemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BadStateUnsat => "bad-state-unsat",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtLibScriptFamily {
    Required,
    CrossCheck,
}

impl SmtLibScriptFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::CrossCheck => "cross-check",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverObligation {
    pub id: String,
    pub kind: SolverObligationKind,
    pub theory: SolverTheory,
    pub bit_width_policy: SolverBitWidthPolicy,
    pub query_semantics: SolverQuerySemantics,
    pub source_stage: String,
    pub source_path: String,
    pub constraint_ids: Vec<String>,
    pub required: bool,
}

impl SolverObligation {
    pub fn required_qfbv(
        id: impl Into<String>,
        kind: SolverObligationKind,
        source_stage: impl Into<String>,
        source_path: impl Into<String>,
        constraint_ids: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            theory: SolverTheory::QfBv,
            bit_width_policy: SolverBitWidthPolicy::Offset64,
            query_semantics: SolverQuerySemantics::BadStateUnsat,
            source_stage: source_stage.into(),
            source_path: source_path.into(),
            constraint_ids,
            required: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverRawResult {
    Unsat,
    Sat,
    Unknown,
    Timeout,
    Error,
    Skipped,
}

impl SolverRawResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsat => "unsat",
            Self::Sat => "sat",
            Self::Unknown => "unknown",
            Self::Timeout => "timeout",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverObligationStatus {
    Discharged,
    Failed,
    Unknown,
    TimedOut,
    Error,
    Skipped,
}

impl SolverObligationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discharged => "discharged",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::TimedOut => "timed-out",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }

    pub fn from_bad_state_result(raw: SolverRawResult) -> Self {
        match raw {
            SolverRawResult::Unsat => Self::Discharged,
            SolverRawResult::Sat => Self::Failed,
            SolverRawResult::Unknown => Self::Unknown,
            SolverRawResult::Timeout => Self::TimedOut,
            SolverRawResult::Error => Self::Error,
            SolverRawResult::Skipped => Self::Skipped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtLibScript {
    pub id: String,
    pub family: SmtLibScriptFamily,
    pub logic: SolverTheory,
    pub text: String,
    pub expected_success: SolverRawResult,
    pub obligation_ids: Vec<String>,
    pub deterministic_id: String,
}

impl SmtLibScript {
    pub fn required_qfbv(id: impl Into<String>, text: String, obligation_ids: Vec<String>) -> Self {
        let id = id.into();
        let deterministic_id = deterministic_text_id(&id, &text);
        Self {
            id,
            family: SmtLibScriptFamily::Required,
            logic: SolverTheory::QfBv,
            text,
            expected_success: SolverRawResult::Unsat,
            obligation_ids,
            deterministic_id,
        }
    }

    pub fn cross_check_qflia(
        id: impl Into<String>,
        text: String,
        obligation_ids: Vec<String>,
    ) -> Self {
        let id = id.into();
        let deterministic_id = deterministic_text_id(&id, &text);
        Self {
            id,
            family: SmtLibScriptFamily::CrossCheck,
            logic: SolverTheory::QfLia,
            text,
            expected_success: SolverRawResult::Unsat,
            obligation_ids,
            deterministic_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverBackendInfo {
    pub kind: SolverBackendKind,
    pub version: Option<String>,
    pub path: Option<String>,
    pub strict: bool,
    pub timeout_ms: u64,
}

impl SolverBackendInfo {
    pub fn bitwuzla(path: Option<impl Into<String>>, strict: bool, timeout_ms: u64) -> Self {
        Self {
            kind: SolverBackendKind::Bitwuzla,
            version: None,
            path: path.map(Into::into),
            strict,
            timeout_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverObligationResult {
    pub obligation_id: String,
    pub backend: SolverBackendInfo,
    pub status: SolverObligationStatus,
    pub raw_result: SolverRawResult,
    pub model_excerpt: Option<String>,
    pub unsat_core_ids: Vec<String>,
    pub reason_unknown: Option<String>,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
}

impl SolverObligationResult {
    pub fn new(
        obligation_id: impl Into<String>,
        backend: SolverBackendInfo,
        raw_result: SolverRawResult,
    ) -> Self {
        Self {
            obligation_id: obligation_id.into(),
            backend,
            status: SolverObligationStatus::from_bad_state_result(raw_result),
            raw_result,
            model_excerpt: None,
            unsat_core_ids: Vec::new(),
            reason_unknown: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SolverDischargeSummary {
    pub required_obligation_count: usize,
    pub discharged_count: usize,
    pub failed_count: usize,
    pub unknown_count: usize,
    pub timed_out_count: usize,
    pub errored_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverDischargeReport {
    pub status: SolverObligationStatus,
    pub backend_results: Vec<SolverObligationResult>,
    pub required_obligation_count: usize,
    pub discharged_count: usize,
    pub failed_count: usize,
    pub unknown_count: usize,
    pub skipped_count: usize,
    pub scripts: Vec<SmtLibScript>,
    pub diagnostics: Vec<String>,
}

impl SolverDischargeReport {
    pub fn from_results(results: Vec<SolverObligationResult>) -> Self {
        let mut summary = SolverDischargeSummary {
            required_obligation_count: results.len(),
            ..SolverDischargeSummary::default()
        };
        for result in &results {
            match result.status {
                SolverObligationStatus::Discharged => summary.discharged_count += 1,
                SolverObligationStatus::Failed => summary.failed_count += 1,
                SolverObligationStatus::Unknown => summary.unknown_count += 1,
                SolverObligationStatus::TimedOut => summary.timed_out_count += 1,
                SolverObligationStatus::Error => summary.errored_count += 1,
                SolverObligationStatus::Skipped => summary.skipped_count += 1,
            }
        }
        let status = if !results.is_empty()
            && summary.discharged_count == summary.required_obligation_count
        {
            SolverObligationStatus::Discharged
        } else if summary.failed_count > 0 {
            SolverObligationStatus::Failed
        } else if summary.timed_out_count > 0 {
            SolverObligationStatus::TimedOut
        } else if summary.errored_count > 0 {
            SolverObligationStatus::Error
        } else if summary.unknown_count > 0 {
            SolverObligationStatus::Unknown
        } else {
            SolverObligationStatus::Skipped
        };

        Self {
            status,
            backend_results: results,
            required_obligation_count: summary.required_obligation_count,
            discharged_count: summary.discharged_count,
            failed_count: summary.failed_count,
            unknown_count: summary.unknown_count,
            skipped_count: summary.skipped_count,
            scripts: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn is_successful(&self) -> bool {
        self.status == SolverObligationStatus::Discharged
            && self.required_obligation_count > 0
            && self.discharged_count == self.required_obligation_count
            && self.failed_count == 0
            && self.unknown_count == 0
            && self.skipped_count == 0
    }
}

pub fn emit_required_qfbv_script(
    script_id: impl Into<String>,
    constraints: &ConstraintSet,
) -> SmtLibScript {
    let script_id = script_id.into();
    let mut constraints: Vec<_> = constraints.iter().collect();
    constraints.sort_by(|left, right| left.id().cmp(right.id()));

    let mut vars = BTreeSet::new();
    for constraint in &constraints {
        collect_constraint_vars(constraint, &mut vars);
    }

    let mut text = String::new();
    text.push_str("(set-info :smt-lib-version 2.7)\n");
    text.push_str("(set-option :print-success false)\n");
    text.push_str("(set-option :produce-models true)\n");
    text.push_str("(set-option :produce-unsat-cores true)\n");
    text.push_str("(set-logic QF_BV)\n\n");
    let _ = writeln!(text, "; loom-smt-script {script_id}");
    text.push_str("; loom-smt-family required\n");
    text.push_str("; loom-smt-primary-backend bitwuzla\n\n");

    for var in vars {
        let _ = writeln!(text, "(declare-const {} (_ BitVec 64))", smt_symbol(&var));
    }
    if !constraints.is_empty() {
        text.push('\n');
    }

    let mut obligation_ids = Vec::new();
    for (idx, constraint) in constraints.iter().enumerate() {
        let name = format!("bad_{}_{}", sanitize_symbol(constraint.id()), idx);
        let bad = bad_state_qfbv(constraint);
        let _ = writeln!(text, "(assert (! {bad} :named {}))", smt_symbol(&name));
        obligation_ids.push(constraint.id().to_string());
    }

    text.push_str("\n(check-sat)\n(exit)\n");
    SmtLibScript::required_qfbv(script_id, text, obligation_ids)
}

fn collect_constraint_vars(constraint: &LoomConstraint, vars: &mut BTreeSet<String>) {
    match constraint {
        LoomConstraint::Le { left, right, .. }
        | LoomConstraint::Lt { left, right, .. }
        | LoomConstraint::Eq { left, right, .. }
        | LoomConstraint::AddNoOverflow { left, right, .. }
        | LoomConstraint::MulNoOverflow { left, right, .. } => {
            collect_term_vars(left, vars);
            collect_term_vars(right, vars);
        }
        LoomConstraint::InRange {
            value,
            lower,
            upper_exclusive,
            ..
        } => {
            collect_term_vars(value, vars);
            collect_term_vars(lower, vars);
            collect_term_vars(upper_exclusive, vars);
        }
        LoomConstraint::Decreases { previous, next, .. } => {
            collect_term_vars(previous, vars);
            collect_term_vars(next, vars);
        }
        LoomConstraint::NonNegative { value, .. } => collect_term_vars(value, vars),
        LoomConstraint::FeatureImplies { .. } => {}
    }
}

fn collect_term_vars(term: &ConstraintTerm, vars: &mut BTreeSet<String>) {
    match term {
        ConstraintTerm::Var(name) => {
            vars.insert(name.clone());
        }
        ConstraintTerm::Int(_) => {}
        ConstraintTerm::Add(left, right)
        | ConstraintTerm::Sub(left, right)
        | ConstraintTerm::Mul(left, right) => {
            collect_term_vars(left, vars);
            collect_term_vars(right, vars);
        }
    }
}

fn bad_state_qfbv(constraint: &LoomConstraint) -> String {
    match constraint {
        LoomConstraint::Le { left, right, .. } => {
            format!("(bvugt {} {})", term_qfbv(left), term_qfbv(right))
        }
        LoomConstraint::Lt { left, right, .. } => {
            format!("(not (bvult {} {}))", term_qfbv(left), term_qfbv(right))
        }
        LoomConstraint::Eq { left, right, .. } => {
            format!("(not (= {} {}))", term_qfbv(left), term_qfbv(right))
        }
        LoomConstraint::AddNoOverflow {
            left, right, ty, ..
        } => {
            let left = term_qfbv_with_type(left, ty);
            let right = term_qfbv_with_type(right, ty);
            format!("(bvult (bvadd {left} {right}) {left})")
        }
        LoomConstraint::MulNoOverflow {
            left, right, ty, ..
        } => {
            let left = term_qfbv_with_type(left, ty);
            let right = term_qfbv_with_type(right, ty);
            format!(
                "(and (not (= {right} {})) (bvult (bvmul {left} {right}) {left}))",
                bv_const(0, bits_for_type(ty))
            )
        }
        LoomConstraint::InRange {
            value,
            lower,
            upper_exclusive,
            ..
        } => format!(
            "(or (bvult {} {}) (not (bvult {} {})))",
            term_qfbv(value),
            term_qfbv(lower),
            term_qfbv(value),
            term_qfbv(upper_exclusive)
        ),
        LoomConstraint::Decreases { previous, next, .. } => {
            format!("(not (bvult {} {}))", term_qfbv(next), term_qfbv(previous))
        }
        LoomConstraint::NonNegative { .. } | LoomConstraint::FeatureImplies { .. } => {
            "false".to_string()
        }
    }
}

fn term_qfbv(term: &ConstraintTerm) -> String {
    term_qfbv_bits(term, 64)
}

fn term_qfbv_with_type(term: &ConstraintTerm, ty: &IntegerType) -> String {
    term_qfbv_bits(term, bits_for_type(ty))
}

fn term_qfbv_bits(term: &ConstraintTerm, bits: u16) -> String {
    match term {
        ConstraintTerm::Var(name) => smt_symbol(name),
        ConstraintTerm::Int(value) => bv_const(*value, bits),
        ConstraintTerm::Add(left, right) => {
            format!(
                "(bvadd {} {})",
                term_qfbv_bits(left, bits),
                term_qfbv_bits(right, bits)
            )
        }
        ConstraintTerm::Sub(left, right) => {
            format!(
                "(bvsub {} {})",
                term_qfbv_bits(left, bits),
                term_qfbv_bits(right, bits)
            )
        }
        ConstraintTerm::Mul(left, right) => {
            format!(
                "(bvmul {} {})",
                term_qfbv_bits(left, bits),
                term_qfbv_bits(right, bits)
            )
        }
    }
}

fn bits_for_type(ty: &IntegerType) -> u16 {
    match ty {
        // Phase 19 starts with a stable 64-bit policy so all symbolic
        // variables have one declaration width. Narrow native widths can be
        // refined later by the backend-specific emitter.
        IntegerType::Int32 | IntegerType::UInt32 => 64,
        IntegerType::Int64 | IntegerType::UInt64 | IntegerType::RowIndex => 64,
    }
}

fn bv_const(value: i128, bits: u16) -> String {
    let bits = bits.clamp(1, 128);
    let mask = if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    let wrapped = if value >= 0 {
        (value as u128) & mask
    } else {
        let magnitude = value.unsigned_abs() & mask;
        ((!magnitude).wrapping_add(1)) & mask
    };
    if bits % 4 == 0 {
        let hex_width = usize::from(bits / 4);
        format!("#x{wrapped:0hex_width$x}")
    } else {
        format!("(_ bv{wrapped} {bits})")
    }
}

fn smt_symbol(raw: &str) -> String {
    format!("loom_{}", sanitize_symbol(raw))
}

fn sanitize_symbol(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "anon".to_string()
    } else {
        out
    }
}

fn deterministic_text_id(id: &str, text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in id.bytes().chain(text.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64-{hash:016x}")
}
