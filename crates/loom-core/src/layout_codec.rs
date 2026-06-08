//! Minimal MVP0 layout payload codec.
//!
//! This is a checked test/fixture format for carrying one-column
//! [`LayoutDescription`] values across the FFI boundary. It is intentionally
//! small and deterministic, not a public long-term storage format.

use arrow_schema::DataType;

use crate::error::LoomDecodeError;
use crate::l1_model::{LayoutDescription, LayoutNode};

const MAGIC: &[u8; 4] = b"LMP1";
const VERSION: u16 = 1;

const DTYPE_BOOL: u8 = 1;
const DTYPE_I32: u8 = 2;
const DTYPE_I64: u8 = 3;
const DTYPE_UTF8: u8 = 4;
const DTYPE_F32: u8 = 5;
const DTYPE_F64: u8 = 6;

const NODE_RAW: u8 = 0;
const NODE_BITPACK: u8 = 1;
const NODE_FOR: u8 = 2;
const NODE_DICT: u8 = 3;
const NODE_RUN_END: u8 = 4;
const NODE_KERNEL_ESCAPE: u8 = 5;

const FLAG_VALIDITY: u8 = 1;
const FLAG_ALL_NULL: u8 = 2;

/// Encode a one-column layout payload for MVP0 tests/fixtures.
pub fn encode_layout_payload(desc: &LayoutDescription) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(encode_data_type(&desc.data_type));
    out.extend_from_slice(&(desc.row_count as u64).to_le_bytes());
    encode_node(&desc.root, &mut out);
    out
}

/// Decode a checked one-column MVP0 layout payload.
pub fn decode_layout_payload(bytes: &[u8]) -> Result<LayoutDescription, LoomDecodeError> {
    let mut reader = PayloadReader::new(bytes);

    let magic = reader.read_array::<4>()?;
    if &magic != MAGIC {
        return Err(LoomDecodeError::MalformedLayoutPayload("wrong magic"));
    }
    let version = reader.read_u16()?;
    if version != VERSION {
        return Err(LoomDecodeError::MalformedLayoutPayload(
            "unsupported version",
        ));
    }
    let data_type = decode_data_type(reader.read_u8()?)?;
    let row_count = reader.read_u64_as_usize("row count")?;
    let root = decode_node(&mut reader)?;
    reader.finish()?;

    Ok(LayoutDescription {
        data_type,
        root,
        row_count,
    })
}

fn encode_data_type(data_type: &DataType) -> u8 {
    match data_type {
        DataType::Boolean => DTYPE_BOOL,
        DataType::Int32 => DTYPE_I32,
        DataType::Int64 => DTYPE_I64,
        DataType::Utf8 => DTYPE_UTF8,
        DataType::Float32 => DTYPE_F32,
        DataType::Float64 => DTYPE_F64,
        other => panic!("layout codec unsupported DataType {other:?}"),
    }
}

fn decode_data_type(tag: u8) -> Result<DataType, LoomDecodeError> {
    match tag {
        DTYPE_BOOL => Ok(DataType::Boolean),
        DTYPE_I32 => Ok(DataType::Int32),
        DTYPE_I64 => Ok(DataType::Int64),
        DTYPE_UTF8 => Ok(DataType::Utf8),
        DTYPE_F32 => Ok(DataType::Float32),
        DTYPE_F64 => Ok(DataType::Float64),
        _ => Err(LoomDecodeError::MalformedLayoutPayload(
            "unknown data type tag",
        )),
    }
}

fn encode_node(node: &LayoutNode, out: &mut Vec<u8>) {
    match node {
        LayoutNode::Raw {
            data,
            elem_size,
            count,
        } => {
            out.push(NODE_RAW);
            out.push(*elem_size);
            out.extend_from_slice(&(*count as u64).to_le_bytes());
            write_bytes(out, data);
        }
        LayoutNode::BitPack {
            values_buf,
            bit_width,
            offset,
            count,
            validity,
            all_null,
        } => {
            out.push(NODE_BITPACK);
            out.push(*bit_width);
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&(*count as u64).to_le_bytes());
            let mut flags = 0u8;
            if validity.is_some() {
                flags |= FLAG_VALIDITY;
            }
            if *all_null {
                flags |= FLAG_ALL_NULL;
            }
            out.push(flags);
            write_bytes(out, values_buf);
            if let Some(validity) = validity {
                write_bool_vec(out, validity);
            }
        }
        LayoutNode::FrameOfReference { reference, inner } => {
            out.push(NODE_FOR);
            out.extend_from_slice(&reference.to_le_bytes());
            encode_node(inner, out);
        }
        LayoutNode::Dictionary { codes, values } => {
            out.push(NODE_DICT);
            encode_node(codes, out);
            encode_node(values, out);
        }
        LayoutNode::RunEnd {
            run_ends,
            values,
            count,
        } => {
            out.push(NODE_RUN_END);
            out.extend_from_slice(&(*count as u64).to_le_bytes());
            encode_node(run_ends, out);
            encode_node(values, out);
        }
        LayoutNode::KernelEscape {
            kernel_id,
            params,
            count,
        } => {
            out.push(NODE_KERNEL_ESCAPE);
            out.extend_from_slice(&kernel_id.to_le_bytes());
            out.extend_from_slice(&(*count as u64).to_le_bytes());
            write_bytes(out, params);
        }
    }
}

fn decode_node(reader: &mut PayloadReader<'_>) -> Result<LayoutNode, LoomDecodeError> {
    match reader.read_u8()? {
        NODE_RAW => {
            let elem_size = reader.read_u8()?;
            let count = reader.read_u64_as_usize("raw count")?;
            let data = reader.read_len_prefixed_bytes()?.to_vec();
            Ok(LayoutNode::Raw {
                data,
                elem_size,
                count,
            })
        }
        NODE_BITPACK => {
            let bit_width = reader.read_u8()?;
            let offset = reader.read_u16()?;
            let count = reader.read_u64_as_usize("bitpack count")?;
            let flags = reader.read_u8()?;
            if flags & !(FLAG_VALIDITY | FLAG_ALL_NULL) != 0 {
                return Err(LoomDecodeError::MalformedLayoutPayload("unknown flags"));
            }
            let values_buf = reader.read_len_prefixed_bytes()?.to_vec();
            let validity = if flags & FLAG_VALIDITY != 0 {
                Some(reader.read_bool_vec(count)?)
            } else {
                None
            };
            Ok(LayoutNode::BitPack {
                values_buf,
                bit_width,
                offset,
                count,
                validity,
                all_null: flags & FLAG_ALL_NULL != 0,
            })
        }
        NODE_FOR => {
            let reference = i128::from_le_bytes(reader.read_array()?);
            let inner = decode_node(reader)?;
            Ok(LayoutNode::FrameOfReference {
                reference,
                inner: Box::new(inner),
            })
        }
        NODE_DICT => {
            let codes = decode_node(reader)?;
            let values = decode_node(reader)?;
            Ok(LayoutNode::Dictionary {
                codes: Box::new(codes),
                values: Box::new(values),
            })
        }
        NODE_RUN_END => {
            let count = reader.read_u64_as_usize("run-end count")?;
            let run_ends = decode_node(reader)?;
            let values = decode_node(reader)?;
            Ok(LayoutNode::RunEnd {
                run_ends: Box::new(run_ends),
                values: Box::new(values),
                count,
            })
        }
        NODE_KERNEL_ESCAPE => {
            let kernel_id = reader.read_u32()?;
            let count = reader.read_u64_as_usize("kernel count")?;
            let params = reader.read_len_prefixed_bytes()?.to_vec();
            Ok(LayoutNode::KernelEscape {
                kernel_id,
                params,
                count,
            })
        }
        _ => Err(LoomDecodeError::MalformedLayoutPayload(
            "unknown layout node tag",
        )),
    }
}

fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn write_bool_vec(out: &mut Vec<u8>, values: &[bool]) {
    out.extend_from_slice(&(values.len() as u64).to_le_bytes());
    out.extend(values.iter().map(|value| u8::from(*value)));
}

struct PayloadReader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], LoomDecodeError> {
        let bytes = self.read_bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn read_u8(&mut self) -> Result<u8, LoomDecodeError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, LoomDecodeError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, LoomDecodeError> {
        Ok(u32::from_le_bytes(self.read_array()?))
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

    fn read_bool_vec(&mut self, expected_len: usize) -> Result<Vec<bool>, LoomDecodeError> {
        let len = self.read_u64_as_usize("validity length")?;
        if len != expected_len {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "validity length mismatch",
            ));
        }
        let bytes = self.read_bytes(len)?;
        let mut out = Vec::with_capacity(len);
        for byte in bytes {
            match *byte {
                0 => out.push(false),
                1 => out.push(true),
                _ => {
                    return Err(LoomDecodeError::MalformedLayoutPayload(
                        "invalid boolean byte",
                    ));
                }
            }
        }
        Ok(out)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedLayoutPayload("truncated payload"))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedLayoutPayload("truncated payload"));
        }
        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), LoomDecodeError> {
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(LoomDecodeError::MalformedLayoutPayload("trailing bytes"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsst_params::FsstParams;

    #[test]
    fn roundtrip_raw_i32_payload() {
        let values = [1i32, -2, 3];
        let desc = LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Raw {
                data: values.iter().flat_map(|v| v.to_le_bytes()).collect(),
                elem_size: 4,
                count: values.len(),
            },
            row_count: values.len(),
        };

        let decoded =
            decode_layout_payload(&encode_layout_payload(&desc)).expect("payload should decode");

        assert_eq!(decoded.data_type, DataType::Int32);
        assert_eq!(decoded.row_count, 3);
        let LayoutNode::Raw {
            data,
            elem_size,
            count,
        } = decoded.root
        else {
            panic!("expected Raw node");
        };
        assert_eq!(elem_size, 4);
        assert_eq!(count, 3);
        assert_eq!(
            data,
            values
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn roundtrip_kernel_escape_payload() {
        let params = FsstParams {
            symbols: vec![],
            symbol_lengths: vec![],
            codes_offsets: vec![0],
            uncompressed_lengths: vec![],
            validity: None,
            codes_bytes: vec![],
        }
        .encode();
        let desc = LayoutDescription {
            data_type: DataType::Utf8,
            root: LayoutNode::KernelEscape {
                kernel_id: 0,
                params: params.clone(),
                count: 0,
            },
            row_count: 0,
        };

        let decoded =
            decode_layout_payload(&encode_layout_payload(&desc)).expect("payload should decode");

        assert_eq!(decoded.data_type, DataType::Utf8);
        let LayoutNode::KernelEscape {
            kernel_id,
            params: decoded_params,
            count,
        } = decoded.root
        else {
            panic!("expected KernelEscape node");
        };
        assert_eq!(kernel_id, 0);
        assert_eq!(count, 0);
        assert_eq!(decoded_params, params);
    }

    #[test]
    fn roundtrip_kernel_escape_float_payloads() {
        for data_type in [DataType::Float32, DataType::Float64] {
            let desc = LayoutDescription {
                data_type: data_type.clone(),
                root: LayoutNode::KernelEscape {
                    kernel_id: 1,
                    params: vec![1, 2, 3, 4],
                    count: 0,
                },
                row_count: 0,
            };

            let decoded = decode_layout_payload(&encode_layout_payload(&desc))
                .expect("payload should decode");

            assert_eq!(decoded.data_type, data_type);
            let LayoutNode::KernelEscape {
                kernel_id,
                params,
                count,
            } = decoded.root
            else {
                panic!("expected KernelEscape node");
            };
            assert_eq!(kernel_id, 1);
            assert_eq!(params, vec![1, 2, 3, 4]);
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn rejects_truncated_payload() {
        let desc = LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Raw {
                data: 1i32.to_le_bytes().to_vec(),
                elem_size: 4,
                count: 1,
            },
            row_count: 1,
        };
        let encoded = encode_layout_payload(&desc);
        let err = decode_layout_payload(&encoded[..encoded.len() - 1])
            .expect_err("truncated payload must be rejected");

        assert_eq!(
            err,
            LoomDecodeError::MalformedLayoutPayload("truncated payload")
        );
    }
}
