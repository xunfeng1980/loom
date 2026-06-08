use std::fmt;

pub const ENTRY_SYMBOL: &str = "loom_l2core_copy_i32";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeliorBackendDiagnosticCode {
    VerifierRejected,
    MissingVerifierFacts,
    UnsupportedLoweringShape,
    ToolchainMissing,
    ToolchainVersionMismatch,
    DialectRegistrationFailed,
    MlirVerificationFailed,
    PassPipelineFailed,
    JitUnavailable,
    JitSymbolMissing,
    NativeOutputMismatch,
}

impl MeliorBackendDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            MeliorBackendDiagnosticCode::VerifierRejected => "verifier-rejected",
            MeliorBackendDiagnosticCode::MissingVerifierFacts => "missing-verifier-facts",
            MeliorBackendDiagnosticCode::UnsupportedLoweringShape => "unsupported-lowering-shape",
            MeliorBackendDiagnosticCode::ToolchainMissing => "toolchain-missing",
            MeliorBackendDiagnosticCode::ToolchainVersionMismatch => "toolchain-version-mismatch",
            MeliorBackendDiagnosticCode::DialectRegistrationFailed => "dialect-registration-failed",
            MeliorBackendDiagnosticCode::MlirVerificationFailed => "mlir-verification-failed",
            MeliorBackendDiagnosticCode::PassPipelineFailed => "pass-pipeline-failed",
            MeliorBackendDiagnosticCode::JitUnavailable => "jit-unavailable",
            MeliorBackendDiagnosticCode::JitSymbolMissing => "jit-symbol-missing",
            MeliorBackendDiagnosticCode::NativeOutputMismatch => "native-output-mismatch",
        }
    }
}

impl fmt::Display for MeliorBackendDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeliorBackendDiagnostic {
    pub code: MeliorBackendDiagnosticCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MlirToolKind {
    LlvmConfig,
    MlirOpt,
    MlirTranslate,
    Lli,
}

impl MlirToolKind {
    pub fn binary_name(self) -> &'static str {
        match self {
            MlirToolKind::LlvmConfig => "llvm-config",
            MlirToolKind::MlirOpt => "mlir-opt",
            MlirToolKind::MlirTranslate => "mlir-translate",
            MlirToolKind::Lli => "lli",
        }
    }

    pub fn as_str(self) -> &'static str {
        self.binary_name()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MlirToolStatus {
    Found { path: String },
    Missing,
}

impl MlirToolStatus {
    pub fn is_found(&self) -> bool {
        matches!(self, MlirToolStatus::Found { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MlirToolFact {
    pub kind: MlirToolKind,
    pub status: MlirToolStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MlirToolchainFacts {
    pub expected_mlir_major: u32,
    pub llvm_config_version: Option<String>,
    pub detected_llvm_major: Option<u32>,
    pub compatible: bool,
    pub tools: Vec<MlirToolFact>,
}

impl MlirToolchainFacts {
    pub fn tool(&self, kind: MlirToolKind) -> Option<&MlirToolFact> {
        self.tools.iter().find(|tool| tool.kind == kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeliorBackendReport {
    pub supported: bool,
    pub diagnostics: Vec<MeliorBackendDiagnostic>,
    pub toolchain: Option<MlirToolchainFacts>,
    pub entry_symbol: Option<String>,
    pub jit_executed: bool,
    pub row_count: Option<u64>,
    pub artifact_summary: Option<String>,
}

impl Default for MeliorBackendReport {
    fn default() -> Self {
        Self {
            supported: false,
            diagnostics: Vec::new(),
            toolchain: None,
            entry_symbol: None,
            jit_executed: false,
            row_count: None,
            artifact_summary: None,
        }
    }
}

impl MeliorBackendReport {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn diagnostic(
        code: MeliorBackendDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let mut report = Self::default();
        report.push(code, path, message);
        report
    }

    pub fn push(
        &mut self,
        code: MeliorBackendDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(MeliorBackendDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_codes_are_stable() {
        assert_eq!(
            MeliorBackendDiagnosticCode::NativeOutputMismatch.as_str(),
            "native-output-mismatch"
        );
        assert_eq!(
            MeliorBackendDiagnosticCode::ToolchainVersionMismatch.as_str(),
            "toolchain-version-mismatch"
        );
    }

    #[test]
    fn report_ok_depends_only_on_diagnostics() {
        let mut report = MeliorBackendReport {
            supported: true,
            ..MeliorBackendReport::default()
        };
        assert!(report.is_ok());
        assert!(!report.jit_executed);

        report.push(
            MeliorBackendDiagnosticCode::JitUnavailable,
            "$.jit",
            "JIT not available",
        );
        assert!(!report.is_ok());
    }
}
