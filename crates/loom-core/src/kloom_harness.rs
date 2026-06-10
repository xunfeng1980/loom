//! K Framework harness for L2Core program trace extraction.
//!
//! Serializes an [`L2CoreProgram`] to the kloom textual format, invokes `krun`,
//! and parses the pretty-printed output to extract trace events.
//!
//! Trust model: spec-oracle, offline/CI only, outside production TCB.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::l2_core::{
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

/// Extract the reference trace for a program by running it through the K
/// semantics (`krun`).
///
/// Returns the ordered list of trace event strings, e.g.:
/// `append-value:col0:int32`, `terminal:finished`, `fail-closed:missing-output-builder`.
pub fn kloom_trace_for_program(program: &L2CoreProgram) -> Result<Vec<String>, KloomHarnessError> {
    let text = serialize_program(program)?;
    run_kloom(&text)
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

fn arrow_type_to_l2ty(dt: &arrow_schema::DataType) -> Result<&'static str, KloomHarnessError> {
    match dt {
        arrow_schema::DataType::Int32 => Ok("int32"),
        arrow_schema::DataType::Int64 => Ok("int64"),
        arrow_schema::DataType::Float32 => Ok("float32"),
        arrow_schema::DataType::Float64 => Ok("float64"),
        arrow_schema::DataType::Boolean => Ok("bool"),
        other => Err(KloomHarnessError::new(format!(
            "unsupported Arrow type for kloom serialization: {other:?}"
        ))),
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
            // kloom v4 does not have min/max; lower to a placeholder.
            out.push('0');
            let _ = (lhs, rhs);
        }
        ScalarExpr::Max(lhs, rhs) => {
            out.push('0');
            let _ = (lhs, rhs);
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
            // Bytes are not representable as kloom constants; emit 0 as placeholder.
            let _ = b;
            out.push('0');
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// krun invocation
// ---------------------------------------------------------------------------

fn run_kloom(text: &str) -> Result<Vec<String>, KloomHarnessError> {
    let def_dir = definition_dir()?;

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

    let output = Command::new("krun")
        .arg(&tmp_path)
        .arg("--definition")
        .arg(&def_dir)
        .arg("--output")
        .arg("pretty")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| KloomHarnessError::new(format!("failed to spawn krun: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(KloomHarnessError::new(format!(
            "krun exited with status {}: {stderr}",
            output.status
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_trace(&stdout)
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
    use crate::l2_core::ResourceBudget;

    #[test]
    fn serialize_pure_append() {
        let program = L2CoreProgram {
            artifact_version: 1,
            required_features: vec![],
            optional_features: vec![],
            capabilities: vec![Capability::OutputBuilder(
                crate::l2_core::OutputBuilderCapability {
                    id: "col0".to_string(),
                    arrow_type: arrow_schema::DataType::Int32,
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
}
