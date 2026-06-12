//! Independent L2Core decode IR codec — Phase 49.
//!
//! This module provides a **standalone, self-describing binary serialization** for
//! [`L2CoreProgram`] and its transitive types. It is deliberately independent of
//! every container codec (`container_codec`, `table_codec`, `layout_codec`,
//! `arrow_semantic_codec`). The wire format carries its own magic and version so
//! that it can be hashed, verified, and distributed as a freestanding artifact.
//!
//! # Wire format
//!
//! ```text
//! [0..4]   magic    = b"L2IR"
//! [4..6]   version  = u16 LE  (currently 1)
//! [6..]    payload  = encoded L2CoreProgram
//! ```
//!
//! All multi-byte integers are **little-endian**. All length prefixes are `u32`.
//! Strings are `length(u32 LE) + UTF-8 bytes`. Vectors are `length(u32 LE) + elements`.
//! Enums are a single `u8` discriminant followed by variant payload.
//!
//! # Canonical / deterministic guarantees
//!
//! - Field order is fixed by the struct definition.
//! - String and vector iteration order is preserved exactly.
//! - `f32`/`f64` bit-patterns are written as raw `u32`/`u64` (no NaN canonicalisation).
//! - The format is architecture- and platform-independent (all sizes fixed-width).
//!
//! Because the encoding is deterministic, the content-hash over the encoded bytes
//! is a stable identity for the program.

use std::io::{Cursor, Read, Write};

use crate::l2_core::L2DataType;

use crate::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, ScalarType, ScalarValue, ScratchCapability,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Magic bytes for the independent L2Core IR codec.
pub const L2CORE_IR_MAGIC: &[u8; 4] = b"L2IR";

/// Current codec version. Bumped only when the wire format changes incompatibly.
pub const L2CORE_IR_VERSION: u16 = 1;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Failure modes for the L2Core IR codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L2CoreCodecError {
    /// Byte slice too short to contain a valid header or field.
    BufferTooShort,
    /// Magic bytes do not match `L2CORE_IR_MAGIC`.
    BadMagic,
    /// Codec version in the header is not supported.
    UnsupportedVersion { found: u16 },
    /// An enum discriminant is out of range.
    BadDiscriminant { context: String, value: u8 },
    /// A string is not valid UTF-8.
    InvalidUtf8,
    /// An [`L2DataType`] variant is not supported by this codec slice.
    UnsupportedDataType,
    /// A `ScalarValue` variant is not supported.
    UnsupportedScalarValue,
    /// Generic I/O error during encode/decode (should not happen in-memory).
    IoError(String),
}

impl std::fmt::Display for L2CoreCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            L2CoreCodecError::BufferTooShort => write!(f, "buffer too short"),
            L2CoreCodecError::BadMagic => write!(f, "bad magic bytes"),
            L2CoreCodecError::UnsupportedVersion { found } => {
                write!(f, "unsupported codec version {found}")
            }
            L2CoreCodecError::BadDiscriminant { context, value } => {
                write!(f, "bad discriminant {value} in {context}")
            }
            L2CoreCodecError::InvalidUtf8 => write!(f, "invalid UTF-8"),
            L2CoreCodecError::UnsupportedDataType => write!(f, "unsupported L2DataType"),
            L2CoreCodecError::UnsupportedScalarValue => write!(f, "unsupported ScalarValue"),
            L2CoreCodecError::IoError(msg) => write!(f, "io error: {msg}"),
        }
    }
}

impl std::error::Error for L2CoreCodecError {}

impl From<std::io::Error> for L2CoreCodecError {
    fn from(err: std::io::Error) -> Self {
        L2CoreCodecError::IoError(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encode an [`L2CoreProgram`] into its canonical wire representation.
///
/// The returned bytes include the `L2IR` magic + version header.
pub fn encode_l2core_program(program: &L2CoreProgram) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_all(L2CORE_IR_MAGIC).unwrap();
    write_u16(&mut buf, L2CORE_IR_VERSION);
    write_program(&mut buf, program);
    buf
}

/// Decode an [`L2CoreProgram`] from its canonical wire representation.
///
/// Validates magic, version, and every enum discriminant. Returns a typed error
/// on any malformed or truncated input — this is the fail-closed parse gate.
pub fn decode_l2core_program(bytes: &[u8]) -> Result<L2CoreProgram, L2CoreCodecError> {
    let mut cur = Cursor::new(bytes);

    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic).map_err(|_| L2CoreCodecError::BufferTooShort)?;
    if &magic != L2CORE_IR_MAGIC {
        return Err(L2CoreCodecError::BadMagic);
    }

    let version = read_u16(&mut cur)?;
    if version != L2CORE_IR_VERSION {
        return Err(L2CoreCodecError::UnsupportedVersion { found: version });
    }

    read_program(&mut cur)
}

/// Compute the BLAKE3-256 content-hash identity of a program.
///
/// The hash is taken over the **canonical encoded bytes** (including the magic
/// and version header), so the same program always yields the same digest.
/// The returned string is formatted as `blake3:<hex>`.
pub fn l2core_program_hash(program: &L2CoreProgram) -> String {
    let bytes = encode_l2core_program(program);
    let hash = blake3::hash(&bytes);
    format!("blake3:{hash}")
}

// ---------------------------------------------------------------------------
// Low-level primitives
// ---------------------------------------------------------------------------

pub(crate) fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

pub(crate) fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_string(buf: &mut Vec<u8>, s: &String) {
    let bytes = s.as_bytes();
    write_u32(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
}

pub(crate) fn write_vec<T>(buf: &mut Vec<u8>, items: &[T], mut write_item: impl FnMut(&mut Vec<u8>, &T)) {
    write_u32(buf, items.len() as u32);
    for item in items {
        write_item(buf, item);
    }
}

fn read_u8(cur: &mut Cursor<&[u8]>) -> Result<u8, L2CoreCodecError> {
    let mut b = [0u8; 1];
    cur.read_exact(&mut b).map_err(|_| L2CoreCodecError::BufferTooShort)?;
    Ok(b[0])
}

fn read_u16(cur: &mut Cursor<&[u8]>) -> Result<u16, L2CoreCodecError> {
    let mut b = [0u8; 2];
    cur.read_exact(&mut b).map_err(|_| L2CoreCodecError::BufferTooShort)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(cur: &mut Cursor<&[u8]>) -> Result<u32, L2CoreCodecError> {
    let mut b = [0u8; 4];
    cur.read_exact(&mut b).map_err(|_| L2CoreCodecError::BufferTooShort)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(cur: &mut Cursor<&[u8]>) -> Result<u64, L2CoreCodecError> {
    let mut b = [0u8; 8];
    cur.read_exact(&mut b).map_err(|_| L2CoreCodecError::BufferTooShort)?;
    Ok(u64::from_le_bytes(b))
}

fn read_string(cur: &mut Cursor<&[u8]>) -> Result<String, L2CoreCodecError> {
    let len = read_u32(cur)? as usize;
    let pos = cur.position() as usize;
    let bytes = cur.get_ref();
    if pos + len > bytes.len() {
        return Err(L2CoreCodecError::BufferTooShort);
    }
    let s = std::str::from_utf8(&bytes[pos..pos + len])
        .map_err(|_| L2CoreCodecError::InvalidUtf8)?;
    cur.set_position((pos + len) as u64);
    Ok(s.to_string())
}

fn read_vec<T>(
    cur: &mut Cursor<&[u8]>,
    mut read_item: impl FnMut(&mut Cursor<&[u8]>) -> Result<T, L2CoreCodecError>,
) -> Result<Vec<T>, L2CoreCodecError> {
    let len = read_u32(cur)? as usize;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(read_item(cur)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// L2CoreProgram and transitive types
// ---------------------------------------------------------------------------

fn write_program(buf: &mut Vec<u8>, program: &L2CoreProgram) {
    write_u16(buf, program.artifact_version);
    write_vec(buf, &program.required_features, write_string);
    write_vec(buf, &program.optional_features, write_string);
    write_vec(buf, &program.capabilities, write_capability);
    write_resource_budget(buf, &program.resource_budget);
    write_vec(buf, &program.body, write_stmt);
}

fn read_program(cur: &mut Cursor<&[u8]>) -> Result<L2CoreProgram, L2CoreCodecError> {
    Ok(L2CoreProgram {
        artifact_version: read_u16(cur)?,
        required_features: read_vec(cur, read_string)?,
        optional_features: read_vec(cur, read_string)?,
        capabilities: read_vec(cur, read_capability)?,
        resource_budget: read_resource_budget(cur)?,
        body: read_vec(cur, read_stmt)?,
    })
}

// Capability

fn write_capability(buf: &mut Vec<u8>, cap: &Capability) {
    match cap {
        Capability::InputSlice(c) => {
            write_u8(buf, 0);
            write_input_slice(buf, c);
        }
        Capability::Scratch(c) => {
            write_u8(buf, 1);
            write_scratch(buf, c);
        }
        Capability::OutputBuilder(c) => {
            write_u8(buf, 2);
            write_output_builder(buf, c);
        }
    }
}

fn read_capability(cur: &mut Cursor<&[u8]>) -> Result<Capability, L2CoreCodecError> {
    match read_u8(cur)? {
        0 => Ok(Capability::InputSlice(read_input_slice(cur)?)),
        1 => Ok(Capability::Scratch(read_scratch(cur)?)),
        2 => Ok(Capability::OutputBuilder(read_output_builder(cur)?)),
        v => Err(L2CoreCodecError::BadDiscriminant {
            context: "Capability".to_string(),
            value: v,
        }),
    }
}

fn write_input_slice(buf: &mut Vec<u8>, c: &InputSliceCapability) {
    write_string(buf, &c.id);
    write_u64(buf, c.offset);
    write_u64(buf, c.length);
}

fn read_input_slice(cur: &mut Cursor<&[u8]>) -> Result<InputSliceCapability, L2CoreCodecError> {
    Ok(InputSliceCapability {
        id: read_string(cur)?,
        offset: read_u64(cur)?,
        length: read_u64(cur)?,
    })
}

fn write_scratch(buf: &mut Vec<u8>, c: &ScratchCapability) {
    write_string(buf, &c.id);
    write_u64(buf, c.max_bytes);
}

fn read_scratch(cur: &mut Cursor<&[u8]>) -> Result<ScratchCapability, L2CoreCodecError> {
    Ok(ScratchCapability {
        id: read_string(cur)?,
        max_bytes: read_u64(cur)?,
    })
}

fn write_output_builder(buf: &mut Vec<u8>, c: &OutputBuilderCapability) {
    write_string(buf, &c.id);
    write_data_type(buf, &c.arrow_type);
    write_u8(buf, if c.nullable { 1 } else { 0 });
    write_u64(buf, c.max_events);
}

fn read_output_builder(
    cur: &mut Cursor<&[u8]>,
) -> Result<OutputBuilderCapability, L2CoreCodecError> {
    Ok(OutputBuilderCapability {
        id: read_string(cur)?,
        arrow_type: read_data_type(cur)?,
        nullable: read_u8(cur)? != 0,
        max_events: read_u64(cur)?,
    })
}

// L2DataType — narrow L2Core-supported subset

fn write_data_type(buf: &mut Vec<u8>, dt: &L2DataType) {
    let disc = match dt {
        L2DataType::Boolean => 0,
        L2DataType::Int32 => 1,
        L2DataType::Int64 => 2,
        L2DataType::Float32 => 3,
        L2DataType::Float64 => 4,
        L2DataType::Utf8 => 5,
    };
    write_u8(buf, disc);
}

fn read_data_type(cur: &mut Cursor<&[u8]>) -> Result<L2DataType, L2CoreCodecError> {
    match read_u8(cur)? {
        0 => Ok(L2DataType::Boolean),
        1 => Ok(L2DataType::Int32),
        2 => Ok(L2DataType::Int64),
        3 => Ok(L2DataType::Float32),
        4 => Ok(L2DataType::Float64),
        5 => Ok(L2DataType::Utf8),
        v => Err(L2CoreCodecError::BadDiscriminant {
            context: "L2DataType".to_string(),
            value: v,
        }),
    }
}

// ResourceBudget

fn write_resource_budget(buf: &mut Vec<u8>, rb: &ResourceBudget) {
    write_u64(buf, rb.max_steps);
    write_u64(buf, rb.max_input_bytes_read);
    write_u64(buf, rb.max_scratch_bytes);
    write_u64(buf, rb.max_builder_events);
    write_u64(buf, rb.max_rows);
    write_u64(buf, rb.max_constraint_count);
}

fn read_resource_budget(cur: &mut Cursor<&[u8]>) -> Result<ResourceBudget, L2CoreCodecError> {
    Ok(ResourceBudget {
        max_steps: read_u64(cur)?,
        max_input_bytes_read: read_u64(cur)?,
        max_scratch_bytes: read_u64(cur)?,
        max_builder_events: read_u64(cur)?,
        max_rows: read_u64(cur)?,
        max_constraint_count: read_u64(cur)?,
    })
}

// ScalarType

#[allow(dead_code)]
fn write_scalar_type(buf: &mut Vec<u8>, st: &ScalarType) {
    let disc = match st {
        ScalarType::Bool => 0,
        ScalarType::Int32 => 1,
        ScalarType::Int64 => 2,
        ScalarType::Float32 => 3,
        ScalarType::Float64 => 4,
        ScalarType::UInt32 => 5,
        ScalarType::UInt64 => 6,
        ScalarType::Bytes => 7,
        ScalarType::RowIndex => 8,
    };
    write_u8(buf, disc);
}

#[allow(dead_code)]
fn read_scalar_type(cur: &mut Cursor<&[u8]>) -> Result<ScalarType, L2CoreCodecError> {
    match read_u8(cur)? {
        0 => Ok(ScalarType::Bool),
        1 => Ok(ScalarType::Int32),
        2 => Ok(ScalarType::Int64),
        3 => Ok(ScalarType::Float32),
        4 => Ok(ScalarType::Float64),
        5 => Ok(ScalarType::UInt32),
        6 => Ok(ScalarType::UInt64),
        7 => Ok(ScalarType::Bytes),
        8 => Ok(ScalarType::RowIndex),
        v => Err(L2CoreCodecError::BadDiscriminant {
            context: "ScalarType".to_string(),
            value: v,
        }),
    }
}

// ScalarValue

fn write_scalar_value(buf: &mut Vec<u8>, sv: &ScalarValue) {
    match sv {
        ScalarValue::Bool(v) => {
            write_u8(buf, 0);
            write_u8(buf, if *v { 1 } else { 0 });
        }
        ScalarValue::Int32(v) => {
            write_u8(buf, 1);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        ScalarValue::Int64(v) => {
            write_u8(buf, 2);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        ScalarValue::Float32Bits(v) => {
            write_u8(buf, 3);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        ScalarValue::Float64Bits(v) => {
            write_u8(buf, 4);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        ScalarValue::UInt32(v) => {
            write_u8(buf, 5);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        ScalarValue::UInt64(v) => {
            write_u8(buf, 6);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        ScalarValue::Bytes(v) => {
            write_u8(buf, 7);
            write_u32(buf, v.len() as u32);
            buf.extend_from_slice(v);
        }
    }
}

fn read_scalar_value(cur: &mut Cursor<&[u8]>) -> Result<ScalarValue, L2CoreCodecError> {
    let mut b4 = [0u8; 4];
    let mut b8 = [0u8; 8];

    Ok(match read_u8(cur)? {
        0 => ScalarValue::Bool(read_u8(cur)? != 0),
        1 => {
            cur.read_exact(&mut b4).map_err(|_| L2CoreCodecError::BufferTooShort)?;
            ScalarValue::Int32(i32::from_le_bytes(b4))
        }
        2 => {
            cur.read_exact(&mut b8).map_err(|_| L2CoreCodecError::BufferTooShort)?;
            ScalarValue::Int64(i64::from_le_bytes(b8))
        }
        3 => {
            cur.read_exact(&mut b4).map_err(|_| L2CoreCodecError::BufferTooShort)?;
            ScalarValue::Float32Bits(u32::from_le_bytes(b4))
        }
        4 => {
            cur.read_exact(&mut b8).map_err(|_| L2CoreCodecError::BufferTooShort)?;
            ScalarValue::Float64Bits(u64::from_le_bytes(b8))
        }
        5 => {
            cur.read_exact(&mut b4).map_err(|_| L2CoreCodecError::BufferTooShort)?;
            ScalarValue::UInt32(u32::from_le_bytes(b4))
        }
        6 => {
            cur.read_exact(&mut b8).map_err(|_| L2CoreCodecError::BufferTooShort)?;
            ScalarValue::UInt64(u64::from_le_bytes(b8))
        }
        7 => {
            let len = read_u32(cur)? as usize;
            let pos = cur.position() as usize;
            let bytes = cur.get_ref();
            if pos + len > bytes.len() {
                return Err(L2CoreCodecError::BufferTooShort);
            }
            let v = bytes[pos..pos + len].to_vec();
            cur.set_position((pos + len) as u64);
            ScalarValue::Bytes(v)
        }
        v => {
            return Err(L2CoreCodecError::BadDiscriminant {
                context: "ScalarValue".to_string(),
                value: v,
            })
        }
    })
}

// ScalarExpr

fn write_scalar_expr(buf: &mut Vec<u8>, expr: &ScalarExpr) {
    match expr {
        ScalarExpr::Const(v) => {
            write_u8(buf, 0);
            write_scalar_value(buf, v);
        }
        ScalarExpr::Var(name) => {
            write_u8(buf, 1);
            write_string(buf, name);
        }
        ScalarExpr::Add(l, r) => {
            write_u8(buf, 2);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Sub(l, r) => {
            write_u8(buf, 3);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Mul(l, r) => {
            write_u8(buf, 4);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Min(l, r) => {
            write_u8(buf, 5);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Max(l, r) => {
            write_u8(buf, 6);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Eq(l, r) => {
            write_u8(buf, 7);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Lt(l, r) => {
            write_u8(buf, 8);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Le(l, r) => {
            write_u8(buf, 9);
            write_scalar_expr(buf, l);
            write_scalar_expr(buf, r);
        }
        ScalarExpr::Bitcast { target, value } => {
            write_u8(buf, 10);
            write_scalar_type(buf, target);
            write_scalar_expr(buf, value);
        }
    }
}

fn read_scalar_expr(cur: &mut Cursor<&[u8]>) -> Result<ScalarExpr, L2CoreCodecError> {
    Ok(match read_u8(cur)? {
        0 => ScalarExpr::Const(read_scalar_value(cur)?),
        1 => ScalarExpr::Var(read_string(cur)?),
        2 => ScalarExpr::Add(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        3 => ScalarExpr::Sub(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        4 => ScalarExpr::Mul(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        5 => ScalarExpr::Min(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        6 => ScalarExpr::Max(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        7 => ScalarExpr::Eq(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        8 => ScalarExpr::Lt(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        9 => ScalarExpr::Le(Box::new(read_scalar_expr(cur)?), Box::new(read_scalar_expr(cur)?)),
        10 => {
            let target = read_scalar_type(cur)?;
            ScalarExpr::Bitcast {
                target,
                value: Box::new(read_scalar_expr(cur)?),
            }
        }
        v => {
            return Err(L2CoreCodecError::BadDiscriminant {
                context: "ScalarExpr".to_string(),
                value: v,
            })
        }
    })
}

// L2CoreStmt

fn write_stmt(buf: &mut Vec<u8>, stmt: &L2CoreStmt) {
    match stmt {
        L2CoreStmt::ForRange { index, start, end, body } => {
            write_u8(buf, 0);
            write_string(buf, index);
            write_scalar_expr(buf, start);
            write_scalar_expr(buf, end);
            write_vec(buf, body, write_stmt);
        }
        L2CoreStmt::CursorLoop { cursor, limit, progress, body } => {
            write_u8(buf, 1);
            write_string(buf, cursor);
            write_scalar_expr(buf, limit);
            write_scalar_expr(buf, progress);
            write_vec(buf, body, write_stmt);
        }
        L2CoreStmt::ReadInput { capability, offset, width, bind } => {
            write_u8(buf, 2);
            write_string(buf, capability);
            write_scalar_expr(buf, offset);
            write_scalar_expr(buf, width);
            write_string(buf, bind);
        }
        L2CoreStmt::LetScalar { name, expr } => {
            write_u8(buf, 3);
            write_string(buf, name);
            write_scalar_expr(buf, expr);
        }
        L2CoreStmt::AppendValue { builder, value } => {
            write_u8(buf, 4);
            write_string(buf, builder);
            write_scalar_expr(buf, value);
        }
        L2CoreStmt::AppendNull { builder } => {
            write_u8(buf, 5);
            write_string(buf, builder);
        }
        L2CoreStmt::FailClosed { code } => {
            write_u8(buf, 6);
            write_string(buf, code);
        }
    }
}

fn read_stmt(cur: &mut Cursor<&[u8]>) -> Result<L2CoreStmt, L2CoreCodecError> {
    Ok(match read_u8(cur)? {
        0 => L2CoreStmt::ForRange {
            index: read_string(cur)?,
            start: read_scalar_expr(cur)?,
            end: read_scalar_expr(cur)?,
            body: read_vec(cur, read_stmt)?,
        },
        1 => L2CoreStmt::CursorLoop {
            cursor: read_string(cur)?,
            limit: read_scalar_expr(cur)?,
            progress: read_scalar_expr(cur)?,
            body: read_vec(cur, read_stmt)?,
        },
        2 => L2CoreStmt::ReadInput {
            capability: read_string(cur)?,
            offset: read_scalar_expr(cur)?,
            width: read_scalar_expr(cur)?,
            bind: read_string(cur)?,
        },
        3 => L2CoreStmt::LetScalar {
            name: read_string(cur)?,
            expr: read_scalar_expr(cur)?,
        },
        4 => L2CoreStmt::AppendValue {
            builder: read_string(cur)?,
            value: read_scalar_expr(cur)?,
        },
        5 => L2CoreStmt::AppendNull {
            builder: read_string(cur)?,
        },
        6 => L2CoreStmt::FailClosed {
            code: read_string(cur)?,
        },
        v => {
            return Err(L2CoreCodecError::BadDiscriminant {
                context: "L2CoreStmt".to_string(),
                value: v,
            })
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::l2_core::{OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue};
use crate::l2_core::L2DataType;

    fn minimal_program() -> L2CoreProgram {
        L2CoreProgram {
            artifact_version: 1,
            required_features: vec!["feat-a".to_string()],
            optional_features: vec!["feat-b".to_string()],
            capabilities: vec![Capability::OutputBuilder(OutputBuilderCapability {
                id: "out0".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: 10,
            })],
            resource_budget: ResourceBudget {
                max_steps: 100,
                max_input_bytes_read: 0,
                max_scratch_bytes: 0,
                max_builder_events: 10,
                max_rows: 10,
                max_constraint_count: 64,
            },
            body: vec![L2CoreStmt::AppendValue {
                builder: "out0".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(42)),
            }],
        }
    }

    #[test]
    fn roundtrip_minimal() {
        let original = minimal_program();
        let bytes = encode_l2core_program(&original);
        let decoded = decode_l2core_program(&bytes).expect("decode must succeed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn reencode_stability() {
        let original = minimal_program();
        let bytes1 = encode_l2core_program(&original);
        let decoded = decode_l2core_program(&bytes1).unwrap();
        let bytes2 = encode_l2core_program(&decoded);
        assert_eq!(bytes1, bytes2, "encode-decode-reencode must be byte-identical");
    }

    #[test]
    fn hash_stability() {
        let p = minimal_program();
        let h1 = l2core_program_hash(&p);
        let h2 = l2core_program_hash(&p);
        assert_eq!(h1, h2, "same program must yield same hash");
        assert!(h1.starts_with("blake3:"));
    }

    #[test]
    fn hash_distinguishes_different_programs() {
        let p1 = minimal_program();
        let mut p2 = minimal_program();
        p2.artifact_version = 2;
        assert_ne!(
            l2core_program_hash(&p1),
            l2core_program_hash(&p2),
            "different programs must yield different hashes"
        );
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bad = encode_l2core_program(&minimal_program());
        bad[0] = b'X';
        assert_eq!(
            decode_l2core_program(&bad),
            Err(L2CoreCodecError::BadMagic)
        );
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut bytes = encode_l2core_program(&minimal_program());
        bytes[5] = 0xFF; // version byte (little-endian u16 at offset 4)
        assert!(
            matches!(
                decode_l2core_program(&bytes),
                Err(L2CoreCodecError::UnsupportedVersion { .. })
            ),
            "unexpected version must be rejected"
        );
    }

    #[test]
    fn truncated_payload_rejected() {
        let bytes = encode_l2core_program(&minimal_program());
        let truncated = &bytes[..8]; // magic + version only
        assert_eq!(
            decode_l2core_program(truncated),
            Err(L2CoreCodecError::BufferTooShort)
        );
    }

    #[test]
    fn bad_discriminant_rejected() {
        // Construct minimal bad payload manually.
        let mut bad = Vec::new();
        bad.write_all(L2CORE_IR_MAGIC).unwrap();
        write_u16(&mut bad, L2CORE_IR_VERSION);
        write_u16(&mut bad, 1); // artifact_version
        write_u32(&mut bad, 0); // required_features len
        write_u32(&mut bad, 0); // optional_features len
        write_u32(&mut bad, 1); // capabilities len
        write_u8(&mut bad, 99); // bad Capability discriminant
        assert!(
            matches!(
                decode_l2core_program(&bad),
                Err(L2CoreCodecError::BadDiscriminant { context, .. }) if context == "Capability"
            ),
            "bad capability discriminant must be rejected"
        );
    }

    #[test]
    fn complex_program_roundtrip() {
        let program = L2CoreProgram {
            artifact_version: 3,
            required_features: vec!["simd".to_string(), "wide".to_string()],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: "input0".to_string(),
                    offset: 0,
                    length: 1024,
                }),
                Capability::Scratch(ScratchCapability {
                    id: "scratch0".to_string(),
                    max_bytes: 256,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "out0".to_string(),
                    arrow_type: L2DataType::Float64,
                    nullable: true,
                    max_events: 100,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(100),
            body: vec![
                L2CoreStmt::LetScalar {
                    name: "x".to_string(),
                    expr: ScalarExpr::Add(
                        Box::new(ScalarExpr::Const(ScalarValue::UInt64(1))),
                        Box::new(ScalarExpr::Var("y".to_string())),
                    ),
                },
                L2CoreStmt::ForRange {
                    index: "i".to_string(),
                    start: ScalarExpr::Const(ScalarValue::UInt64(0)),
                    end: ScalarExpr::Const(ScalarValue::UInt64(10)),
                    body: vec![
                        L2CoreStmt::ReadInput {
                            capability: "input0".to_string(),
                            offset: ScalarExpr::Var("i".to_string()),
                            width: ScalarExpr::Const(ScalarValue::UInt64(8)),
                            bind: "chunk".to_string(),
                        },
                        L2CoreStmt::AppendValue {
                            builder: "out0".to_string(),
                            value: ScalarExpr::Min(
                                Box::new(ScalarExpr::Var("chunk".to_string())),
                                Box::new(ScalarExpr::Const(ScalarValue::Float64Bits(1.0f64.to_bits()))),
                            ),
                        },
                        L2CoreStmt::AppendNull {
                            builder: "out0".to_string(),
                        },
                    ],
                },
                L2CoreStmt::FailClosed {
                    code: "UNREACHABLE".to_string(),
                },
            ],
        };
        let bytes = encode_l2core_program(&program);
        let decoded = decode_l2core_program(&bytes).expect("decode must succeed");
        assert_eq!(program, decoded);
        let re_bytes = encode_l2core_program(&decoded);
        assert_eq!(bytes, re_bytes);
    }

    #[test]
    fn diverse_programs_no_hash_collisions() {
        // Generate a small equivalence-class corpus inline and assert all hashes are unique.
        let mut programs: Vec<L2CoreProgram> = Vec::new();
        let base_types = vec![
            L2DataType::Int32,
            L2DataType::Int64,
            L2DataType::Float32,
            L2DataType::Float64,
            L2DataType::Boolean,
            L2DataType::Utf8,
        ];

        for (idx, arrow_type) in base_types.iter().enumerate() {
            for nullable in [false, true] {
                for depth in 0..=2 {
                    let mut body = Vec::new();
                    let expr = mk_expr(depth, arrow_type);
                    body.push(L2CoreStmt::AppendValue {
                        builder: "out0".to_string(),
                        value: expr,
                    });
                    if nullable {
                        body.push(L2CoreStmt::AppendNull {
                            builder: "out0".to_string(),
                        });
                    }
                    programs.push(L2CoreProgram {
                        artifact_version: 1,
                        required_features: if idx % 2 == 0 {
                            vec!["simd".to_string()]
                        } else {
                            vec![]
                        },
                        optional_features: vec![],
                        capabilities: vec![Capability::OutputBuilder(OutputBuilderCapability {
                            id: "out0".to_string(),
                            arrow_type: arrow_type.clone(),
                            nullable,
                            max_events: 10,
                        })],
                        resource_budget: ResourceBudget::bounded_rows(10),
                        body,
                    });
                }
            }
        }

        // Add programs with loops and input capabilities.
        programs.push(L2CoreProgram {
            artifact_version: 1,
            required_features: vec![],
            optional_features: vec![],
            capabilities: vec![
                Capability::InputSlice(InputSliceCapability {
                    id: "inp0".to_string(),
                    offset: 0,
                    length: 256,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "out0".to_string(),
                    arrow_type: L2DataType::Int32,
                    nullable: false,
                    max_events: 4,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(4),
            body: vec![L2CoreStmt::ForRange {
                index: "i".to_string(),
                start: ScalarExpr::Const(ScalarValue::UInt64(0)),
                end: ScalarExpr::Const(ScalarValue::UInt64(4)),
                body: vec![L2CoreStmt::ReadInput {
                    capability: "inp0".to_string(),
                    offset: ScalarExpr::Var("i".to_string()),
                    width: ScalarExpr::Const(ScalarValue::UInt64(4)),
                    bind: "v".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: "out0".to_string(),
                    value: ScalarExpr::Var("v".to_string()),
                }],
            }],
        });

        programs.push(L2CoreProgram {
            artifact_version: 1,
            required_features: vec![],
            optional_features: vec![],
            capabilities: vec![
                Capability::Scratch(ScratchCapability {
                    id: "scratch0".to_string(),
                    max_bytes: 128,
                }),
                Capability::OutputBuilder(OutputBuilderCapability {
                    id: "out0".to_string(),
                    arrow_type: L2DataType::Int64,
                    nullable: false,
                    max_events: 4,
                }),
            ],
            resource_budget: ResourceBudget::bounded_rows(4),
            body: vec![L2CoreStmt::CursorLoop {
                cursor: "c".to_string(),
                limit: ScalarExpr::Const(ScalarValue::UInt64(4)),
                progress: ScalarExpr::Add(
                    Box::new(ScalarExpr::Var("c".to_string())),
                    Box::new(ScalarExpr::Const(ScalarValue::UInt64(1))),
                ),
                body: vec![L2CoreStmt::AppendValue {
                    builder: "out0".to_string(),
                    value: ScalarExpr::Var("c".to_string()),
                }],
            }],
        });

        let mut hashes = std::collections::HashSet::new();
        for (idx, prog) in programs.iter().enumerate() {
            let bytes = encode_l2core_program(prog);
            let decoded = decode_l2core_program(&bytes).expect("decode must succeed");
            assert_eq!(*prog, decoded, "roundtrip failed for program {idx}");
            let hash = l2core_program_hash(prog);
            assert!(
                hashes.insert(hash.clone()),
                "hash collision at program {idx}: {hash}"
            );
        }
        assert!(
            hashes.len() > 10,
            "corpus must contain more than 10 distinct programs"
        );
    }

    fn mk_expr(depth: usize, arrow_type: &L2DataType) -> ScalarExpr {
        if depth == 0 {
            return match arrow_type {
                L2DataType::Boolean => ScalarExpr::Const(ScalarValue::Bool(true)),
                L2DataType::Int32 => ScalarExpr::Const(ScalarValue::Int32(42)),
                L2DataType::Int64 => ScalarExpr::Const(ScalarValue::Int64(42)),
                L2DataType::Float32 => ScalarExpr::Const(ScalarValue::Float32Bits(3.14f32.to_bits())),
                L2DataType::Float64 => ScalarExpr::Const(ScalarValue::Float64Bits(3.14f64.to_bits())),
                L2DataType::Utf8 => ScalarExpr::Const(ScalarValue::Bytes(vec![0xAB, 0xCD])),
            };
        }
        let inner = mk_expr(depth - 1, arrow_type);
        match depth % 4 {
            0 => ScalarExpr::Add(Box::new(inner.clone()), Box::new(inner)),
            1 => ScalarExpr::Sub(Box::new(inner.clone()), Box::new(inner)),
            2 => ScalarExpr::Mul(Box::new(inner.clone()), Box::new(inner)),
            _ => ScalarExpr::Min(Box::new(inner.clone()), Box::new(inner)),
        }
    }
}
