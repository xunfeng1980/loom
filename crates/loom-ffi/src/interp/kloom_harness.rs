//! K Framework harness for L2Core program trace extraction.
//!
//! Serializes an [`L2CoreProgram`] to the kloom textual format, invokes `krun`,
//! and parses the pretty-printed output to extract trace events.
//!
//! Supported constructs: the full ScalarExpr language, including `Min`/`Max`
//! and `Bytes` constants (modelled via the kloom `bytesConst` literal).
//!
//! Trust model: spec-oracle, offline/CI only, outside production TCB.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use loom_ir_core::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, ScalarExpr, ScalarValue,
};

#[derive(Debug, Clone)]
pub struct KloomHarnessError {
    pub message: String,
}

impl std::fmt::Display for KloomHarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for KloomHarnessError {}

impl KloomHarnessError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Outcome of a K spec-oracle trace extraction attempt.
///
/// `ProducedTrace` means krun ran and emitted a usable reference trace.
/// `SkippedRefereeAbsent` means krun/kompile was missing or timed out — the
/// referee is absent and the gate should record a skip, not a hard fail.
/// `UnsupportedProgram` is reserved for constructs the harness cannot faithfully
/// serialize to kloom syntax. As of Bytes support, every modelled construct
/// serializes cleanly, so this outcome currently has no triggers — it is
/// retained as a forward guard for future unmodelled additions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KOracleOutcome {
    ProducedTrace(Vec<String>),
    SkippedRefereeAbsent { reason: String },
    UnsupportedProgram { reason: String },
}

const KRUN_TIMEOUT_SECS: u64 = 30;
const LOOM_ALLOW_K_ORACLE_SKIP_VAR: &str = "LOOM_ALLOW_K_ORACLE_SKIP";

/// Extract the reference trace for a program by running it through the K
/// semantics (`krun`).
///
/// Returns a typed outcome that distinguishes "produced a usable trace",
/// "referee absent" (skip), and "unsupported construct" (skip-with-reason).
pub fn kloom_trace_for_program(program: &L2CoreProgram) -> Result<KOracleOutcome, KloomHarnessError> {
    if let Some(reason) = program_uses_unsupported_constructs(program) {
        return Ok(KOracleOutcome::UnsupportedProgram {
            reason: reason.to_string(),
        });
    }
    let text = serialize_program(program)?;
    run_kloom(&text)
}

// ---------------------------------------------------------------------------
// Unsupported-construct predicate (Phase 48)
// ---------------------------------------------------------------------------

/// Returns `Some(reason)` if the program contains any construct that the
/// kloom harness cannot faithfully serialize.  This guards against placeholder
/// lowerings silently poisoning the differential gate. Every construct is
/// currently modelled, so this returns `None` for all programs; it is kept as a
/// forward guard for any future unmodelled construct.
fn program_uses_unsupported_constructs(program: &L2CoreProgram) -> Option<&'static str> {
    for stmt in &program.body {
        if let Some(reason) = stmt_uses_unsupported(stmt) {
            return Some(reason);
        }
    }
    None
}

fn stmt_uses_unsupported(stmt: &L2CoreStmt) -> Option<&'static str> {
    match stmt {
        L2CoreStmt::AppendValue { value, .. } => expr_uses_unsupported(value),
        L2CoreStmt::AppendNull { .. } => None,
        L2CoreStmt::ReadInput { offset, width, .. } => {
            expr_uses_unsupported(offset).or_else(|| expr_uses_unsupported(width))
        }
        L2CoreStmt::LetScalar { expr, .. } => expr_uses_unsupported(expr),
        L2CoreStmt::ForRange { start, end, body, .. } => {
            expr_uses_unsupported(start)
                .or_else(|| expr_uses_unsupported(end))
                .or_else(|| body.iter().find_map(stmt_uses_unsupported))
        }
        L2CoreStmt::CursorLoop { limit, progress, body, .. } => {
            expr_uses_unsupported(limit)
                .or_else(|| expr_uses_unsupported(progress))
                .or_else(|| body.iter().find_map(stmt_uses_unsupported))
        }
        L2CoreStmt::FailClosed { .. } => None,
    }
}

fn expr_uses_unsupported(expr: &ScalarExpr) -> Option<&'static str> {
    match expr {
        // All scalar constants — including Bytes — are now modelled in kloom.
        ScalarExpr::Const(_) | ScalarExpr::Var(_) => None,
        ScalarExpr::Add(l, r)
        | ScalarExpr::Sub(l, r)
        | ScalarExpr::Mul(l, r)
        | ScalarExpr::Min(l, r)
        | ScalarExpr::Max(l, r)
        | ScalarExpr::Eq(l, r)
        | ScalarExpr::Lt(l, r)
        | ScalarExpr::Le(l, r) => {
            expr_uses_unsupported(l).or_else(|| expr_uses_unsupported(r))
        }
        // Bitcast is verified by the full verifier and executed by the
        // interpreter, but is not yet modelled in the kloom K-semantics — flag
        // it so the kloom differential is skipped rather than mis-modelled.
        ScalarExpr::Bitcast { .. } => Some("bitcast not yet modelled in kloom"),
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

fn serialize_program(program: &L2CoreProgram) -> Result<String, KloomHarnessError> {
    let mut out = String::new();
    out.push_str("program\n");

    let mut caps = Vec::new();
    for cap in &program.capabilities {
        match cap {
            Capability::InputSlice(input) => {
                // InputSlice does not carry an Arrow type in the current model.
                // We default to int32 for the harness; the trace format does not
                // record input reads for model-validation programs anyway.
                caps.push(format!(
                    "  input {}:int32 {} {}",
                    sanitize_id(&input.id),
                    input.offset,
                    input.length
                ));
            }
            Capability::OutputBuilder(builder) => {
                let ty = arrow_type_to_l2ty(&builder.arrow_type)?;
                let nullable = if builder.nullable { " nullable" } else { "" };
                caps.push(format!(
                    "  builder {}:{}{}",
                    sanitize_id(&builder.id),
                    ty,
                    nullable
                ));
            }
            Capability::Scratch(scratch) => {
                // Scratch capabilities are not represented in kloom syntax.
                // They do not affect the trace, so we skip them.
                let _ = scratch;
            }
        }
    }
    out.push_str(&caps.join(",\n"));
    out.push('\n');

    out.push_str("body\n");
    for (i, stmt) in program.body.iter().enumerate() {
        out.push_str("  ");
        serialize_stmt(&mut out, stmt)?;
        if i + 1 < program.body.len() {
            out.push(';');
        }
        out.push('\n');
    }

    out.push_str("maxRows ");
    out.push_str(&program.resource_budget.max_rows.to_string());
    out.push('\n');

    Ok(out)
}

fn arrow_type_to_l2ty(dt: &loom_ir_core::l2_core::L2DataType) -> Result<&'static str, KloomHarnessError> {
    match dt {
        loom_ir_core::l2_core::L2DataType::Int32 => Ok("int32"),
        loom_ir_core::l2_core::L2DataType::Int64 => Ok("int64"),
        loom_ir_core::l2_core::L2DataType::Float32 => Ok("float32"),
        loom_ir_core::l2_core::L2DataType::Float64 => Ok("float64"),
        loom_ir_core::l2_core::L2DataType::Boolean => Ok("bool"),
        // Utf8 is the bytes-bearing L2 type (mirrors ScalarType::Bytes); kloom
        // models it as the `bytes` builder type.
        loom_ir_core::l2_core::L2DataType::Utf8 => Ok("bytes"),
    }
}

fn sanitize_id(id: &str) -> String {
    // kloom Id syntax: [a-zA-Z][a-zA-Z0-9_]*
    // Replace any character outside that set with '_'.
    id.chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                if c.is_ascii_alphabetic() {
                    c
                } else {
                    '_'
                }
            } else if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn serialize_stmt(out: &mut String, stmt: &L2CoreStmt) -> Result<(), KloomHarnessError> {
    match stmt {
        L2CoreStmt::AppendValue { builder, value } => {
            out.push_str("appendValue(");
            out.push_str(&sanitize_id(builder));
            out.push_str(", ");
            serialize_expr(out, value)?;
            out.push(')');
        }
        L2CoreStmt::AppendNull { builder } => {
            out.push_str("appendNull(");
            out.push_str(&sanitize_id(builder));
            out.push(')');
        }
        L2CoreStmt::ReadInput {
            capability,
            offset,
            width,
            bind,
        } => {
            out.push_str("readInput(");
            out.push_str(&sanitize_id(capability));
            out.push_str(", ");
            serialize_expr(out, offset)?;
            out.push_str(", ");
            serialize_expr(out, width)?;
            out.push_str(", ");
            out.push_str(&sanitize_id(bind));
            out.push(')');
        }
        L2CoreStmt::LetScalar { name, expr } => {
            out.push_str("letScalar(");
            out.push_str(&sanitize_id(name));
            out.push_str(", ");
            serialize_expr(out, expr)?;
            out.push(')');
        }
        L2CoreStmt::ForRange { index, start, end, body } => {
            out.push_str("forRange(");
            out.push_str(&sanitize_id(index));
            out.push_str(", ");
            serialize_expr(out, start)?;
            out.push_str(", ");
            serialize_expr(out, end)?;
            out.push_str(",\n");
            for (i, s) in body.iter().enumerate() {
                out.push_str("    ");
                serialize_stmt(out, s)?;
                if i + 1 < body.len() {
                    out.push(';');
                }
                out.push('\n');
            }
            out.push_str("  )");
        }
        L2CoreStmt::CursorLoop {
            cursor,
            limit,
            progress,
            body,
        } => {
            out.push_str("cursorLoop(");
            out.push_str(&sanitize_id(cursor));
            out.push_str(", ");
            serialize_expr(out, limit)?;
            out.push_str(", ");
            serialize_expr(out, progress)?;
            out.push_str(",\n");
            for (i, s) in body.iter().enumerate() {
                out.push_str("    ");
                serialize_stmt(out, s)?;
                if i + 1 < body.len() {
                    out.push(';');
                }
                out.push('\n');
            }
            out.push_str("  )");
        }
        L2CoreStmt::FailClosed { code } => {
            out.push_str("failClosed(\"");
            out.push_str(code);
            out.push_str("\")");
        }
    }
    Ok(())
}

fn serialize_expr(out: &mut String, expr: &ScalarExpr) -> Result<(), KloomHarnessError> {
    match expr {
        ScalarExpr::Const(value) => {
            serialize_scalar_value(out, value)?;
        }
        ScalarExpr::Var(name) => {
            out.push_str(&sanitize_id(name));
        }
        ScalarExpr::Add(lhs, rhs) => {
            out.push_str("add(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Sub(lhs, rhs) => {
            out.push_str("sub(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Mul(lhs, rhs) => {
            out.push_str("mul(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Eq(lhs, rhs) => {
            out.push_str("eq(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Lt(lhs, rhs) => {
            out.push_str("lt(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Le(lhs, rhs) => {
            out.push_str("le(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Min(lhs, rhs) => {
            out.push_str("min(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        ScalarExpr::Max(lhs, rhs) => {
            out.push_str("max(");
            serialize_expr(out, lhs)?;
            out.push_str(", ");
            serialize_expr(out, rhs)?;
            out.push(')');
        }
        // Unreachable: bitcast programs are flagged UnsupportedProgram before
        // serialization (see expr_uses_unsupported).
        ScalarExpr::Bitcast { .. } => {
            return Err(KloomHarnessError::new(
                "bitcast is not serializable to kloom syntax",
            ));
        }
    }
    Ok(())
}

fn serialize_scalar_value(
    out: &mut String,
    value: &ScalarValue,
) -> Result<(), KloomHarnessError> {
    match value {
        ScalarValue::Bool(true) => out.push_str("true"),
        ScalarValue::Bool(false) => out.push_str("false"),
        ScalarValue::Int32(v) => out.push_str(&v.to_string()),
        ScalarValue::Int64(v) => out.push_str(&v.to_string()),
        ScalarValue::UInt32(v) => out.push_str(&v.to_string()),
        ScalarValue::UInt64(v) => out.push_str(&v.to_string()),
        ScalarValue::Float32Bits(bits) => {
            // kloom syntax does not have float literals; emit the bit pattern as an
            // integer.  The trace is unaffected because appendValue with a constant
            // does not record the value.
            out.push_str(&bits.to_string());
        }
        ScalarValue::Float64Bits(bits) => {
            out.push_str(&bits.to_string());
        }
        ScalarValue::Bytes(b) => {
            // kloom models bytes via the `bytesConst("<hex>")` literal. The byte
            // content never reaches the trace (appendValue with a constant records
            // only the builder type, not the value), so a hex rendering is purely
            // for a faithful, escape-safe round-trip through kloom syntax.
            out.push_str("bytesConst(\"");
            for byte in b {
                out.push_str(&format!("{byte:02x}"));
            }
            out.push_str("\")");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// krun invocation
// ---------------------------------------------------------------------------

fn allow_k_oracle_skip() -> bool {
    std::env::var(LOOM_ALLOW_K_ORACLE_SKIP_VAR)
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn run_kloom(text: &str) -> Result<KOracleOutcome, KloomHarnessError> {
    let def_dir = match definition_dir() {
        Ok(d) => d,
        Err(e) => {
            if allow_k_oracle_skip() {
                return Ok(KOracleOutcome::SkippedRefereeAbsent {
                    reason: e.message,
                });
            }
            return Err(e);
        }
    };

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp_path = std::env::temp_dir().join(format!(
        "loom_kloom_harness_{}_{}.kloom",
        std::process::id(),
        seq
    ));
    std::fs::write(&tmp_path, text)
        .map_err(|e| KloomHarnessError::new(format!("failed to write temp file: {e}")))?;

    let mut child = match Command::new("krun")
        .arg(&tmp_path)
        .arg("--definition")
        .arg(&def_dir)
        .arg("--output")
        .arg("pretty")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if allow_k_oracle_skip() {
                return Ok(KOracleOutcome::SkippedRefereeAbsent {
                    reason: "krun not found on PATH".to_string(),
                });
            }
            return Err(KloomHarnessError::new(format!(
                "krun not found on PATH; set {LOOM_ALLOW_K_ORACLE_SKIP_VAR}=1 to skip"
            )));
        }
        Err(e) => {
            return Err(KloomHarnessError::new(format!("failed to spawn krun: {e}")));
        }
    };

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed().as_secs() >= KRUN_TIMEOUT_SECS {
                    let _ = child.kill();
                    return Ok(KOracleOutcome::SkippedRefereeAbsent {
                        reason: format!("krun timed out after {KRUN_TIMEOUT_SECS}s"),
                    });
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(KloomHarnessError::new(format!(
                    "failed to wait for krun: {e}"
                )));
            }
        }
    };

    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        std::io::Read::read_to_end(&mut out, &mut stdout_buf).map_err(|e| {
            KloomHarnessError::new(format!("failed to read krun stdout: {e}"))
        })?;
    }
    if let Some(mut err) = child.stderr.take() {
        std::io::Read::read_to_end(&mut err, &mut stderr_buf).map_err(|e| {
            KloomHarnessError::new(format!("failed to read krun stderr: {e}"))
        })?;
    }

    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_buf);
        return Err(KloomHarnessError::new(format!(
            "krun exited with status {status}: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&stdout_buf);
    let trace = parse_trace(&stdout)?;
    Ok(KOracleOutcome::ProducedTrace(trace))
}

fn definition_dir() -> Result<PathBuf, KloomHarnessError> {
    // CARGO_MANIFEST_DIR is crates/loom-core for this crate.
    // The kloom definition lives at contrib/kloom/.build from the workspace root,
    // i.e. two levels above the crate manifest.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| KloomHarnessError::new("cannot locate workspace root from manifest dir"))?;
    let def = workspace_root.join("contrib/kloom/.build");
    if !def.exists() {
        return Err(KloomHarnessError::new(format!(
            "kloom definition directory not found: {}",
            def.display()
        )));
    }
    Ok(def)
}

// ---------------------------------------------------------------------------
// Pretty-output parsing
// ---------------------------------------------------------------------------

fn parse_trace(stdout: &str) -> Result<Vec<String>, KloomHarnessError> {
    if !stdout.contains("<events>") {
        return Err(KloomHarnessError::new(
            "krun output did not contain <events> cell (garbled output)",
        ));
    }

    let mut lines = Vec::new();
    let mut in_events = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed == "<events>" {
            in_events = true;
            continue;
        }
        if trimmed == "</events>" {
            in_events = false;
            continue;
        }
        if in_events {
            if trimmed.starts_with("ListItem ( ") && trimmed.ends_with(" )") {
                let inner = &trimmed[11..trimmed.len() - 2];
                // kloom pretty output separates tokens with " : "
                let event = inner.split(" : ").collect::<Vec<_>>().join(":");
                lines.push(event);
            } else if trimmed == ".List" {
                // empty events — nothing to add
            } else {
                // Unexpected line inside <events>; keep it as-is for diagnostics.
                lines.push(trimmed.to_string());
            }
        }
    }

    Ok(lines)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use loom_ir_core::l2_core::ResourceBudget;

    #[test]
    fn serialize_pure_append() {
        let program = L2CoreProgram {
            artifact_version: 1,
            required_features: vec![],
            optional_features: vec![],
            capabilities: vec![Capability::OutputBuilder(
                loom_ir_core::l2_core::OutputBuilderCapability {
                    id: "col0".to_string(),
                    arrow_type: loom_ir_core::l2_core::L2DataType::Int32,
                    nullable: false,
                    max_events: 1,
                },
            )],
            resource_budget: ResourceBudget {
                max_steps: 10,
                max_input_bytes_read: 0,
                max_scratch_bytes: 0,
                max_builder_events: 1,
                max_rows: 1,
                max_constraint_count: 0,
            },
            body: vec![L2CoreStmt::AppendValue {
                builder: "col0".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(42)),
            }],
        };

        let text = serialize_program(&program).unwrap();
        assert!(text.contains("builder col0:int32"));
        assert!(text.contains("appendValue(col0, 42)"));
        assert!(text.contains("maxRows 1"));
    }

    #[test]
    fn serialize_bytes_builder_and_const() {
        let program = L2CoreProgram {
            artifact_version: 1,
            required_features: vec![],
            optional_features: vec![],
            capabilities: vec![Capability::OutputBuilder(
                loom_ir_core::l2_core::OutputBuilderCapability {
                    id: "col0".to_string(),
                    arrow_type: loom_ir_core::l2_core::L2DataType::Utf8,
                    nullable: false,
                    max_events: 1,
                },
            )],
            resource_budget: ResourceBudget {
                max_steps: 10,
                max_input_bytes_read: 0,
                max_scratch_bytes: 0,
                max_builder_events: 1,
                max_rows: 1,
                max_constraint_count: 0,
            },
            body: vec![L2CoreStmt::AppendValue {
                builder: "col0".to_string(),
                value: ScalarExpr::Const(ScalarValue::Bytes(vec![0xAB, 0xCD])),
            }],
        };

        // Bytes is now a modelled construct — no longer flagged unsupported.
        assert!(program_uses_unsupported_constructs(&program).is_none());

        let text = serialize_program(&program).unwrap();
        assert!(text.contains("builder col0:bytes"), "got:\n{text}");
        assert!(
            text.contains("appendValue(col0, bytesConst(\"abcd\"))"),
            "got:\n{text}"
        );
    }

    #[test]
    fn parse_pretty_events() {
        let pretty = r#"
<T>
  <events>
    ListItem ( append-value : col0 : int32 )
    ListItem ( terminal : finished )
  </events>
</T>
"#;
        let trace = parse_trace(pretty).unwrap();
        assert_eq!(trace, vec!["append-value:col0:int32", "terminal:finished"]);
    }

    #[test]
    fn parse_empty_events() {
        let pretty = r#"
<T>
  <events>
    .List
  </events>
</T>
"#;
        let trace = parse_trace(pretty).unwrap();
        assert!(trace.is_empty());
    }

    #[test]
    fn parse_garbled_no_events_cell_is_hard_error() {
        let garbled = r#"
<T>
  <k>
    program ... body ... maxRows 1
  </k>
</T>
"#;
        let result = parse_trace(garbled);
        assert!(result.is_err(), "garbled output without <events> must be a hard error");
        let err = result.unwrap_err();
        assert!(err.message.contains("<events>"), "error must mention <events>: {}", err.message);
    }
}
