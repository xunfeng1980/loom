//! Stable binary parameter format for the FSST L2 kernel.
//!
//! The format intentionally stores only the pieces needed by `fsst-rs`
//! decompression plus Arrow-style per-row offsets and validity. All integers are
//! little-endian.

use loom_ir_core::error::LoomDecodeError;

const MAGIC: &[u8; 4] = b"LFS1";
const VERSION: u16 = 1;
const FLAG_VALIDITY: u16 = 1;

/// Decoded FSST L2 kernel parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsstParams {
    pub symbols: Vec<fsst::Symbol>,
    pub symbol_lengths: Vec<u8>,
    pub codes_offsets: Vec<u64>,
    pub uncompressed_lengths: Vec<u64>,
    pub validity: Option<Vec<bool>>,
    pub codes_bytes: Vec<u8>,
}

impl FsstParams {
    /// Encode this parameter set into Loom's stable `LFS1` binary format.
    pub fn encode(&self) -> Vec<u8> {
        let row_count = self.uncompressed_lengths.len() as u64;
        let flags = if self.validity.is_some() {
            FLAG_VALIDITY
        } else {
            0
        };

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&row_count.to_le_bytes());
        out.extend_from_slice(&(self.symbols.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());

        for symbol in &self.symbols {
            out.extend_from_slice(&symbol.to_u64().to_le_bytes());
        }
        out.extend_from_slice(&self.symbol_lengths);

        out.extend_from_slice(&(self.codes_offsets.len() as u64).to_le_bytes());
        for offset in &self.codes_offsets {
            out.extend_from_slice(&offset.to_le_bytes());
        }
        for len in &self.uncompressed_lengths {
            out.extend_from_slice(&len.to_le_bytes());
        }

        if let Some(validity) = &self.validity {
            out.extend_from_slice(&(validity.len() as u64).to_le_bytes());
            out.extend(validity.iter().map(|is_valid| u8::from(*is_valid)));
        }

        out.extend_from_slice(&(self.codes_bytes.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.codes_bytes);
        out
    }

    /// Decode and validate Loom's stable `LFS1` parameter format.
    pub fn decode(params: &[u8], expected_count: usize) -> Result<Self, LoomDecodeError> {
        let mut reader = ParamsReader::new(params);

        let magic = reader.read_array::<4>()?;
        if &magic != MAGIC {
            return Err(LoomDecodeError::MalformedFsstParams("wrong magic"));
        }

        let version = reader.read_u16()?;
        if version != VERSION {
            return Err(LoomDecodeError::MalformedFsstParams("unsupported version"));
        }

        let flags = reader.read_u16()?;
        if flags & !FLAG_VALIDITY != 0 {
            return Err(LoomDecodeError::MalformedFsstParams("unknown flags"));
        }

        let row_count = reader.read_u64_as_usize("row count")?;
        if row_count != expected_count {
            return Err(LoomDecodeError::MalformedFsstParams("row count mismatch"));
        }

        let symbol_count = reader.read_u16()? as usize;
        if symbol_count >= 256 {
            return Err(LoomDecodeError::InvalidFsstSymbolTable(
                "symbol count must be less than 256",
            ));
        }

        let reserved = reader.read_u16()?;
        if reserved != 0 {
            return Err(LoomDecodeError::MalformedFsstParams(
                "reserved field must be zero",
            ));
        }

        let mut symbols = Vec::with_capacity(symbol_count);
        for _ in 0..symbol_count {
            let raw = reader.read_u64()?;
            symbols.push(fsst::Symbol::from_slice(&raw.to_le_bytes()));
        }

        let symbol_lengths = reader.read_bytes(symbol_count)?.to_vec();
        if symbol_lengths.iter().any(|len| !(1..=8).contains(len)) {
            return Err(LoomDecodeError::InvalidFsstSymbolTable(
                "symbol lengths must be in 1..=8",
            ));
        }

        let offsets_len = reader.read_u64_as_usize("offsets length")?;
        if offsets_len != row_count + 1 {
            return Err(LoomDecodeError::InvalidFsstOffsets(
                "offset count must equal rows + 1",
            ));
        }

        let mut codes_offsets = Vec::with_capacity(offsets_len);
        for _ in 0..offsets_len {
            codes_offsets.push(reader.read_u64()?);
        }
        validate_offsets_shape(&codes_offsets)?;

        let mut uncompressed_lengths = Vec::with_capacity(row_count);
        for _ in 0..row_count {
            uncompressed_lengths.push(reader.read_u64()?);
        }

        let validity = if flags & FLAG_VALIDITY != 0 {
            let validity_len = reader.read_u64_as_usize("validity length")?;
            if validity_len != row_count {
                return Err(LoomDecodeError::MalformedFsstParams(
                    "validity length mismatch",
                ));
            }

            let bytes = reader.read_bytes(validity_len)?;
            let mut validity = Vec::with_capacity(validity_len);
            for byte in bytes {
                match *byte {
                    0 => validity.push(false),
                    1 => validity.push(true),
                    _ => {
                        return Err(LoomDecodeError::MalformedFsstParams(
                            "invalid validity byte",
                        ));
                    }
                }
            }
            Some(validity)
        } else {
            None
        };

        let codes_len = reader.read_u64_as_usize("codes bytes length")?;
        let codes_bytes = reader.read_bytes(codes_len)?.to_vec();
        validate_offsets_against_codes(&codes_offsets, codes_len)?;
        reader.finish()?;

        Ok(Self {
            symbols,
            symbol_lengths,
            codes_offsets,
            uncompressed_lengths,
            validity,
            codes_bytes,
        })
    }
}

fn validate_offsets_shape(offsets: &[u64]) -> Result<(), LoomDecodeError> {
    if offsets.first().copied() != Some(0) {
        return Err(LoomDecodeError::InvalidFsstOffsets(
            "first offset must be zero",
        ));
    }

    for (index, pair) in offsets.windows(2).enumerate() {
        if pair[1] < pair[0] {
            return Err(LoomDecodeError::InvalidFsstOffsets(if index == 0 {
                "offsets must be monotonic"
            } else {
                "offsets must be monotonic"
            }));
        }
    }

    Ok(())
}

fn validate_offsets_against_codes(
    offsets: &[u64],
    codes_len: usize,
) -> Result<(), LoomDecodeError> {
    let last = offsets.last().copied().unwrap_or(0);
    if last > codes_len as u64 {
        return Err(LoomDecodeError::InvalidFsstOffsets(
            "last offset exceeds codes length",
        ));
    }

    Ok(())
}

struct ParamsReader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> ParamsReader<'a> {
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
        usize::try_from(self.read_u64()?).map_err(|_| LoomDecodeError::MalformedFsstParams(field))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedFsstParams("truncated params"))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedFsstParams("truncated params"));
        }

        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), LoomDecodeError> {
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(LoomDecodeError::MalformedFsstParams("trailing bytes"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params() -> FsstParams {
        FsstParams {
            symbols: vec![fsst::Symbol::from_u8(b'a'), fsst::Symbol::from_u8(b'b')],
            symbol_lengths: vec![1, 1],
            codes_offsets: vec![0, 2, 3],
            uncompressed_lengths: vec![2, 1],
            validity: Some(vec![true, false]),
            codes_bytes: vec![0, 1, 0],
        }
    }

    #[test]
    fn roundtrip_params() {
        let params = sample_params();
        let encoded = params.encode();
        let decoded = FsstParams::decode(&encoded, 2).expect("params should decode");

        assert_eq!(decoded.symbols, params.symbols);
        assert_eq!(decoded.symbol_lengths, params.symbol_lengths);
        assert_eq!(decoded.codes_offsets, params.codes_offsets);
        assert_eq!(decoded.uncompressed_lengths, params.uncompressed_lengths);
        assert_eq!(decoded.validity, params.validity);
        assert_eq!(decoded.codes_bytes, params.codes_bytes);
    }

    #[test]
    fn rejects_non_monotonic_offsets() {
        let mut params = sample_params();
        params.codes_offsets = vec![0, 3, 2];
        let encoded = params.encode();

        let err = FsstParams::decode(&encoded, 2).expect_err("offsets must be rejected");
        assert!(matches!(err, LoomDecodeError::InvalidFsstOffsets(_)));
    }

    #[test]
    fn rejects_truncated_params() {
        let encoded = sample_params().encode();
        let err = FsstParams::decode(&encoded[..encoded.len() - 1], 2)
            .expect_err("truncated params must be rejected");
        assert_eq!(
            err,
            LoomDecodeError::MalformedFsstParams("truncated params")
        );
    }

    #[test]
    fn rejects_row_count_mismatch() {
        let encoded = sample_params().encode();
        let err = FsstParams::decode(&encoded, 1).expect_err("row count mismatch");
        assert_eq!(
            err,
            LoomDecodeError::MalformedFsstParams("row count mismatch")
        );
    }
}
