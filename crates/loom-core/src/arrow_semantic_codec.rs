//! `LMA1` Arrow semantic payload codec.

use std::io::Cursor;

use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::StreamWriter;

use crate::arrow_semantic::{ArrowSemanticPayload, LMA1_MAGIC, LMC2_MAGIC};
use crate::arrow_semantic_verifier::verify_arrow_semantic_payload;
use crate::error::LoomDecodeError;

const LMA1_VERSION: u16 = 1;

pub fn is_arrow_semantic_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(LMA1_MAGIC)
}

pub fn is_arrow_semantic_container(bytes: &[u8]) -> bool {
    bytes.starts_with(LMC2_MAGIC)
}

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

    fn read_u64(&mut self) -> Result<u64, LoomDecodeError> {
        Ok(u64::from_le_bytes(self.read_array()?))
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
