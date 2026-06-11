//! `LMA1` Arrow semantic payload codec and `LMC2` wrapper.
//!
//! **Phase 50.1: OUT-OF-TCB — dev-time reference packaging only.**
//! These codecs are retained for kloom/verifier test fixtures and backward-compat
//! decode. Production emission paths do NOT use LMC2/LMA1 (replaced by sidecar
//! overlay in Phase 50). Encode functions are explicitly documented as dev-time only.

use std::io::Cursor;

use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::StreamWriter;

use crate::arrow_semantic::{ArrowSemanticPayload, LMA1_MAGIC, LMC2_MAGIC};
use crate::arrow_semantic_verifier::verify_arrow_semantic_payload;
use crate::error::LoomDecodeError;

const LMA1_VERSION: u16 = 1;
pub const LMC2_VERSION: u16 = 1;

const LMC2_HEADER_PREFIX_LEN: usize = 4 + 2 + 2 + 8 + 8 + 4;
const LMC2_SECTION_ENTRY_LEN: usize = 2 + 2 + 8 + 8 + 4 + 4;
const LMC2_SECTION_FLAG_REQUIRED: u16 = 1;
const LMC2_SECTION_ARROW_SEMANTIC_PAYLOAD: u16 = 1;
const LMC2_FEATURE_ARROW_SEMANTIC_LMA1: u8 = 0;
const LMC2_KNOWN_REQUIRED_FEATURE_MASK: u64 = 1 << LMC2_FEATURE_ARROW_SEMANTIC_LMA1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowSemanticContainerPayload {
    pub version: u16,
    pub required_features: u64,
    pub optional_features: u64,
    pub payload: Vec<u8>,
}

pub fn is_arrow_semantic_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(LMA1_MAGIC)
}

pub fn is_arrow_semantic_container(bytes: &[u8]) -> bool {
    bytes.starts_with(LMC2_MAGIC)
}

pub fn arrow_semantic_container_feature_names(bits: u64) -> Vec<&'static str> {
    let mut names = Vec::new();
    if bits & (1 << LMC2_FEATURE_ARROW_SEMANTIC_LMA1) != 0 {
        names.push("arrow_semantic_lma1");
    }
    names
}

/// Dev-time only. Encodes an `ArrowSemanticPayload` into LMA1 bytes.
pub fn encode_arrow_semantic_payload(
    payload: &ArrowSemanticPayload,
) -> Result<Vec<u8>, LoomDecodeError> {
    let report = verify_arrow_semantic_payload(payload);
    if !report.is_ok() {
        return Err(LoomDecodeError::MalformedLayoutPayload(
            "arrow semantic payload verification failed",
        ));
    }

    let batches = payload.to_record_batches()?;
    let mut ipc = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut ipc, payload.schema())
            .map_err(|_| LoomDecodeError::MalformedLayoutPayload("arrow semantic ipc writer"))?;
        for batch in &batches {
            writer.write(batch).map_err(|_| {
                LoomDecodeError::MalformedLayoutPayload("arrow semantic ipc write batch")
            })?;
        }
        writer
            .finish()
            .map_err(|_| LoomDecodeError::MalformedLayoutPayload("arrow semantic ipc finish"))?;
    }

    let mut out = Vec::with_capacity(LMA1_MAGIC.len() + 2 + 8 + ipc.len());
    out.extend_from_slice(LMA1_MAGIC);
    out.extend_from_slice(&LMA1_VERSION.to_le_bytes());
    out.extend_from_slice(&(ipc.len() as u64).to_le_bytes());
    out.extend_from_slice(&ipc);
    Ok(out)
}

/// Dev-time only. Encodes an `ArrowSemanticPayload` into an LMC2 container.
pub fn encode_arrow_semantic_container_payload(
    payload: &ArrowSemanticPayload,
) -> Result<Vec<u8>, LoomDecodeError> {
    let lma1 = encode_arrow_semantic_payload(payload)?;
    wrap_arrow_semantic_payload(&lma1)
}

/// Dev-time only. Wraps pre-encoded LMA1 bytes into an LMC2 container.
pub fn wrap_arrow_semantic_payload(lma1_bytes: &[u8]) -> Result<Vec<u8>, LoomDecodeError> {
    decode_arrow_semantic_payload(lma1_bytes)?;

    let section_count = 1u32;
    let header_len = LMC2_HEADER_PREFIX_LEN
        .checked_add(LMC2_SECTION_ENTRY_LEN)
        .ok_or(LoomDecodeError::MalformedContainer(
            "header length overflow",
        ))?;
    let header_len_u16 = u16::try_from(header_len)
        .map_err(|_| LoomDecodeError::MalformedContainer("header length overflow"))?;
    let section_offset = u64::try_from(header_len)
        .map_err(|_| LoomDecodeError::MalformedContainer("header length overflow"))?;
    let section_length = u64::try_from(lma1_bytes.len())
        .map_err(|_| LoomDecodeError::MalformedContainer("section length overflow"))?;

    let mut out = Vec::with_capacity(header_len + lma1_bytes.len() + LMC2_MAGIC.len());
    out.extend_from_slice(LMC2_MAGIC);
    out.extend_from_slice(&LMC2_VERSION.to_le_bytes());
    out.extend_from_slice(&header_len_u16.to_le_bytes());
    out.extend_from_slice(&LMC2_KNOWN_REQUIRED_FEATURE_MASK.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    out.extend_from_slice(&section_count.to_le_bytes());
    out.extend_from_slice(&LMC2_SECTION_ARROW_SEMANTIC_PAYLOAD.to_le_bytes());
    out.extend_from_slice(&LMC2_SECTION_FLAG_REQUIRED.to_le_bytes());
    out.extend_from_slice(&section_offset.to_le_bytes());
    out.extend_from_slice(&section_length.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(lma1_bytes);
    out.extend_from_slice(LMC2_MAGIC);
    Ok(out)
}

pub fn unwrap_arrow_semantic_payload(bytes: &[u8]) -> Result<Vec<u8>, LoomDecodeError> {
    decode_arrow_semantic_container(bytes).map(|container| container.payload)
}

pub fn decode_arrow_semantic_container_payload(
    bytes: &[u8],
) -> Result<ArrowSemanticPayload, LoomDecodeError> {
    let payload = unwrap_arrow_semantic_payload(bytes)?;
    decode_arrow_semantic_payload(&payload)
}

pub fn decode_arrow_semantic_container(
    bytes: &[u8],
) -> Result<ArrowSemanticContainerPayload, LoomDecodeError> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_array_container::<4>()?;
    if &magic != LMC2_MAGIC {
        return Err(LoomDecodeError::MalformedContainer("wrong magic"));
    }

    let version = reader.read_u16_container()?;
    if version != LMC2_VERSION {
        return Err(LoomDecodeError::MalformedContainer("unsupported version"));
    }

    let header_len = usize::from(reader.read_u16_container()?);
    if header_len < LMC2_HEADER_PREFIX_LEN {
        return Err(LoomDecodeError::MalformedContainer("header too short"));
    }

    let required_features = reader.read_u64_container()?;
    if required_features & !LMC2_KNOWN_REQUIRED_FEATURE_MASK != 0 {
        return Err(LoomDecodeError::MalformedContainer(
            "unknown required feature",
        ));
    }
    let optional_features = reader.read_u64_container()?;
    let section_count = usize::try_from(reader.read_u32_container()?)
        .map_err(|_| LoomDecodeError::MalformedContainer("section count overflow"))?;
    let expected_header_len = LMC2_HEADER_PREFIX_LEN
        .checked_add(section_count.checked_mul(LMC2_SECTION_ENTRY_LEN).ok_or(
            LoomDecodeError::MalformedContainer("header length overflow"),
        )?)
        .ok_or(LoomDecodeError::MalformedContainer(
            "header length overflow",
        ))?;
    if header_len != expected_header_len {
        return Err(LoomDecodeError::MalformedContainer(
            "header length mismatch",
        ));
    }
    if header_len > bytes.len() {
        return Err(LoomDecodeError::MalformedContainer("truncated header"));
    }

    let mut payload_section: Option<(u64, u64)> = None;
    let mut sorted = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        let tag = reader.read_u16_container()?;
        let flags = reader.read_u16_container()?;
        if flags & !LMC2_SECTION_FLAG_REQUIRED != 0 {
            return Err(LoomDecodeError::MalformedContainer("unknown section flags"));
        }
        let offset = reader.read_u64_container()?;
        let length = reader.read_u64_container()?;
        let checksum_or_reserved = reader.read_u32_container()?;
        if checksum_or_reserved != 0 {
            return Err(LoomDecodeError::MalformedContainer(
                "checksum field must be zero",
            ));
        }
        let reserved = reader.read_u32_container()?;
        if reserved != 0 {
            return Err(LoomDecodeError::MalformedContainer(
                "reserved field must be zero",
            ));
        }
        if tag == LMC2_SECTION_ARROW_SEMANTIC_PAYLOAD {
            if flags & LMC2_SECTION_FLAG_REQUIRED == 0 {
                return Err(LoomDecodeError::MalformedContainer(
                    "arrow semantic payload must be required",
                ));
            }
            if payload_section.replace((offset, length)).is_some() {
                return Err(LoomDecodeError::MalformedContainer(
                    "duplicate arrow semantic payload",
                ));
            }
        } else if flags & LMC2_SECTION_FLAG_REQUIRED != 0 {
            return Err(LoomDecodeError::MalformedContainer(
                "unknown required section",
            ));
        }
        sorted.push((offset, length));
    }
    if reader.pos != header_len {
        return Err(LoomDecodeError::MalformedContainer(
            "header length mismatch",
        ));
    }

    let has_trailer =
        bytes.len() >= LMC2_MAGIC.len() && &bytes[bytes.len() - LMC2_MAGIC.len()..] == LMC2_MAGIC;
    let payload_end = if has_trailer {
        bytes.len() - LMC2_MAGIC.len()
    } else {
        bytes.len()
    };
    sorted.sort_by_key(|entry| entry.0);
    let mut expected_offset = header_len;
    for (offset_u64, length_u64) in &sorted {
        let offset = usize::try_from(*offset_u64)
            .map_err(|_| LoomDecodeError::MalformedContainer("section offset overflow"))?;
        let length = usize::try_from(*length_u64)
            .map_err(|_| LoomDecodeError::MalformedContainer("section length overflow"))?;
        let end = offset
            .checked_add(length)
            .ok_or(LoomDecodeError::MalformedContainer(
                "section offset overflow",
            ))?;
        if offset != expected_offset {
            return Err(LoomDecodeError::MalformedContainer(
                "section gap or overlap",
            ));
        }
        if end > payload_end {
            return Err(LoomDecodeError::MalformedContainer(
                "section outside container",
            ));
        }
        expected_offset = end;
    }
    if expected_offset != payload_end {
        return Err(LoomDecodeError::MalformedContainer(
            "trailing section bytes",
        ));
    }

    let (offset_u64, length_u64) = payload_section.ok_or(LoomDecodeError::MalformedContainer(
        "missing arrow semantic payload",
    ))?;
    let offset = usize::try_from(offset_u64)
        .map_err(|_| LoomDecodeError::MalformedContainer("section offset overflow"))?;
    let length = usize::try_from(length_u64)
        .map_err(|_| LoomDecodeError::MalformedContainer("section length overflow"))?;
    let end = offset
        .checked_add(length)
        .ok_or(LoomDecodeError::MalformedContainer(
            "section offset overflow",
        ))?;
    let payload = bytes[offset..end].to_vec();
    decode_arrow_semantic_payload(&payload)
        .map_err(|_| LoomDecodeError::MalformedContainer("malformed inner LMA1 payload"))?;

    Ok(ArrowSemanticContainerPayload {
        version,
        required_features,
        optional_features,
        payload,
    })
}

pub fn decode_arrow_semantic_payload(
    bytes: &[u8],
) -> Result<ArrowSemanticPayload, LoomDecodeError> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_array::<4>()?;
    if &magic != LMA1_MAGIC {
        return Err(LoomDecodeError::MalformedLayoutPayload(
            "wrong arrow semantic magic",
        ));
    }
    let version = reader.read_u16()?;
    if version != LMA1_VERSION {
        return Err(LoomDecodeError::MalformedLayoutPayload(
            "unsupported arrow semantic version",
        ));
    }
    let ipc_len = reader.read_u64_as_usize("arrow semantic ipc length")?;
    let ipc = reader.read_bytes(ipc_len)?;
    reader.finish()?;

    let mut stream = StreamReader::try_new(Cursor::new(ipc), None)
        .map_err(|_| LoomDecodeError::MalformedLayoutPayload("arrow semantic ipc reader"))?;
    let schema = stream.schema();
    let mut batches = Vec::new();
    for batch in &mut stream {
        let batch = batch.map_err(|_| {
            LoomDecodeError::MalformedLayoutPayload("arrow semantic ipc read batch")
        })?;
        batches.push(batch);
    }
    let payload = ArrowSemanticPayload::from_record_batches(&batches)
        .or_else(|_| ArrowSemanticPayload::try_new(schema, Vec::new()))?;
    let report = verify_arrow_semantic_payload(&payload);
    if !report.is_ok() {
        return Err(LoomDecodeError::MalformedLayoutPayload(
            "decoded arrow semantic payload verification failed",
        ));
    }
    Ok(payload)
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

    fn read_u16_container(&mut self) -> Result<u16, LoomDecodeError> {
        Ok(u16::from_le_bytes(self.read_array_container()?))
    }

    fn read_u32_container(&mut self) -> Result<u32, LoomDecodeError> {
        Ok(u32::from_le_bytes(self.read_array_container()?))
    }

    fn read_u64(&mut self) -> Result<u64, LoomDecodeError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_u64_container(&mut self) -> Result<u64, LoomDecodeError> {
        Ok(u64::from_le_bytes(self.read_array_container()?))
    }

    fn read_u64_as_usize(&mut self, field: &'static str) -> Result<usize, LoomDecodeError> {
        usize::try_from(self.read_u64()?)
            .map_err(|_| LoomDecodeError::MalformedLayoutPayload(field))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedLayoutPayload(
                "payload length overflow",
            ))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedLayoutPayload(
                "truncated arrow semantic payload",
            ));
        }
        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn read_array_container<const N: usize>(&mut self) -> Result<[u8; N], LoomDecodeError> {
        let bytes = self.read_bytes_container(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn read_bytes_container(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedContainer(
                "payload length overflow",
            ))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedContainer("truncated container"));
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
                "trailing arrow semantic bytes",
            ))
        }
    }
}
