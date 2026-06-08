//! Optional command-line SMT solver execution for Loom verifier obligations.
//!
//! `loom-core` owns solver-neutral reports and SMT-LIB scripts. This crate is
//! the subprocess boundary: it discovers installed solvers, runs scripts, and
//! maps command results back into `SolverDischargeReport`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use loom_core::solver::{
    SmtLibScript, SolverBackendInfo, SolverBackendKind, SolverDischargeReport,
    SolverObligationResult, SolverObligationStatus, SolverRawResult,
};
use loom_core::{
    artifact_verifier::{
        apply_solver_discharge, verify_artifact_with_l2_core, ArtifactVerificationOptions,
        ArtifactVerificationReport, ArtifactVerificationStatus,
    },
    full_verifier::verify_l2_core,
    l2_core::L2CoreProgram,
    l2_kernel_registry::L2KernelRegistry,
    solver::emit_required_qfbv_script,
};

const DEFAULT_TIMEOUT_MS: u64 = 5_000;
const EXCERPT_LIMIT: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverRunOptions {
    pub strict: bool,
    pub timeout_ms: u64,
    pub path_override: Option<PathBuf>,
}

impl Default for SolverRunOptions {
    fn default() -> Self {
        Self {
            strict: env::var("LOOM_REQUIRE_SOLVER").ok().as_deref() == Some("1"),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            path_override: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverBackendDiscovery {
    pub kind: SolverBackendKind,
    pub binary: &'static str,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub available: bool,
    pub diagnostic: Option<String>,
}

impl SolverBackendDiscovery {
    pub fn backend_info(&self, strict: bool, timeout_ms: u64) -> SolverBackendInfo {
        SolverBackendInfo {
            kind: self.kind,
            version: self.version.clone(),
            path: self.path.as_ref().map(|path| path.display().to_string()),
            strict,
            timeout_ms,
        }
    }
}

pub trait SolverCommandBackend {
    fn kind(&self) -> SolverBackendKind;
    fn discover(&self) -> SolverBackendDiscovery;
    fn execute(&self, script: &SmtLibScript) -> SolverDischargeReport;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitwuzlaBackend {
    options: SolverRunOptions,
}

impl BitwuzlaBackend {
    pub fn new(options: SolverRunOptions) -> Self {
        Self { options }
    }
}

impl Default for BitwuzlaBackend {
    fn default() -> Self {
        Self::new(SolverRunOptions::default())
    }
}

impl SolverCommandBackend for BitwuzlaBackend {
    fn kind(&self) -> SolverBackendKind {
        SolverBackendKind::Bitwuzla
    }

    fn discover(&self) -> SolverBackendDiscovery {
        discover_backend_with_override(
            SolverBackendKind::Bitwuzla,
            self.options.path_override.as_deref(),
        )
    }

    fn execute(&self, script: &SmtLibScript) -> SolverDischargeReport {
        execute_bitwuzla_script(script, &self.options)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredBackend {
    kind: SolverBackendKind,
    options: SolverRunOptions,
}

impl DeclaredBackend {
    pub fn z3(options: SolverRunOptions) -> Self {
        Self {
            kind: SolverBackendKind::Z3,
            options,
        }
    }

    pub fn cvc5(options: SolverRunOptions) -> Self {
        Self {
            kind: SolverBackendKind::Cvc5,
            options,
        }
    }
}

impl SolverCommandBackend for DeclaredBackend {
    fn kind(&self) -> SolverBackendKind {
        self.kind
    }

    fn discover(&self) -> SolverBackendDiscovery {
        discover_backend_with_override(self.kind, self.options.path_override.as_deref())
    }

    fn execute(&self, script: &SmtLibScript) -> SolverDischargeReport {
        let discovery = self.discover();
        let backend = discovery.backend_info(self.options.strict, self.options.timeout_ms);
        report_for_raw(
            script,
            backend,
            if self.options.strict {
                SolverRawResult::Error
            } else {
                SolverRawResult::Skipped
            },
            format!(
                "{} execution adapter is declared but deferred; Phase 19 executes bitwuzla",
                self.kind.as_str()
            ),
            None,
            None,
        )
    }
}

pub fn declared_backends(options: SolverRunOptions) -> Vec<Box<dyn SolverCommandBackend>> {
    vec![
        Box::new(DeclaredBackend::z3(options.clone())),
        Box::new(DeclaredBackend::cvc5(options.clone())),
        Box::new(BitwuzlaBackend::new(options)),
    ]
}

pub fn discover_backend(kind: SolverBackendKind) -> SolverBackendDiscovery {
    discover_backend_with_override(kind, None)
}

pub fn discover_all_backends() -> Vec<SolverBackendDiscovery> {
    [
        SolverBackendKind::Z3,
        SolverBackendKind::Cvc5,
        SolverBackendKind::Bitwuzla,
    ]
    .into_iter()
    .map(discover_backend)
    .collect()
}

pub fn execute_bitwuzla_script(
    script: &SmtLibScript,
    options: &SolverRunOptions,
) -> SolverDischargeReport {
    let discovery = discover_backend_with_override(
        SolverBackendKind::Bitwuzla,
        options.path_override.as_deref(),
    );
    let backend = discovery.backend_info(options.strict, options.timeout_ms);

    let Some(path) = discovery.path.as_deref() else {
        let raw = if options.strict {
            SolverRawResult::Error
        } else {
            SolverRawResult::Skipped
        };
        return report_for_raw(
            script,
            backend,
            raw,
            "bitwuzla binary unavailable; solver evidence skipped".to_string(),
            None,
            None,
        );
    };

    let script_path = temp_script_path(script);
    if let Err(err) = fs::write(&script_path, &script.text) {
        return report_for_raw(
            script,
            backend,
            SolverRawResult::Error,
            format!("failed to write temporary SMT-LIB script: {err}"),
            None,
            None,
        );
    }

    let output = run_with_timeout(path, &[script_path.as_path()], options.timeout_ms);
    let _ = fs::remove_file(&script_path);

    match output {
        Ok(CommandRunOutput::Completed {
            status_success,
            stdout,
            stderr,
        }) => {
            if !status_success {
                return report_for_raw(
                    script,
                    backend,
                    SolverRawResult::Error,
                    "bitwuzla exited with non-zero status".to_string(),
                    Some(stdout),
                    Some(stderr),
                );
            }

            match parse_decisive_result(&stdout) {
                Some(raw) => report_for_raw(
                    script,
                    backend,
                    raw,
                    format!("bitwuzla returned {}", raw.as_str()),
                    Some(stdout),
                    Some(stderr),
                ),
                None => report_for_raw(
                    script,
                    backend,
                    SolverRawResult::Error,
                    "bitwuzla stdout did not contain a decisive sat/unsat/unknown result"
                        .to_string(),
                    Some(stdout),
                    Some(stderr),
                ),
            }
        }
        Ok(CommandRunOutput::TimedOut { stdout, stderr }) => report_for_raw(
            script,
            backend,
            SolverRawResult::Timeout,
            format!("bitwuzla timed out after {} ms", options.timeout_ms),
            Some(stdout),
            Some(stderr),
        ),
        Err(err) => report_for_raw(
            script,
            backend,
            SolverRawResult::Error,
            format!("failed to execute bitwuzla: {err}"),
            None,
            None,
        ),
    }
}

pub fn verify_artifact_with_l2_core_and_bitwuzla(
    bytes: &[u8],
    registry: &L2KernelRegistry,
    program: &L2CoreProgram,
    artifact_options: &ArtifactVerificationOptions,
    solver_options: &SolverRunOptions,
) -> ArtifactVerificationReport {
    let artifact_report = verify_artifact_with_l2_core(bytes, registry, program, artifact_options);
    if artifact_report.status() != ArtifactVerificationStatus::Accepted {
        return artifact_report;
    }

    let l2_report = verify_l2_core(program);
    let script = emit_required_qfbv_script("artifact-l2core.required", l2_report.constraints());
    let solver_report = execute_bitwuzla_script(&script, solver_options);
    apply_solver_discharge(artifact_report, solver_report)
}

pub fn parse_decisive_result(stdout: &str) -> Option<SolverRawResult> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| match line {
            "unsat" => Some(SolverRawResult::Unsat),
            "sat" => Some(SolverRawResult::Sat),
            "unknown" => Some(SolverRawResult::Unknown),
            _ => None,
        })
}

fn discover_backend_with_override(
    kind: SolverBackendKind,
    path_override: Option<&Path>,
) -> SolverBackendDiscovery {
    let binary = match kind {
        SolverBackendKind::Z3 => "z3",
        SolverBackendKind::Cvc5 => "cvc5",
        SolverBackendKind::Bitwuzla => "bitwuzla",
    };
    let path = path_override
        .filter(|path| path.is_file())
        .map(Path::to_path_buf)
        .or_else(|| find_binary(binary));
    let version = path.as_deref().and_then(command_version);
    let available = path.is_some();
    let diagnostic = if available {
        None
    } else {
        Some(format!("{binary} not found on PATH"))
    };

    SolverBackendDiscovery {
        kind,
        binary,
        path,
        version,
        available,
        diagnostic,
    }
}

fn report_for_raw(
    script: &SmtLibScript,
    backend: SolverBackendInfo,
    raw: SolverRawResult,
    diagnostic: String,
    stdout: Option<String>,
    stderr: Option<String>,
) -> SolverDischargeReport {
    let obligation_ids = if script.obligation_ids.is_empty() {
        vec![script.id.clone()]
    } else {
        script.obligation_ids.clone()
    };

    let stdout_excerpt = stdout.map(|value| excerpt(&value));
    let stderr_excerpt = stderr.map(|value| excerpt(&value));
    let results = obligation_ids
        .into_iter()
        .map(|obligation_id| {
            let mut result = SolverObligationResult::new(obligation_id, backend.clone(), raw);
            result.stdout_excerpt = stdout_excerpt.clone();
            result.stderr_excerpt = stderr_excerpt.clone();
            if result.status == SolverObligationStatus::Unknown {
                result.reason_unknown = Some("solver returned unknown".to_string());
            }
            result
        })
        .collect();

    let mut report = SolverDischargeReport::from_results(results);
    report.scripts.push(script.clone());
    report.diagnostics.push(diagnostic);
    report
}

enum CommandRunOutput {
    Completed {
        status_success: bool,
        stdout: String,
        stderr: String,
    },
    TimedOut {
        stdout: String,
        stderr: String,
    },
}

fn run_with_timeout(
    program: &Path,
    args: &[&Path],
    timeout_ms: u64,
) -> Result<CommandRunOutput, std::io::Error> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started = SystemTime::now();

    loop {
        if let Some(status) = child.try_wait()? {
            let output = child.wait_with_output()?;
            return Ok(CommandRunOutput::Completed {
                status_success: status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        if started.elapsed().unwrap_or_default() >= timeout {
            let _ = child.kill();
            let output = child.wait_with_output()?;
            return Ok(CommandRunOutput::TimedOut {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn find_binary(binary: &str) -> Option<PathBuf> {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(binary))
        .find(|path| path.is_file())
}

fn command_version(path: &Path) -> Option<String> {
    Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                (!stderr.is_empty()).then_some(stderr)
            } else {
                Some(stdout)
            }
        })
        .map(|version| excerpt(&version))
}

fn temp_script_path(script: &SmtLibScript) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "loom-{}-{}-{}.smt2",
        std::process::id(),
        sanitize_filename(&script.deterministic_id),
        nanos
    ))
}

fn sanitize_filename(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn excerpt(value: &str) -> String {
    if value.len() <= EXCERPT_LIMIT {
        value.to_string()
    } else {
        value.chars().take(EXCERPT_LIMIT).collect()
    }
}
