//! Solver-neutral obligation and discharge report model.
//!
//! `loom-core` owns the report vocabulary and deterministic SMT-LIB contract
//! metadata. Solver process execution lives outside this crate.

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
    pub logic: SolverTheory,
    pub text: String,
    pub expected_success: SolverRawResult,
    pub obligation_ids: Vec<String>,
    pub deterministic_id: String,
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
