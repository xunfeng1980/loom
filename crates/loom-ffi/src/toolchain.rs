use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_ffi::report::{
    MeliorBackendDiagnosticCode, MeliorBackendReport, MlirToolFact, MlirToolKind, MlirToolStatus,
    MlirToolchainFacts,
};

pub const EXPECTED_MLIR_MAJOR: u32 = 22;

const EXTRA_TOOL_DIRS: &[&str] = &["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"];

pub fn probe_toolchain() -> MlirToolchainFacts {
    let llvm_config = find_tool(MlirToolKind::LlvmConfig);
    let llvm_config_version = llvm_config
        .as_ref()
        .and_then(|path| command_output(path, &["--version"]).ok());
    let detected_llvm_major = llvm_config_version
        .as_deref()
        .and_then(parse_llvm_major_version);

    let tools = [
        MlirToolKind::LlvmConfig,
        MlirToolKind::MlirOpt,
        MlirToolKind::MlirTranslate,
        MlirToolKind::Lli,
    ]
    .into_iter()
    .map(|kind| MlirToolFact {
        kind,
        status: find_tool(kind)
            .map(|path| MlirToolStatus::Found {
                path: path.display().to_string(),
            })
            .unwrap_or(MlirToolStatus::Missing),
    })
    .collect::<Vec<_>>();

    let compatible = detected_llvm_major == Some(EXPECTED_MLIR_MAJOR)
        && tools.iter().all(|tool| tool.status.is_found());

    MlirToolchainFacts {
        expected_mlir_major: EXPECTED_MLIR_MAJOR,
        llvm_config_version,
        detected_llvm_major,
        compatible,
        tools,
    }
}

pub fn require_compatible_toolchain() -> Result<MlirToolchainFacts, MeliorBackendReport> {
    let facts = probe_toolchain();
    if facts.compatible {
        return Ok(facts);
    }

    let mut report = MeliorBackendReport {
        toolchain: Some(facts.clone()),
        ..MeliorBackendReport::default()
    };
    if facts.detected_llvm_major.is_some()
        && facts.detected_llvm_major != Some(facts.expected_mlir_major)
    {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainVersionMismatch,
            "$.toolchain.llvm_config_version",
            format!(
                "detected LLVM/MLIR major {:?}, expected {}",
                facts.detected_llvm_major, facts.expected_mlir_major
            ),
        );
    } else {
        report.push(
            MeliorBackendDiagnosticCode::ToolchainMissing,
            "$.toolchain",
            "required MLIR/LLVM tools are not all available",
        );
    }
    Err(report)
}

pub fn parse_llvm_major_version(version: &str) -> Option<u32> {
    version
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse::<u32>().ok())
}

pub fn find_tool(kind: MlirToolKind) -> Option<PathBuf> {
    let binary = kind.binary_name();
    path_dirs()
        .into_iter()
        .chain(EXTRA_TOOL_DIRS.iter().map(PathBuf::from))
        .map(|dir| dir.join(binary))
        .find(|candidate| is_executable(candidate))
}

fn path_dirs() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).collect())
        .unwrap_or_default()
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn command_output(path: &Path, args: &[&str]) -> Result<String, std::io::Error> {
    let output = Command::new(path).args(args).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_major_version_from_homebrew_llvm() {
        assert_eq!(parse_llvm_major_version("21.1.2"), Some(21));
        assert_ne!(
            parse_llvm_major_version("21.1.2"),
            Some(EXPECTED_MLIR_MAJOR)
        );
    }

    #[test]
    fn parses_major_version_from_plain_major() {
        assert_eq!(parse_llvm_major_version("22"), Some(22));
    }

    #[test]
    fn reports_missing_or_incompatible_without_panicking() {
        let result = require_compatible_toolchain();
        if let Err(report) = result {
            assert!(!report.diagnostics.is_empty());
            assert!(matches!(
                report.diagnostics[0].code,
                MeliorBackendDiagnosticCode::ToolchainMissing
                    | MeliorBackendDiagnosticCode::ToolchainVersionMismatch
            ));
        }
    }
}
