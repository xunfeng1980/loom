//! Auto-generate executable L2Core Decode IR from a Parquet schema (Plan 3).
//!
//! Reads a Parquet file's schema and row count, then emits a canonical L2Core
//! IR program with a **real decode body** that copies each supported column
//! value out of host data into a typed Arrow output builder. The generated IR
//! can be embedded as a sidecar and replayed by the L2Core interpreter for a
//! verified decode.
//!
//! # Host data layout contract
//!
//! The generated IR reads from a **raw column-major little-endian** host
//! buffer. For each supported column, in schema order:
//!   - **non-null** column → `rows * width` value bytes;
//!   - **nullable** column → `rows` validity bytes (1 = valid, 0 = null) then
//!     `rows * width` value bytes (placeholder bytes for null rows).
//!
//! [`parquet_to_raw_host`] produces exactly this layout from a Parquet file's
//! Arrow values, so generated-IR + interpreter reproduce the file's values.
//!
//! # Tiers
//!
//! - **Tier 1** — fixed-width non-null (Int32/Int64 direct, Float32/Float64/
//!   Boolean via `Bitcast`).
//! - **Tier 2** — nullable fixed-width (validity slice + `If` per row).
//! - Utf8 (Tier 3) and dictionary (Tier 4) are skipped here.
//!
//! Binding this layout to real Parquet *physical* column-chunk bytes is Plan 4.

use loom_ir_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarType, ScalarValue,
};
use loom_ir_core::sidecar::SidecarCodecError;
use std::path::Path;

/// Map an Arrow DataType to an L2DataType.
fn arrow_to_l2(dt: &arrow_schema::DataType) -> Option<L2DataType> {
    match dt {
        arrow_schema::DataType::Boolean => Some(L2DataType::Boolean),
        arrow_schema::DataType::Int32 => Some(L2DataType::Int32),
        arrow_schema::DataType::Int64 => Some(L2DataType::Int64),
        arrow_schema::DataType::Float32 => Some(L2DataType::Float32),
        arrow_schema::DataType::Float64 => Some(L2DataType::Float64),
        arrow_schema::DataType::Utf8 | arrow_schema::DataType::LargeUtf8 => Some(L2DataType::Utf8),
        _ => None,
    }
}

/// A fixed-width decodable column: its L2 type and per-row byte width. Returns
/// `None` for variable-length (Utf8 — Tier 3) and unsupported types.
fn fixed_width_column(field: &arrow_schema::Field) -> Option<(L2DataType, u64)> {
    match arrow_to_l2(field.data_type())? {
        L2DataType::Int32 => Some((L2DataType::Int32, 4)),
        L2DataType::Int64 => Some((L2DataType::Int64, 8)),
        L2DataType::Float32 => Some((L2DataType::Float32, 4)),
        L2DataType::Float64 => Some((L2DataType::Float64, 8)),
        L2DataType::Boolean => Some((L2DataType::Boolean, 1)),
        L2DataType::Utf8 => None,
    }
}

/// The `AppendValue` expression for a fixed-width value bind: integers append
/// the width-typed read directly; floats/Boolean reinterpret it via `Bitcast`
/// (the verifier infers a read type from byte width alone, which is ambiguous
/// for floats and not Bool-typed for a 1-byte read).
fn append_expr_for(l2_type: &L2DataType, bind: String) -> ScalarExpr {
    let var = ScalarExpr::Var(bind);
    match l2_type {
        L2DataType::Int32 | L2DataType::Int64 => var,
        L2DataType::Float32 => ScalarExpr::Bitcast {
            target: ScalarType::Float32,
            value: Box::new(var),
        },
        L2DataType::Float64 => ScalarExpr::Bitcast {
            target: ScalarType::Float64,
            value: Box::new(var),
        },
        L2DataType::Boolean => ScalarExpr::Bitcast {
            target: ScalarType::Bool,
            value: Box::new(var),
        },
        L2DataType::Utf8 => var, // unreachable for fixed-width
    }
}

/// Generate an executable L2Core IR program from a Parquet file's schema.
/// Returns `None` if no columns are decodable.
pub fn generate_decode_ir_from_parquet(
    path: &Path,
) -> Result<Option<L2CoreProgram>, SidecarCodecError> {
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    let file = File::open(path).map_err(|e| {
        SidecarCodecError::Malformed(format!("open parquet file {}: {e}", path.display()))
    })?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
        SidecarCodecError::Malformed(format!("parquet reader for {}: {e}", path.display()))
    })?;

    let schema = builder.schema().clone();
    let metadata = builder.metadata().clone();
    let total_rows = metadata.file_metadata().num_rows() as u64;
    let rows_expr = || ScalarExpr::Const(ScalarValue::UInt64(total_rows));

    let mut capabilities = Vec::new();
    let mut body: Vec<L2CoreStmt> = Vec::new();
    let mut offset: u64 = 0;
    let mut column_count: u64 = 0;

    for field in schema.fields() {
        let Some((l2_type, width)) = fixed_width_column(field) else {
            continue; // Skip Utf8 / unsupported.
        };
        let nullable = field.is_nullable();
        let col_name = field.name();
        let input_id = format!("input_{col_name}");
        let output_id = format!("output_{col_name}");
        let value_bind = format!("v_{col_name}");

        // The read of the row value, relative to the value slice window.
        let read_value = L2CoreStmt::ReadInput {
            capability: input_id.clone(),
            offset: ScalarExpr::Mul(
                Box::new(ScalarExpr::Var("i".to_string())),
                Box::new(ScalarExpr::Const(ScalarValue::UInt64(width))),
            ),
            width: ScalarExpr::Const(ScalarValue::UInt64(width)),
            bind: value_bind.clone(),
        };

        capabilities.push(Capability::OutputBuilder(OutputBuilderCapability {
            id: output_id.clone(),
            arrow_type: l2_type.clone(),
            nullable,
            max_events: total_rows,
        }));

        let loop_body: Vec<L2CoreStmt> = if nullable {
            // Validity slice precedes the value slice in the host layout.
            let valid_id = format!("valid_{col_name}");
            let valid_bind = format!("valid_{col_name}");
            capabilities.push(Capability::InputSlice(InputSliceCapability {
                id: valid_id.clone(),
                offset,
                length: total_rows,
            }));
            offset = offset.saturating_add(total_rows);
            capabilities.push(Capability::InputSlice(InputSliceCapability {
                id: input_id.clone(),
                offset,
                length: total_rows.saturating_mul(width),
            }));

            vec![
                L2CoreStmt::ReadInput {
                    capability: valid_id,
                    offset: ScalarExpr::Var("i".to_string()),
                    width: ScalarExpr::Const(ScalarValue::UInt64(1)),
                    bind: valid_bind.clone(),
                },
                read_value,
                L2CoreStmt::If {
                    cond: ScalarExpr::Bitcast {
                        target: ScalarType::Bool,
                        value: Box::new(ScalarExpr::Var(valid_bind)),
                    },
                    then_body: vec![L2CoreStmt::AppendValue {
                        builder: output_id.clone(),
                        value: append_expr_for(&l2_type, value_bind),
                    }],
                    else_body: vec![L2CoreStmt::AppendNull { builder: output_id }],
                },
            ]
        } else {
            capabilities.push(Capability::InputSlice(InputSliceCapability {
                id: input_id.clone(),
                offset,
                length: total_rows.saturating_mul(width),
            }));
            vec![
                read_value,
                L2CoreStmt::AppendValue {
                    builder: output_id,
                    value: append_expr_for(&l2_type, value_bind),
                },
            ]
        };

        offset = offset.saturating_add(total_rows.saturating_mul(width));

        body.push(L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: rows_expr(),
            body: loop_body,
        });
        column_count += 1;
    }

    if column_count == 0 {
        return Ok(None);
    }

    // Generous fail-closed budget: a handful of steps + appends per row/column.
    let events = total_rows.saturating_mul(column_count);
    let resource_budget = ResourceBudget {
        max_steps: events
            .saturating_mul(4)
            .saturating_add(column_count.saturating_mul(4))
            .saturating_add(32),
        max_input_bytes_read: offset,
        max_scratch_bytes: 0,
        max_builder_events: events.saturating_mul(2).saturating_add(column_count),
        max_rows: total_rows,
        max_constraint_count: 64,
    };

    let program = L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities,
        resource_budget,
        body,
    };

    Ok(Some(program))
}

/// Append one column's per-row value bytes (little-endian) to `out`. Null rows
/// emit placeholder bytes (the Arrow default at that slot) — they are never
/// appended by the IR.
fn pack_value_bytes(
    col: &dyn arrow_array::Array,
    l2_type: &L2DataType,
    out: &mut Vec<u8>,
) -> Result<(), SidecarCodecError> {
    use arrow_array::{BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array};
    match l2_type {
        L2DataType::Boolean => {
            let a = col
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| SidecarCodecError::Malformed("expected BooleanArray".into()))?;
            for i in 0..a.len() {
                out.push(if a.value(i) { 1u8 } else { 0u8 });
            }
        }
        L2DataType::Int32 => {
            let a = col
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or_else(|| SidecarCodecError::Malformed("expected Int32Array".into()))?;
            for i in 0..a.len() {
                out.extend_from_slice(&a.value(i).to_le_bytes());
            }
        }
        L2DataType::Int64 => {
            let a = col
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| SidecarCodecError::Malformed("expected Int64Array".into()))?;
            for i in 0..a.len() {
                out.extend_from_slice(&a.value(i).to_le_bytes());
            }
        }
        L2DataType::Float32 => {
            let a = col
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| SidecarCodecError::Malformed("expected Float32Array".into()))?;
            for i in 0..a.len() {
                out.extend_from_slice(&a.value(i).to_bits().to_le_bytes());
            }
        }
        L2DataType::Float64 => {
            let a = col
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| SidecarCodecError::Malformed("expected Float64Array".into()))?;
            for i in 0..a.len() {
                out.extend_from_slice(&a.value(i).to_bits().to_le_bytes());
            }
        }
        L2DataType::Utf8 => unreachable!("Utf8 is not a fixed-width column"),
    }
    Ok(())
}

/// Pack a Parquet file's decodable column values into the raw column-major
/// little-endian host buffer that [`generate_decode_ir_from_parquet`] reads.
/// Nullable columns are prefixed with a validity byte per row.
pub fn parquet_to_raw_host(path: &Path) -> Result<Vec<u8>, SidecarCodecError> {
    use arrow_array::Array;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::File;

    let file = File::open(path).map_err(|e| {
        SidecarCodecError::Malformed(format!("open parquet file {}: {e}", path.display()))
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| SidecarCodecError::Malformed(format!("parquet reader: {e}")))?
        .build()
        .map_err(|e| SidecarCodecError::Malformed(format!("parquet build: {e}")))?;

    let batches = reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| SidecarCodecError::Malformed(format!("parquet read: {e}")))?;

    let schema = match batches.first() {
        Some(b) => b.schema(),
        None => return Ok(Vec::new()),
    };

    let mut out = Vec::new();

    for (col_idx, field) in schema.fields().iter().enumerate() {
        let Some((l2_type, _width)) = fixed_width_column(field) else {
            continue;
        };
        // Nullable: validity bytes (all rows) precede value bytes.
        if field.is_nullable() {
            for batch in &batches {
                let col = batch.column(col_idx);
                for i in 0..col.len() {
                    out.push(if col.is_valid(i) { 1u8 } else { 0u8 });
                }
            }
        }
        for batch in &batches {
            pack_value_bytes(batch.column(col_idx).as_ref(), &l2_type, &mut out)?;
        }
    }

    Ok(out)
}
