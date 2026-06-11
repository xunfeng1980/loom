//! Human-readable MVP0 layout descriptor codec.
//!
//! The descriptor is intentionally scoped to MVP0. It mirrors
//! [`LayoutDescription`] as readable RON because the layout is a recursive enum
//! tree; RON keeps nested `Dictionary`, `RunEnd`, and `KernelEscape` nodes clear
//! without forcing table keys into a TOML shape.

use arrow_schema::DataType;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use loom_ir_core::error::LoomDecodeError;
use crate::l1_model::{LayoutDescription, LayoutNode};
use crate::layout_codec::{decode_layout_payload, encode_layout_payload};

const DESCRIPTOR_VERSION: u16 = 1;

/// Print a [`LayoutDescription`] as deterministic MVP0 descriptor text.
pub fn to_descriptor_text(desc: &LayoutDescription) -> Result<String, LoomDecodeError> {
    let descriptor = DescriptorFile::from_layout(desc)?;
    ron::ser::to_string_pretty(
        &descriptor,
        PrettyConfig::new()
            .depth_limit(64)
            .separate_tuple_members(true)
            .enumerate_arrays(true),
    )
    .map_err(|err| LoomDecodeError::MalformedDescriptor(err.to_string()))
}

/// Parse MVP0 descriptor text into a [`LayoutDescription`].
pub fn from_descriptor_text(input: &str) -> Result<LayoutDescription, LoomDecodeError> {
    let descriptor: DescriptorFile = ron::from_str(input)
        .map_err(|err| LoomDecodeError::MalformedDescriptor(err.to_string()))?;
    descriptor.into_layout()
}

/// Decode binary MVP0 payload bytes and print the equivalent descriptor text.
pub fn payload_to_descriptor_text(bytes: &[u8]) -> Result<String, LoomDecodeError> {
    let desc = decode_layout_payload(bytes)?;
    to_descriptor_text(&desc)
}

/// Parse descriptor text and encode it as the binary MVP0 payload format.
pub fn descriptor_text_to_payload(input: &str) -> Result<Vec<u8>, LoomDecodeError> {
    let desc = from_descriptor_text(input)?;
    Ok(encode_layout_payload(&desc))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DescriptorFile {
    version: u16,
    data_type: DescriptorDataType,
    row_count: usize,
    root: DescriptorNode,
}

impl DescriptorFile {
    fn from_layout(desc: &LayoutDescription) -> Result<Self, LoomDecodeError> {
        Ok(Self {
            version: DESCRIPTOR_VERSION,
            data_type: DescriptorDataType::from_arrow(&desc.data_type)?,
            row_count: desc.row_count,
            root: DescriptorNode::from_layout(&desc.root),
        })
    }

    fn into_layout(self) -> Result<LayoutDescription, LoomDecodeError> {
        if self.version != DESCRIPTOR_VERSION {
            return Err(LoomDecodeError::MalformedDescriptor(format!(
                "unsupported descriptor version {}",
                self.version
            )));
        }
        Ok(LayoutDescription {
            data_type: self.data_type.into_arrow(),
            root: self.root.into_layout()?,
            row_count: self.row_count,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum DescriptorDataType {
    Boolean,
    Int32,
    Int64,
    Float32,
    Float64,
    Utf8,
}

impl DescriptorDataType {
    fn from_arrow(data_type: &DataType) -> Result<Self, LoomDecodeError> {
        match data_type {
            DataType::Boolean => Ok(Self::Boolean),
            DataType::Int32 => Ok(Self::Int32),
            DataType::Int64 => Ok(Self::Int64),
            DataType::Float32 => Ok(Self::Float32),
            DataType::Float64 => Ok(Self::Float64),
            DataType::Utf8 => Ok(Self::Utf8),
            other => Err(LoomDecodeError::MalformedDescriptor(format!(
                "unsupported descriptor data type {other:?}"
            ))),
        }
    }

    fn into_arrow(self) -> DataType {
        match self {
            Self::Boolean => DataType::Boolean,
            Self::Int32 => DataType::Int32,
            Self::Int64 => DataType::Int64,
            Self::Float32 => DataType::Float32,
            Self::Float64 => DataType::Float64,
            Self::Utf8 => DataType::Utf8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum DescriptorNode {
    Raw {
        elem_size: u8,
        count: usize,
        data: Vec<u8>,
    },
    BitPack {
        bit_width: u8,
        offset: u16,
        count: usize,
        values_buf: Vec<u8>,
        validity: Option<Vec<bool>>,
        all_null: bool,
    },
    FrameOfReference {
        reference: String,
        inner: Box<DescriptorNode>,
    },
    Dictionary {
        codes: Box<DescriptorNode>,
        values: Box<DescriptorNode>,
    },
    RunEnd {
        count: usize,
        run_ends: Box<DescriptorNode>,
        values: Box<DescriptorNode>,
    },
    KernelEscape {
        kernel_id: u32,
        count: usize,
        params: Vec<u8>,
    },
}

impl DescriptorNode {
    fn from_layout(node: &LayoutNode) -> Self {
        match node {
            LayoutNode::Raw {
                data,
                elem_size,
                count,
            } => Self::Raw {
                elem_size: *elem_size,
                count: *count,
                data: data.clone(),
            },
            LayoutNode::BitPack {
                values_buf,
                bit_width,
                offset,
                count,
                validity,
                all_null,
            } => Self::BitPack {
                bit_width: *bit_width,
                offset: *offset,
                count: *count,
                values_buf: values_buf.clone(),
                validity: validity.clone(),
                all_null: *all_null,
            },
            LayoutNode::FrameOfReference { reference, inner } => Self::FrameOfReference {
                reference: reference.to_string(),
                inner: Box::new(Self::from_layout(inner)),
            },
            LayoutNode::Dictionary { codes, values } => Self::Dictionary {
                codes: Box::new(Self::from_layout(codes)),
                values: Box::new(Self::from_layout(values)),
            },
            LayoutNode::RunEnd {
                run_ends,
                values,
                count,
            } => Self::RunEnd {
                count: *count,
                run_ends: Box::new(Self::from_layout(run_ends)),
                values: Box::new(Self::from_layout(values)),
            },
            LayoutNode::KernelEscape {
                kernel_id,
                params,
                count,
            } => Self::KernelEscape {
                kernel_id: *kernel_id,
                count: *count,
                params: params.clone(),
            },
        }
    }

    fn into_layout(self) -> Result<LayoutNode, LoomDecodeError> {
        match self {
            Self::Raw {
                elem_size,
                count,
                data,
            } => Ok(LayoutNode::Raw {
                data,
                elem_size,
                count,
            }),
            Self::BitPack {
                bit_width,
                offset,
                count,
                values_buf,
                validity,
                all_null,
            } => Ok(LayoutNode::BitPack {
                values_buf,
                bit_width,
                offset,
                count,
                validity,
                all_null,
            }),
            Self::FrameOfReference { reference, inner } => {
                let reference = reference.parse::<i128>().map_err(|err| {
                    LoomDecodeError::MalformedDescriptor(format!(
                        "invalid frame-of-reference reference '{reference}': {err}"
                    ))
                })?;
                Ok(LayoutNode::FrameOfReference {
                    reference,
                    inner: Box::new(inner.into_layout()?),
                })
            }
            Self::Dictionary { codes, values } => Ok(LayoutNode::Dictionary {
                codes: Box::new(codes.into_layout()?),
                values: Box::new(values.into_layout()?),
            }),
            Self::RunEnd {
                count,
                run_ends,
                values,
            } => Ok(LayoutNode::RunEnd {
                run_ends: Box::new(run_ends.into_layout()?),
                values: Box::new(values.into_layout()?),
                count,
            }),
            Self::KernelEscape {
                kernel_id,
                count,
                params,
            } => Ok(LayoutNode::KernelEscape {
                kernel_id,
                params,
                count,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use arrow_schema::DataType;

    use super::{from_descriptor_text, to_descriptor_text};
    use crate::l1_model::{LayoutDescription, LayoutNode};

    fn roundtrip(desc: LayoutDescription) {
        let text = to_descriptor_text(&desc).expect("print descriptor");
        let parsed = from_descriptor_text(&text).expect("parse descriptor");
        let reparsed_text = to_descriptor_text(&parsed).expect("reprint descriptor");
        assert_eq!(text, reparsed_text);
    }

    #[test]
    fn raw_roundtrip() {
        roundtrip(LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Raw {
                data: vec![1, 0, 0, 0, 2, 0, 0, 0],
                elem_size: 4,
                count: 2,
            },
            row_count: 2,
        });
    }

    #[test]
    fn bitpack_roundtrip() {
        roundtrip(LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::BitPack {
                values_buf: vec![0x11, 0x22],
                bit_width: 3,
                offset: 7,
                count: 4,
                validity: Some(vec![true, false, true, true]),
                all_null: false,
            },
            row_count: 4,
        });
    }

    #[test]
    fn for_roundtrip() {
        roundtrip(LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::FrameOfReference {
                reference: -10,
                inner: Box::new(LayoutNode::Raw {
                    data: vec![1, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
            },
            row_count: 1,
        });
    }

    #[test]
    fn dictionary_roundtrip() {
        roundtrip(LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Dictionary {
                codes: Box::new(LayoutNode::Raw {
                    data: vec![0, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
                values: Box::new(LayoutNode::Raw {
                    data: vec![42, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
            },
            row_count: 1,
        });
    }

    #[test]
    fn run_end_roundtrip() {
        roundtrip(LayoutDescription {
            data_type: DataType::Boolean,
            root: LayoutNode::RunEnd {
                count: 3,
                run_ends: Box::new(LayoutNode::Raw {
                    data: vec![2, 0, 0, 0, 3, 0, 0, 0],
                    elem_size: 4,
                    count: 2,
                }),
                values: Box::new(LayoutNode::Raw {
                    data: vec![1, 0],
                    elem_size: 1,
                    count: 2,
                }),
            },
            row_count: 3,
        });
    }

    #[test]
    fn dict_over_kernel_escape_roundtrip() {
        roundtrip(LayoutDescription {
            data_type: DataType::Utf8,
            root: LayoutNode::Dictionary {
                codes: Box::new(LayoutNode::Raw {
                    data: vec![0, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
                values: Box::new(LayoutNode::KernelEscape {
                    kernel_id: 0,
                    params: vec![1, 2, 3, 4],
                    count: 1,
                }),
            },
            row_count: 1,
        });
    }

    #[test]
    fn float_kernel_escape_roundtrip() {
        for data_type in [DataType::Float32, DataType::Float64] {
            roundtrip(LayoutDescription {
                data_type,
                root: LayoutNode::KernelEscape {
                    kernel_id: 1,
                    params: vec![1, 2, 3, 4],
                    count: 0,
                },
                row_count: 0,
            });
        }
    }

    #[test]
    fn invalid_descriptor_returns_typed_error() {
        let err = from_descriptor_text("not ron").expect_err("invalid descriptor must fail");
        assert!(err.to_string().contains("malformed descriptor"));
    }

    #[test]
    fn payload_descriptor_payload_roundtrip() {
        let desc = LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Raw {
                data: vec![7, 0, 0, 0],
                elem_size: 4,
                count: 1,
            },
            row_count: 1,
        };
        let payload = crate::layout_codec::encode_layout_payload(&desc);
        let text = super::payload_to_descriptor_text(&payload).expect("payload -> descriptor");
        let encoded = super::descriptor_text_to_payload(&text).expect("descriptor -> payload");
        assert_eq!(payload, encoded);
    }
}
