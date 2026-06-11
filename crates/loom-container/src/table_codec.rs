//! Table-shaped MVP0 payload codec.
//!
//! `LMT1` payloads compose the existing one-column `LMP1` layout payloads
//! without changing the single-column format. This keeps the Phase 5/7 release
//! gate stable while allowing Phase 8 to represent table-shaped output.

use std::collections::HashSet;

use arrow_data::ArrayData;

use loom_ir_core::error::LoomDecodeError;
use crate::l1_model::{decode_layout_to_array_data, LayoutDescription};
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::layout_codec::{decode_layout_payload, encode_layout_payload};
use crate::verifier::verify_table;

const MAGIC: &[u8; 4] = b"LMT1";
const VERSION: u16 = 1;

#[derive(Debug, Clone)]
pub struct TableDescription {
    pub row_count: usize,
    pub columns: Vec<TableColumn>,
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub name: String,
    pub layout: LayoutDescription,
}

impl TableDescription {
    pub fn validate(&self) -> Result<(), LoomDecodeError> {
        if self.columns.is_empty() {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "table has no columns",
            ));
        }
        let mut names = HashSet::new();
        for column in &self.columns {
            if column.name.is_empty() {
                return Err(LoomDecodeError::MalformedLayoutPayload("empty column name"));
            }
            if !names.insert(column.name.as_str()) {
                return Err(LoomDecodeError::MalformedLayoutPayload(
                    "duplicate column name",
                ));
            }
            if column.layout.row_count != self.row_count {
                return Err(LoomDecodeError::MalformedLayoutPayload(
                    "table column row count mismatch",
                ));
            }
        }
        Ok(())
    }
}

pub fn encode_table_payload(table: &TableDescription) -> Result<Vec<u8>, LoomDecodeError> {
    table.validate()?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(table.row_count as u64).to_le_bytes());
    out.extend_from_slice(&(table.columns.len() as u64).to_le_bytes());
    for column in &table.columns {
        write_bytes(&mut out, column.name.as_bytes());
        let payload = encode_layout_payload(&column.layout);
        write_bytes(&mut out, &payload);
    }
    Ok(out)
}

pub fn decode_table_payload(bytes: &[u8]) -> Result<TableDescription, LoomDecodeError> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_array::<4>()?;
    if &magic != MAGIC {
        return Err(LoomDecodeError::MalformedLayoutPayload("wrong table magic"));
    }
    let version = reader.read_u16()?;
    if version != VERSION {
        return Err(LoomDecodeError::MalformedLayoutPayload(
            "unsupported table version",
        ));
    }
    let row_count = reader.read_u64_as_usize("table row count")?;
    let column_count = reader.read_u64_as_usize("table column count")?;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        let name_bytes = reader.read_len_prefixed_bytes()?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| LoomDecodeError::MalformedLayoutPayload("column name utf8"))?
            .to_string();
        let payload = reader.read_len_prefixed_bytes()?;
        let layout = decode_layout_payload(payload)?;
        columns.push(TableColumn { name, layout });
    }
    reader.finish()?;
    Ok(TableDescription { row_count, columns })
}

pub fn is_table_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

pub fn decode_table_to_array_data(
    table: &TableDescription,
    registry: &L2KernelRegistry,
) -> Result<Vec<ArrayData>, LoomDecodeError> {
    let report = verify_table(table, registry);
    if let Some(err) = report.first_error() {
        return Err(err);
    }
    let mut arrays = Vec::with_capacity(table.columns.len());
    for column in &table.columns {
        let data = decode_layout_to_array_data(&column.layout, registry)?;
        if data.len() != table.row_count {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "decoded column row count mismatch",
            ));
        }
        arrays.push(data);
    }
    Ok(arrays)
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], LoomDecodeError> {
        let bytes = self.read_bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn read_u16(&mut self) -> Result<u16, LoomDecodeError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, LoomDecodeError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_u64_as_usize(&mut self, field: &'static str) -> Result<usize, LoomDecodeError> {
        usize::try_from(self.read_u64()?)
            .map_err(|_| LoomDecodeError::MalformedLayoutPayload(field))
    }

    fn read_len_prefixed_bytes(&mut self) -> Result<&'a [u8], LoomDecodeError> {
        let len = self.read_u64_as_usize("byte length")?;
        self.read_bytes(len)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedLayoutPayload(
                "truncated table payload",
            ))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "truncated table payload",
            ));
        }
        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), LoomDecodeError> {
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(LoomDecodeError::MalformedLayoutPayload(
                "trailing table bytes",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use arrow_schema::DataType;

    use super::*;
    use crate::l1_model::LayoutNode;

    fn raw_i32(name: &str, row_count: usize) -> TableColumn {
        TableColumn {
            name: name.to_string(),
            layout: LayoutDescription {
                data_type: DataType::Int32,
                root: LayoutNode::Raw {
                    data: vec![1, 0, 0, 0, 2, 0, 0, 0],
                    elem_size: 4,
                    count: row_count,
                },
                row_count,
            },
        }
    }

    fn raw_bool(name: &str, row_count: usize) -> TableColumn {
        TableColumn {
            name: name.to_string(),
            layout: LayoutDescription {
                data_type: DataType::Boolean,
                root: LayoutNode::Raw {
                    data: vec![1, 0],
                    elem_size: 1,
                    count: row_count,
                },
                row_count,
            },
        }
    }

    #[test]
    fn table_payload_roundtrip() {
        let table = TableDescription {
            row_count: 2,
            columns: vec![raw_i32("id", 2), raw_bool("flag", 2)],
        };
        let payload = encode_table_payload(&table).unwrap();
        assert!(is_table_payload(&payload));
        let decoded = decode_table_payload(&payload).unwrap();
        assert_eq!(decoded.row_count, 2);
        assert_eq!(decoded.columns.len(), 2);
        assert_eq!(decoded.columns[0].name, "id");
        assert_eq!(decoded.columns[1].name, "flag");
    }

    #[test]
    fn rejects_duplicate_names() {
        let table = TableDescription {
            row_count: 2,
            columns: vec![raw_i32("id", 2), raw_bool("id", 2)],
        };
        assert!(encode_table_payload(&table).is_err());
    }

    #[test]
    fn rejects_empty_names() {
        let table = TableDescription {
            row_count: 2,
            columns: vec![raw_i32("", 2)],
        };
        assert!(encode_table_payload(&table).is_err());
    }

    #[test]
    fn rejects_row_count_mismatch() {
        let table = TableDescription {
            row_count: 3,
            columns: vec![raw_i32("id", 2)],
        };
        assert!(encode_table_payload(&table).is_err());
    }
}
