//! General `L2Core` body interpreter — the single production decode engine.
//!
//! Phase: decode-chain (Plan 1). This module replaces the i32-only shortcut in
//! [`super::native_lowering::execute_supported_copy_i32`] with a *general*
//! statement-walking interpreter over [`L2CoreProgram`]. It is the one execution
//! engine wired into the sidecar FFI ([`crate::ffi::loom_sidecar_decode`]).
//!
//! # Design contract
//!
//! - **One engine, table-driven types.** Adding a new output type is a new
//!   dispatch arm in [`append_scalar`] — never a second copy of the control
//!   flow. The statement walker is type-agnostic.
//! - **Fail-closed.** Any unsupported statement shape, type, out-of-bounds read,
//!   or budget overrun returns a typed [`InterpError`]; the FFI caller maps that
//!   to a host-native fallback. The interpreter never panics on malformed input
//!   and never emits a partial array.
//! - **Verifier-gated.** Callers run `verify_l2_core_bytes` + the 4-gate routing
//!   *before* calling [`interpret_l2core`]. The interpreter assumes an accepted,
//!   feature-supported program but still bounds every loop by the program's
//!   [`ResourceBudget::max_steps`].
//!
//! # Execution model
//!
//! - Each [`Capability::OutputBuilder`] becomes one [`OutputBuilder`], keyed by
//!   its capability id, in declaration order (== output column order).
//! - Each [`Capability::InputSlice`] is bound to a host byte window supplied by
//!   the caller (keyed by capability id).
//! - `ForRange` / `CursorLoop` drive iteration; `ReadInput` reads `width` bytes
//!   at `offset` (both evaluated as byte positions *within the input slice*) and
//!   binds the raw bytes; `AppendValue` evaluates an expression and appends to a
//!   builder (raw bytes are decoded little-endian per the builder's type);
//!   `AppendNull` appends a null; `LetScalar` binds a scalar; `FailClosed`
//!   aborts.

use std::collections::HashMap;

use arrow_data::ArrayData;
use arrow_schema::{DataType, Field, Schema};

use loom_ir_core::l2_core::{
    Capability, L2CoreProgram, L2CoreStmt, L2DataType, ScalarExpr, ScalarType, ScalarValue,
};

use super::arrow_builder_output::OutputBuilder;

/// A typed error raised by the L2Core interpreter. Every variant is a
/// fail-closed condition: the FFI caller routes the artifact to a host-native
/// reader rather than emitting a partial result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpError {
    /// `ReadInput` referenced an input slice capability that was not provided.
    UnknownInputSlice(String),
    /// `AppendValue` / `AppendNull` referenced an unknown output builder.
    UnknownBuilder(String),
    /// A `Var` expression referenced an unbound scalar name.
    UnboundVar(String),
    /// A `ReadInput` window fell outside the supplied byte slice.
    ReadOutOfBounds {
        capability: String,
        offset: u64,
        width: u64,
        available: usize,
    },
    /// An expression could not be evaluated to the required shape.
    UnsupportedExpr(&'static str),
    /// A value could not be appended to a builder of the given type.
    TypeMismatch {
        builder_type: L2DataType,
        detail: &'static str,
    },
    /// The program exceeded its declared `max_steps` budget (loop guard).
    StepBudgetExceeded(u64),
    /// The program executed a `FailClosed` statement.
    FailClosed(String),
    /// The program declared no output builders (nothing to decode).
    NoOutputBuilders,
    /// Two output builders shared a capability id.
    DuplicateBuilder(String),
    /// Final array assembly produced inconsistent row counts across columns.
    RaggedColumns,
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpError::UnknownInputSlice(id) => {
                write!(f, "read from unknown input slice capability '{id}'")
            }
            InterpError::UnknownBuilder(id) => {
                write!(f, "append to unknown output builder '{id}'")
            }
            InterpError::UnboundVar(name) => write!(f, "unbound scalar variable '{name}'"),
            InterpError::ReadOutOfBounds {
                capability,
                offset,
                width,
                available,
            } => write!(
                f,
                "read of {width} bytes at offset {offset} exceeds input slice '{capability}' ({available} bytes)"
            ),
            InterpError::UnsupportedExpr(detail) => write!(f, "unsupported expression: {detail}"),
            InterpError::TypeMismatch {
                builder_type,
                detail,
            } => write!(f, "type mismatch for {builder_type:?} builder: {detail}"),
            InterpError::StepBudgetExceeded(max) => {
                write!(f, "step budget exceeded (max_steps = {max})")
            }
            InterpError::FailClosed(code) => write!(f, "fail-closed: {code}"),
            InterpError::NoOutputBuilders => write!(f, "program declares no output builders"),
            InterpError::DuplicateBuilder(id) => write!(f, "duplicate output builder id '{id}'"),
            InterpError::RaggedColumns => {
                write!(f, "output columns have inconsistent row counts")
            }
        }
    }
}

impl std::error::Error for InterpError {}

/// One decoded output column: the originating builder id, its Arrow type, and
/// the finished [`ArrayData`].
#[derive(Debug, Clone)]
pub struct DecodedColumn {
    pub builder_id: String,
    pub arrow_type: L2DataType,
    pub data: ArrayData,
}

/// Map an [`L2DataType`] to its Arrow [`DataType`].
pub fn l2_to_arrow_type(t: &L2DataType) -> DataType {
    match t {
        L2DataType::Boolean => DataType::Boolean,
        L2DataType::Int32 => DataType::Int32,
        L2DataType::Int64 => DataType::Int64,
        L2DataType::Float32 => DataType::Float32,
        L2DataType::Float64 => DataType::Float64,
        L2DataType::Utf8 => DataType::Utf8,
    }
}

/// Input byte windows for the program's input slice capabilities, keyed by
/// capability id. Each slice is the host byte window the capability declares
/// authority over; `ReadInput` offsets are relative to the start of the slice.
pub type InputSlices<'a> = HashMap<String, &'a [u8]>;

/// Interpret a verified, feature-supported [`L2CoreProgram`] against the given
/// input byte slices, returning one [`DecodedColumn`] per output builder in
/// declaration order.
///
/// The caller is responsible for verifier acceptance and routing gates; this
/// function only executes and bounds the program.
pub fn interpret_l2core(
    program: &L2CoreProgram,
    inputs: &InputSlices<'_>,
) -> Result<Vec<DecodedColumn>, InterpError> {
    // Initialise one output builder per OutputBuilder capability, in order.
    let mut builder_order: Vec<String> = Vec::new();
    let mut builder_types: HashMap<String, L2DataType> = HashMap::new();
    let mut builders: HashMap<String, OutputBuilder> = HashMap::new();

    for capability in &program.capabilities {
        if let Capability::OutputBuilder(b) = capability {
            if builders.contains_key(&b.id) {
                return Err(InterpError::DuplicateBuilder(b.id.clone()));
            }
            builder_order.push(b.id.clone());
            builder_types.insert(b.id.clone(), b.arrow_type.clone());
            builders.insert(b.id.clone(), OutputBuilder::new(&l2_to_arrow_type(&b.arrow_type)));
        }
    }

    if builder_order.is_empty() {
        return Err(InterpError::NoOutputBuilders);
    }

    let mut state = ExecState {
        inputs,
        builders: &mut builders,
        builder_types: &builder_types,
        env: HashMap::new(),
        steps: 0,
        max_steps: program.resource_budget.max_steps,
    };

    exec_block(&program.body, &mut state)?;

    // Finish builders in declaration order; verify equal row counts.
    let mut columns = Vec::with_capacity(builder_order.len());
    let mut row_count: Option<usize> = None;
    for id in &builder_order {
        let builder = builders.remove(id).expect("builder present");
        let arrow_type = builder_types.get(id).cloned().expect("type present");
        let data = builder.finish();
        match row_count {
            None => row_count = Some(data.len()),
            Some(rc) if rc != data.len() => return Err(InterpError::RaggedColumns),
            _ => {}
        }
        columns.push(DecodedColumn {
            builder_id: id.clone(),
            arrow_type,
            data,
        });
    }

    Ok(columns)
}

/// Build an Arrow [`Schema`] from decoded columns. Field name == builder id with
/// a leading `output_` stripped (the convention emitted by `decode_ir_gen`);
/// nullability is taken from the finished array's null count being permitted.
pub fn schema_from_columns(columns: &[DecodedColumn]) -> Schema {
    let fields: Vec<Field> = columns
        .iter()
        .map(|c| {
            let name = c
                .builder_id
                .strip_prefix("output_")
                .unwrap_or(&c.builder_id)
                .to_string();
            // Arrow array may carry nulls; mark field nullable if so.
            let nullable = c.data.null_count() > 0;
            Field::new(name, l2_to_arrow_type(&c.arrow_type), nullable)
        })
        .collect();
    Schema::new(fields)
}

// ---------------------------------------------------------------------------
// Execution internals
// ---------------------------------------------------------------------------

struct ExecState<'a, 'b> {
    inputs: &'a InputSlices<'a>,
    builders: &'b mut HashMap<String, OutputBuilder>,
    builder_types: &'b HashMap<String, L2DataType>,
    env: HashMap<String, ScalarValue>,
    steps: u64,
    max_steps: u64,
}

impl ExecState<'_, '_> {
    fn tick(&mut self) -> Result<(), InterpError> {
        self.steps = self.steps.saturating_add(1);
        if self.max_steps != 0 && self.steps > self.max_steps {
            return Err(InterpError::StepBudgetExceeded(self.max_steps));
        }
        Ok(())
    }
}

fn exec_block(body: &[L2CoreStmt], state: &mut ExecState) -> Result<(), InterpError> {
    for stmt in body {
        exec_stmt(stmt, state)?;
    }
    Ok(())
}

fn exec_stmt(stmt: &L2CoreStmt, state: &mut ExecState) -> Result<(), InterpError> {
    state.tick()?;
    match stmt {
        L2CoreStmt::ForRange {
            index,
            start,
            end,
            body,
        } => {
            let start = eval_int(start, &state.env)?;
            let end = eval_int(end, &state.env)?;
            let mut i = start;
            while i < end {
                state
                    .env
                    .insert(index.clone(), ScalarValue::UInt64(i as u64));
                exec_block(body, state)?;
                i += 1;
            }
            Ok(())
        }
        L2CoreStmt::CursorLoop {
            cursor,
            limit,
            progress,
            body,
        } => {
            // Bounded cursor: advance by `progress` (>=1) until `limit`.
            let limit = eval_int(limit, &state.env)?;
            let step = eval_int(progress, &state.env)?.max(1);
            let mut c = 0i128;
            while c < limit {
                state
                    .env
                    .insert(cursor.clone(), ScalarValue::UInt64(c as u64));
                exec_block(body, state)?;
                c += step;
            }
            Ok(())
        }
        L2CoreStmt::ReadInput {
            capability,
            offset,
            width,
            bind,
        } => {
            let slice = state
                .inputs
                .get(capability)
                .ok_or_else(|| InterpError::UnknownInputSlice(capability.clone()))?;
            let off = eval_int(offset, &state.env)?.max(0) as u64;
            let w = eval_int(width, &state.env)?.max(0) as u64;
            let end = off.saturating_add(w);
            if end as usize > slice.len() {
                return Err(InterpError::ReadOutOfBounds {
                    capability: capability.clone(),
                    offset: off,
                    width: w,
                    available: slice.len(),
                });
            }
            let bytes = slice[off as usize..end as usize].to_vec();
            state.env.insert(bind.clone(), ScalarValue::Bytes(bytes));
            Ok(())
        }
        L2CoreStmt::LetScalar { name, expr } => {
            let v = eval_scalar(expr, &state.env)?;
            state.env.insert(name.clone(), v);
            Ok(())
        }
        L2CoreStmt::AppendValue { builder, value } => {
            let v = eval_scalar(value, &state.env)?;
            let ty = state
                .builder_types
                .get(builder)
                .cloned()
                .ok_or_else(|| InterpError::UnknownBuilder(builder.clone()))?;
            let b = state
                .builders
                .get_mut(builder)
                .ok_or_else(|| InterpError::UnknownBuilder(builder.clone()))?;
            append_scalar(b, &ty, &v)
        }
        L2CoreStmt::AppendNull { builder } => {
            let b = state
                .builders
                .get_mut(builder)
                .ok_or_else(|| InterpError::UnknownBuilder(builder.clone()))?;
            b.append_null();
            Ok(())
        }
        L2CoreStmt::FailClosed { code } => Err(InterpError::FailClosed(code.clone())),
        L2CoreStmt::If {
            cond,
            then_body,
            else_body,
        } => {
            let truthy = match eval_scalar(cond, &state.env)? {
                ScalarValue::Bool(b) => b,
                other => scalar_as_int(&other)? != 0,
            };
            if truthy {
                exec_block(then_body, state)
            } else {
                exec_block(else_body, state)
            }
        }
    }
}

/// Append a scalar value to a typed builder, decoding raw bytes little-endian
/// when the value was produced by `ReadInput`. Adding a new output type means
/// adding an arm here — the control flow above is untouched.
fn append_scalar(
    builder: &mut OutputBuilder,
    ty: &L2DataType,
    value: &ScalarValue,
) -> Result<(), InterpError> {
    match ty {
        L2DataType::Boolean => {
            let v = match value {
                ScalarValue::Bool(b) => *b,
                ScalarValue::Bytes(b) if !b.is_empty() => b[0] != 0,
                _ => {
                    return Err(InterpError::TypeMismatch {
                        builder_type: ty.clone(),
                        detail: "expected bool or >=1 byte",
                    })
                }
            };
            builder.append_bool(v);
        }
        L2DataType::Int32 => {
            let v = scalar_to_i32(value).ok_or(InterpError::TypeMismatch {
                builder_type: ty.clone(),
                detail: "expected i32 or 4 bytes",
            })?;
            builder.append_i32(v);
        }
        L2DataType::Int64 => {
            let v = scalar_to_i64(value).ok_or(InterpError::TypeMismatch {
                builder_type: ty.clone(),
                detail: "expected i64 or 8 bytes",
            })?;
            builder.append_i64(v);
        }
        L2DataType::Float32 => {
            let bits = scalar_to_u32(value).ok_or(InterpError::TypeMismatch {
                builder_type: ty.clone(),
                detail: "expected f32 bits or 4 bytes",
            })?;
            builder.append_f32(f32::from_bits(bits));
        }
        L2DataType::Float64 => {
            let bits = scalar_to_u64(value).ok_or(InterpError::TypeMismatch {
                builder_type: ty.clone(),
                detail: "expected f64 bits or 8 bytes",
            })?;
            builder.append_f64(f64::from_bits(bits));
        }
        L2DataType::Utf8 => {
            let s = match value {
                ScalarValue::Bytes(b) => std::str::from_utf8(b).map_err(|_| {
                    InterpError::TypeMismatch {
                        builder_type: ty.clone(),
                        detail: "bytes are not valid UTF-8",
                    }
                })?,
                _ => {
                    return Err(InterpError::TypeMismatch {
                        builder_type: ty.clone(),
                        detail: "expected UTF-8 bytes",
                    })
                }
            };
            builder.append_string(s);
        }
    }
    Ok(())
}

fn scalar_to_i32(v: &ScalarValue) -> Option<i32> {
    match v {
        ScalarValue::Int32(x) => Some(*x),
        ScalarValue::UInt32(x) => Some(*x as i32),
        ScalarValue::Bytes(b) if b.len() == 4 => {
            Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        }
        _ => None,
    }
}

fn scalar_to_i64(v: &ScalarValue) -> Option<i64> {
    match v {
        ScalarValue::Int64(x) => Some(*x),
        ScalarValue::UInt64(x) => Some(*x as i64),
        ScalarValue::Int32(x) => Some(*x as i64),
        ScalarValue::Bytes(b) if b.len() == 8 => {
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            Some(i64::from_le_bytes(a))
        }
        _ => None,
    }
}

fn scalar_to_u32(v: &ScalarValue) -> Option<u32> {
    match v {
        ScalarValue::Float32Bits(x) | ScalarValue::UInt32(x) => Some(*x),
        ScalarValue::Bytes(b) if b.len() == 4 => {
            Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        }
        _ => None,
    }
}

fn scalar_to_u64(v: &ScalarValue) -> Option<u64> {
    match v {
        ScalarValue::Float64Bits(x) | ScalarValue::UInt64(x) => Some(*x),
        ScalarValue::Bytes(b) if b.len() == 8 => {
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            Some(u64::from_le_bytes(a))
        }
        _ => None,
    }
}

/// Evaluate a scalar expression to a [`ScalarValue`].
fn eval_scalar(
    expr: &ScalarExpr,
    env: &HashMap<String, ScalarValue>,
) -> Result<ScalarValue, InterpError> {
    match expr {
        ScalarExpr::Const(v) => Ok(v.clone()),
        ScalarExpr::Var(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| InterpError::UnboundVar(name.clone())),
        // Arithmetic and comparison evaluate over integers; result as UInt64 /
        // Bool. (Float arithmetic in the IR is out of scope for this engine.)
        ScalarExpr::Add(..)
        | ScalarExpr::Sub(..)
        | ScalarExpr::Mul(..)
        | ScalarExpr::Min(..)
        | ScalarExpr::Max(..) => {
            let v = eval_int(expr, env)?;
            Ok(ScalarValue::UInt64(v.max(0) as u64))
        }
        ScalarExpr::Eq(a, b) => Ok(ScalarValue::Bool(eval_int(a, env)? == eval_int(b, env)?)),
        ScalarExpr::Lt(a, b) => Ok(ScalarValue::Bool(eval_int(a, env)? < eval_int(b, env)?)),
        ScalarExpr::Le(a, b) => Ok(ScalarValue::Bool(eval_int(a, env)? <= eval_int(b, env)?)),
        ScalarExpr::Bitcast { target, value } => {
            let inner = eval_scalar(value, env)?;
            reinterpret_scalar(&inner, target)
        }
    }
}

/// Reinterpret a scalar's little-endian bytes as `target`. The inner value is
/// typically `Bytes` (from `ReadInput`); a typed scalar is also accepted.
fn reinterpret_scalar(value: &ScalarValue, target: &ScalarType) -> Result<ScalarValue, InterpError> {
    let bytes: Vec<u8> = match value {
        ScalarValue::Bytes(b) => b.clone(),
        ScalarValue::Bool(b) => vec![*b as u8],
        ScalarValue::Int32(x) => x.to_le_bytes().to_vec(),
        ScalarValue::UInt32(x) | ScalarValue::Float32Bits(x) => x.to_le_bytes().to_vec(),
        ScalarValue::Int64(x) => x.to_le_bytes().to_vec(),
        ScalarValue::UInt64(x) | ScalarValue::Float64Bits(x) => x.to_le_bytes().to_vec(),
    };
    let need = |n: usize| -> Result<(), InterpError> {
        if bytes.len() == n {
            Ok(())
        } else {
            Err(InterpError::UnsupportedExpr("bitcast width mismatch"))
        }
    };
    Ok(match target {
        ScalarType::Bool => {
            if bytes.is_empty() {
                return Err(InterpError::UnsupportedExpr("bitcast to bool needs >=1 byte"));
            }
            ScalarValue::Bool(bytes[0] != 0)
        }
        ScalarType::Int32 => {
            need(4)?;
            ScalarValue::Int32(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        ScalarType::UInt32 => {
            need(4)?;
            ScalarValue::UInt32(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        ScalarType::Float32 => {
            need(4)?;
            ScalarValue::Float32Bits(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        ScalarType::Int64 => {
            need(8)?;
            let mut a = [0u8; 8];
            a.copy_from_slice(&bytes);
            ScalarValue::Int64(i64::from_le_bytes(a))
        }
        ScalarType::UInt64 => {
            need(8)?;
            let mut a = [0u8; 8];
            a.copy_from_slice(&bytes);
            ScalarValue::UInt64(u64::from_le_bytes(a))
        }
        ScalarType::Float64 => {
            need(8)?;
            let mut a = [0u8; 8];
            a.copy_from_slice(&bytes);
            ScalarValue::Float64Bits(u64::from_le_bytes(a))
        }
        ScalarType::Bytes => ScalarValue::Bytes(bytes),
        ScalarType::RowIndex => {
            Err(InterpError::UnsupportedExpr("bitcast to RowIndex unsupported"))?
        }
    })
}

/// Evaluate a scalar expression to an integer (for loop bounds, offsets, widths).
fn eval_int(expr: &ScalarExpr, env: &HashMap<String, ScalarValue>) -> Result<i128, InterpError> {
    match expr {
        ScalarExpr::Const(v) => scalar_as_int(v),
        ScalarExpr::Var(name) => {
            let v = env
                .get(name)
                .ok_or_else(|| InterpError::UnboundVar(name.clone()))?;
            scalar_as_int(v)
        }
        ScalarExpr::Add(a, b) => Ok(eval_int(a, env)? + eval_int(b, env)?),
        ScalarExpr::Sub(a, b) => Ok(eval_int(a, env)? - eval_int(b, env)?),
        ScalarExpr::Mul(a, b) => Ok(eval_int(a, env)? * eval_int(b, env)?),
        ScalarExpr::Min(a, b) => Ok(eval_int(a, env)?.min(eval_int(b, env)?)),
        ScalarExpr::Max(a, b) => Ok(eval_int(a, env)?.max(eval_int(b, env)?)),
        ScalarExpr::Eq(..) | ScalarExpr::Lt(..) | ScalarExpr::Le(..) => {
            Err(InterpError::UnsupportedExpr("comparison used where integer required"))
        }
        ScalarExpr::Bitcast { .. } => {
            Err(InterpError::UnsupportedExpr("bitcast used where integer required"))
        }
    }
}

fn scalar_as_int(v: &ScalarValue) -> Result<i128, InterpError> {
    match v {
        ScalarValue::Int32(x) => Ok(*x as i128),
        ScalarValue::Int64(x) => Ok(*x as i128),
        ScalarValue::UInt32(x) => Ok(*x as i128),
        ScalarValue::UInt64(x) => Ok(*x as i128),
        ScalarValue::Bool(b) => Ok(*b as i128),
        _ => Err(InterpError::UnsupportedExpr("non-integer scalar in integer position")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interp::native_lowering::execute_supported_copy_i32;
    use arrow_array::Int32Array;
    use loom_ir_core::full_verifier::verify_l2_core;
    use loom_ir_core::l2_core::{
        InputSliceCapability, OutputBuilderCapability, ResourceBudget,
    };

    /// Legacy i32-copy program shape accepted by `check_lowering_support`:
    /// `ReadInput` offset is `index` (element-indexed), width 4. The legacy
    /// executor takes a pre-parsed `&[i32]` and ignores byte offsets.
    fn legacy_copy_program(input_id: &str, output_id: &str, rows: u64) -> L2CoreProgram {
        L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["l2core.copy.v0".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: input_id.to_string(),
                    offset: 0,
                    length: rows.saturating_mul(4),
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: output_id.to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                    max_events: rows,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(rows),
            body: vec![L2CoreStmt::ForRange {
                index: "i".to_string(),
                start: ScalarExpr::Const(ScalarValue::UInt64(0)),
                end: ScalarExpr::Const(ScalarValue::UInt64(rows)),
                body: vec![
                    L2CoreStmt::ReadInput {
                        capability: input_id.to_string(),
                        offset: ScalarExpr::Var("i".to_string()),
                        width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                        bind: "v".to_string(),
                    },
                    L2CoreStmt::AppendValue {
                        builder: output_id.to_string(),
                        value: ScalarExpr::Var("v".to_string()),
                    },
                ],
            }],
        }
    }

    /// The general interpreter reproduces the legacy `execute_supported_copy_i32`
    /// result for the i32 copy use-case (Plan 1 Task A: subsume the i32 shortcut).
    #[test]
    fn subsumes_legacy_execute_supported_copy_i32() {
        let vals = [11i32, 22, 33, 44];

        // Legacy engine: pre-parsed i32 slice, verifier-gated copy program.
        let legacy = legacy_copy_program("in", "output_col", vals.len() as u64);
        let report = verify_l2_core(&legacy);
        assert!(report.is_ok(), "legacy copy program must verify");
        let legacy_out =
            execute_supported_copy_i32(&legacy, &report, &vals).expect("legacy copy ok");
        assert_eq!(legacy_out, vals.to_vec());

        // General interpreter: byte-offset program over LE bytes.
        let program = i32_copy_program("in", "output_col", vals.len() as u64);
        let bytes = i32_le_bytes(&vals);
        let mut inputs = InputSlices::new();
        inputs.insert("in".to_string(), bytes.as_slice());
        let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
        let interp_out: Vec<i32> = Int32Array::from(columns[0].data.clone()).values().to_vec();

        // Both engines agree on the i32 copy semantics.
        assert_eq!(interp_out, legacy_out);
    }

    /// Build an i32-copy program: for i in 0..n { v = read(input, i*4, 4); append(out, v) }.
    fn i32_copy_program(input_id: &str, output_id: &str, rows: u64) -> L2CoreProgram {
        let body = vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(rows)),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: input_id.to_string(),
                    offset: ScalarExpr::Mul(
                        Box::new(ScalarExpr::Var("i".to_string())),
                        Box::new(ScalarExpr::Const(ScalarValue::UInt64(4))),
                    ),
                    width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                    bind: "v".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: output_id.to_string(),
                    value: ScalarExpr::Var("v".to_string()),
                },
            ],
        }];
        L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["l2core.copy.v0".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: input_id.to_string(),
                    offset: 0,
                    length: rows * 4,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: output_id.to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                    max_events: rows,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(rows),
            body,
        }
    }

    fn i32_le_bytes(vals: &[i32]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn i32_non_null_roundtrip() {
        let vals = [10i32, -20, 30, 0, 2_000_000];
        let program = i32_copy_program("in", "output_col", vals.len() as u64);
        let bytes = i32_le_bytes(&vals);
        let mut inputs = InputSlices::new();
        inputs.insert("in".to_string(), bytes.as_slice());

        let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].builder_id, "output_col");
        let arr = Int32Array::from(columns[0].data.clone());
        assert_eq!(arr.values(), &vals);
    }

    #[test]
    fn schema_strips_output_prefix() {
        let vals = [1i32, 2, 3];
        let program = i32_copy_program("in", "output_amount", vals.len() as u64);
        let bytes = i32_le_bytes(&vals);
        let mut inputs = InputSlices::new();
        inputs.insert("in".to_string(), bytes.as_slice());
        let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
        let schema = schema_from_columns(&columns);
        assert_eq!(schema.field(0).name(), "amount");
        assert_eq!(schema.field(0).data_type(), &DataType::Int32);
    }

    #[test]
    fn read_out_of_bounds_fails_closed() {
        let program = i32_copy_program("in", "output_col", 4);
        // Only 2 i32 worth of bytes for a 4-row program.
        let bytes = i32_le_bytes(&[1, 2]);
        let mut inputs = InputSlices::new();
        inputs.insert("in".to_string(), bytes.as_slice());
        let err = interpret_l2core(&program, &inputs).unwrap_err();
        assert!(matches!(err, InterpError::ReadOutOfBounds { .. }));
    }

    #[test]
    fn missing_input_slice_fails_closed() {
        let program = i32_copy_program("in", "output_col", 2);
        let inputs = InputSlices::new(); // no "in" provided
        let err = interpret_l2core(&program, &inputs).unwrap_err();
        assert!(matches!(err, InterpError::UnknownInputSlice(_)));
    }

    #[test]
    fn step_budget_guards_runaway() {
        let mut program = i32_copy_program("in", "output_col", 4);
        program.resource_budget.max_steps = 2; // far too small
        let bytes = i32_le_bytes(&[1, 2, 3, 4]);
        let mut inputs = InputSlices::new();
        inputs.insert("in".to_string(), bytes.as_slice());
        let err = interpret_l2core(&program, &inputs).unwrap_err();
        assert!(matches!(err, InterpError::StepBudgetExceeded(2)));
    }

    #[test]
    fn fail_closed_statement_aborts() {
        let mut program = i32_copy_program("in", "output_col", 1);
        program.body.push(L2CoreStmt::FailClosed {
            code: "deliberate".to_string(),
        });
        let bytes = i32_le_bytes(&[7]);
        let mut inputs = InputSlices::new();
        inputs.insert("in".to_string(), bytes.as_slice());
        let err = interpret_l2core(&program, &inputs).unwrap_err();
        assert_eq!(err, InterpError::FailClosed("deliberate".to_string()));
    }

    #[test]
    fn multi_column_independent_loops() {
        // Two columns, each its own ForRange + ReadInput + AppendValue.
        let rows = 3u64;
        let body = vec![
            L2CoreStmt::ForRange {
                index: "i".to_string(),
                start: ScalarExpr::Const(ScalarValue::UInt64(0)),
                end: ScalarExpr::Const(ScalarValue::UInt64(rows)),
                body: vec![
                    L2CoreStmt::ReadInput {
                        capability: "in_a".to_string(),
                        offset: ScalarExpr::Mul(
                            Box::new(ScalarExpr::Var("i".to_string())),
                            Box::new(ScalarExpr::Const(ScalarValue::UInt64(4))),
                        ),
                        width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                        bind: "v".to_string(),
                    },
                    L2CoreStmt::AppendValue {
                        builder: "output_a".to_string(),
                        value: ScalarExpr::Var("v".to_string()),
                    },
                ],
            },
            L2CoreStmt::ForRange {
                index: "j".to_string(),
                start: ScalarExpr::Const(ScalarValue::UInt64(0)),
                end: ScalarExpr::Const(ScalarValue::UInt64(rows)),
                body: vec![
                    L2CoreStmt::ReadInput {
                        capability: "in_b".to_string(),
                        offset: ScalarExpr::Mul(
                            Box::new(ScalarExpr::Var("j".to_string())),
                            Box::new(ScalarExpr::Const(ScalarValue::UInt64(4))),
                        ),
                        width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                        bind: "w".to_string(),
                    },
                    L2CoreStmt::AppendValue {
                        builder: "output_b".to_string(),
                        value: ScalarExpr::Var("w".to_string()),
                    },
                ],
            },
        ];
        let program = L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["l2core.copy.v0".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: "in_a".to_string(),
                    offset: 0,
                    length: rows * 4,
                }),
                Capability::InputSlice(InputSliceCapability {
                    id: "in_b".to_string(),
                    offset: 0,
                    length: rows * 4,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "output_a".to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                    max_events: rows,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "output_b".to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                    max_events: rows,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(rows * 2 + 8),
            body,
        };

        let a = i32_le_bytes(&[1, 2, 3]);
        let b = i32_le_bytes(&[40, 50, 60]);
        let mut inputs = InputSlices::new();
        inputs.insert("in_a".to_string(), a.as_slice());
        inputs.insert("in_b".to_string(), b.as_slice());

        let columns = interpret_l2core(&program, &inputs).expect("interpret ok");
        assert_eq!(columns.len(), 2);
        let col_a = Int32Array::from(columns[0].data.clone());
        let col_b = Int32Array::from(columns[1].data.clone());
        assert_eq!(col_a.values(), &[1, 2, 3]);
        assert_eq!(col_b.values(), &[40, 50, 60]);
    }
}
