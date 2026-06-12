//! Auto-generate executable L2Core Decode IR from a Parquet schema (Plan 3).
//!
//! Reads a Parquet file's schema and row count, then emits a canonical L2Core
//! IR program with a **real decode body** that copies each supported column
//! value out of host data into a typed Arrow output builder. The generated IR
//! can be embedded as a sidecar and replayed by the L2Core interpreter for a
//! verified decode.
//!
//! # Host data layout contract (Tier 1)
//!
//! The generated IR reads from a **raw column-major little-endian** host
//! buffer: each supported column's values are packed contiguously (`rows *
//! width` bytes), columns concatenated in schema order. Column `k`'s
//! `InputSlice` window is `[offset_k, offset_k + rows*width_k)` and the per-row
//! `ReadInput` offset within that window is `i * width`. [`parquet_to_raw_host`]
//! produces exactly this layout from a Parquet file's Arrow values, so
//! generated-IR + interpreter reproduce the file's values.
//!
//! Binding this layout to real Parquet *physical* column-chunk bytes (so no
//! transcode step is needed) is Plan 4. Variable-length (Utf8), nullable, and
//! dictionary columns are later tiers and are skipped here.

use loom_ir_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
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

/// Fixed-width byte size for a Tier 1 type in the raw host layout, or `None`
/// for variable-length types (Utf8 — handled in a later tier).
fn tier1_width(t: &L2DataType) -> Option<u64> {
    match t {
        L2DataType::Boolean => Some(1),
        L2DataType::Int32 | L2DataType::Float32 => Some(4),
        L2DataType::Int64 | L2DataType::Float64 => Some(8),
        L2DataType::Utf8 => None,
    }
}

/// Decide whether a field is a Tier 1 decodable column: non-nullable and
/// fixed-width. Returns its `(L2DataType, width)` when supported.
fn tier1_column(field: &arrow_schema::Field) -> Option<(L2DataType, u64)> {
    if field.is_nullable() {
        return None; // Tier 1 is non-null only (nullable is Tier 2).
    }
    let t = arrow_to_l2(field.data_type())?;
    let w = tier1_width(&t)?;
    Some((t, w))
}

/// Generate an executable L2Core IR program from a Parquet file's schema.
///
/// For each Tier 1 column (non-null fixed-width) emits:
/// - an `InputSlice` capability windowing that column's raw bytes,
/// - an `OutputBuilder` capability of the matching type,
/// - a `ForRange` loop that reads each row's value and appends it.
///
/// Returns `None` if no columns are Tier 1-supported.
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

    let mut capabilities = Vec::new();
    let mut body: Vec<L2CoreStmt> = Vec::new();
    let mut offset: u64 = 0;
    let mut column_count: u64 = 0;

    for field in schema.fields() {
        let Some((l2_type, width)) = tier1_column(field) else {
            continue; // Skip unsupported / nullable / variable-length columns.
        };
        let length = total_rows.saturating_mul(width);

        let col_name = field.name();
        let input_id = format!("input_{col_name}");
        let output_id = format!("output_{col_name}");
        let bind = format!("v_{col_name}");

        capabilities.push(Capability::InputSlice(InputSliceCapability {
            id: input_id.clone(),
            offset,
            length,
        }));
        capabilities.push(Capability::OutputBuilder(OutputBuilderCapability {
            id: output_id.clone(),
            arrow_type: l2_type,
            nullable: false,
            max_events: total_rows,
        }));

        // for i in 0..rows { v = read(input, i*width, width); append(output, v) }
        body.push(L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(total_rows)),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: input_id,
                    offset: ScalarExpr::Mul(
                        Box::new(ScalarExpr::Var("i".to_string())),
                        Box::new(ScalarExpr::Const(ScalarValue::UInt64(width))),
                    ),
                    width: ScalarExpr::Const(ScalarValue::UInt64(width)),
                    bind: bind.clone(),
                },
                L2CoreStmt::AppendValue {
                    builder: output_id,
                    value: ScalarExpr::Var(bind),
                },
            ],
        });

        offset = offset.saturating_add(length);
        column_count += 1;
    }

    if capabilities.is_empty() {
        return Ok(None);
    }

    // Budget the program for the real workload: one read + one append per row
    // per column, plus one loop-entry step per column.
    let events = total_rows.saturating_mul(column_count);
    let resource_budget = ResourceBudget {
        max_steps: events
            .saturating_mul(2)
            .saturating_add(column_count)
            .saturating_add(16),
        max_input_bytes_read: offset,
        max_scratch_bytes: 0,
        max_builder_events: events,
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

/// Pack a Parquet file's Tier 1 column values into the raw column-major
/// little-endian host buffer that [`generate_decode_ir_from_parquet`] reads.
///
/// Skips the same columns the IR generator skips (nullable / variable-length),
/// so the buffer's layout matches the generated `InputSlice` offsets exactly.
pub fn parquet_to_raw_host(path: &Path) -> Result<Vec<u8>, SidecarCodecError> {
    use arrow_array::{Array, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array};
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

    // Column-major: for each supported column, append all rows (across batches).
    for (col_idx, field) in schema.fields().iter().enumerate() {
        let Some((l2_type, _width)) = tier1_column(field) else {
            continue;
        };
        for batch in &batches {
            let col = batch.column(col_idx);
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
                L2DataType::Utf8 => unreachable!("Utf8 is not a Tier 1 column"),
            }
        }
    }

    Ok(out)
}
