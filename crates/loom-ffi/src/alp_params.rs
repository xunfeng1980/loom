//! Stable binary parameter format for the ALP-style float L2 kernel.
//!
//! Phase 10 uses a Loom-owned ALP params contract rather than depending on a
//! Vortex ALP crate. The format stores scaled integer mantissas, a decimal
//! exponent, an output float type, and optional validity.

use arrow_schema::DataType;

use loom_ir_core::error::LoomDecodeError;

const MAGIC: &[u8; 4] = b"LAP1";
const VERSION: u16 = 1;
const FLAG_VALIDITY: u16 = 1;

const OUTPUT_F32: u8 = 1;
const OUTPUT_F64: u8 = 2;

/// Decoded output type carried by `AlpParams`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlpOutputType {
    Float32,
    Float64,
}

impl AlpOutputType {
    pub fn from_data_type(data_type: &DataType) -> Result<Self, LoomDecodeError> {
        match data_type {
            DataType::Float32 => Ok(Self::Float32),
            DataType::Float64 => Ok(Self::Float64),
            _ => Err(LoomDecodeError::MalformedAlpParams(
                "unsupported output type",
            )),
        }
    }

    pub fn to_data_type(self) -> DataType {
        match self {
            Self::Float32 => DataType::Float32,
            Self::Float64 => DataType::Float64,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Float32 => "Float32",
            Self::Float64 => "Float64",
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::Float32 => OUTPUT_F32,
            Self::Float64 => OUTPUT_F64,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, LoomDecodeError> {
        match tag {
            OUTPUT_F32 => Ok(Self::Float32),
            OUTPUT_F64 => Ok(Self::Float64),
            _ => Err(LoomDecodeError::MalformedAlpParams("invalid output type")),
        }
    }
}

/// Decoded ALP-style L2 kernel parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlpParams {
    pub output_type: AlpOutputType,
    pub decimal_exponent: i32,
    pub mantissas: Vec<i64>,
    pub validity: Option<Vec<bool>>,
}

impl AlpParams {
    /// Encode this parameter set into Loom's stable `LAP1` binary format.
    pub fn encode(&self) -> Vec<u8> {
        let row_count = self.mantissas.len() as u64;
        let flags = if self.validity.is_some() {
            FLAG_VALIDITY
        } else {
            0
        };

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.push(self.output_type.tag());
        out.extend_from_slice(&[0, 0, 0]);
        out.extend_from_slice(&row_count.to_le_bytes());
        out.extend_from_slice(&self.decimal_exponent.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(self.mantissas.len() as u64).to_le_bytes());
        for mantissa in &self.mantissas {
            out.extend_from_slice(&mantissa.to_le_bytes());
        }

        if let Some(validity) = &self.validity {
            out.extend_from_slice(&(validity.len() as u64).to_le_bytes());
            out.extend(validity.iter().map(|is_valid| u8::from(*is_valid)));
        }

        out
    }

    /// Decode and validate Loom's stable `LAP1` parameter format.
    pub fn decode(params: &[u8], expected_count: usize) -> Result<Self, LoomDecodeError> {
        let mut reader = ParamsReader::new(params);

        let magic = reader.read_array::<4>()?;
        if &magic != MAGIC {
            return Err(LoomDecodeError::MalformedAlpParams("wrong magic"));
        }

        let version = reader.read_u16()?;
        if version != VERSION {
            return Err(LoomDecodeError::MalformedAlpParams("unsupported version"));
        }

        let flags = reader.read_u16()?;
        if flags & !FLAG_VALIDITY != 0 {
            return Err(LoomDecodeError::MalformedAlpParams("unknown flags"));
        }

        let output_type = AlpOutputType::from_tag(reader.read_u8()?)?;
        let reserved = reader.read_array::<3>()?;
        if reserved != [0, 0, 0] {
            return Err(LoomDecodeError::MalformedAlpParams(
                "reserved field must be zero",
            ));
        }

        let row_count = reader.read_u64_as_usize("row count")?;
        if row_count != expected_count {
            return Err(LoomDecodeError::MalformedAlpParams("row count mismatch"));
        }

        let decimal_exponent = reader.read_i32()?;
        let reserved = reader.read_u32()?;
        if reserved != 0 {
            return Err(LoomDecodeError::MalformedAlpParams(
                "reserved field must be zero",
            ));
        }

        let mantissas_len = reader.read_u64_as_usize("mantissas length")?;
        if mantissas_len != row_count {
            return Err(LoomDecodeError::MalformedAlpParams(
                "mantissas length mismatch",
            ));
        }

        let mut mantissas = Vec::with_capacity(mantissas_len);
        for _ in 0..mantissas_len {
            mantissas.push(reader.read_i64()?);
        }

        let validity = if flags & FLAG_VALIDITY != 0 {
            let validity_len = reader.read_u64_as_usize("validity length")?;
            if validity_len != row_count {
                return Err(LoomDecodeError::MalformedAlpParams(
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
                        return Err(LoomDecodeError::MalformedAlpParams("invalid validity byte"));
                    }
                }
            }
            Some(validity)
        } else {
            None
        };

        reader.finish()?;

        Ok(Self {
            output_type,
            decimal_exponent,
            mantissas,
            validity,
        })
    }
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

    fn read_i32(&mut self) -> Result<i32, LoomDecodeError> {
        Ok(i32::from_le_bytes(self.read_array()?))
    }

    fn read_i64(&mut self) -> Result<i64, LoomDecodeError> {
        Ok(i64::from_le_bytes(self.read_array()?))
    }

    fn read_u64_as_usize(&mut self, field: &'static str) -> Result<usize, LoomDecodeError> {
        usize::try_from(self.read_u64()?).map_err(|_| LoomDecodeError::MalformedAlpParams(field))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedAlpParams("truncated params"))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedAlpParams("truncated params"));
        }

        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), LoomDecodeError> {
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(LoomDecodeError::MalformedAlpParams("trailing bytes"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params(output_type: AlpOutputType) -> AlpParams {
        AlpParams {
            output_type,
            decimal_exponent: -2,
            mantissas: vec![125, -25, 0],
            validity: Some(vec![true, false, true]),
        }
    }

    #[test]
    fn roundtrip_float32_params() {
        let params = sample_params(AlpOutputType::Float32);
        let decoded = AlpParams::decode(&params.encode(), 3).expect("params should decode");

        assert_eq!(decoded, params);
        assert_eq!(decoded.output_type.to_data_type(), DataType::Float32);
    }

    #[test]
    fn roundtrip_float64_params_without_validity() {
        let mut params = sample_params(AlpOutputType::Float64);
        params.validity = None;
        let decoded = AlpParams::decode(&params.encode(), 3).expect("params should decode");

        assert_eq!(decoded, params);
        assert_eq!(decoded.output_type.to_data_type(), DataType::Float64);
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut encoded = sample_params(AlpOutputType::Float32).encode();
        encoded[0] = b'X';

        let err = AlpParams::decode(&encoded, 3).expect_err("wrong magic must fail");
        assert_eq!(err, LoomDecodeError::MalformedAlpParams("wrong magic"));
    }

    #[test]
    fn rejects_wrong_version() {
        let mut encoded = sample_params(AlpOutputType::Float32).encode();
        encoded[4] = 2;

        let err = AlpParams::decode(&encoded, 3).expect_err("wrong version must fail");
        assert_eq!(
            err,
            LoomDecodeError::MalformedAlpParams("unsupported version")
        );
    }

    #[test]
    fn rejects_row_count_mismatch() {
        let encoded = sample_params(AlpOutputType::Float32).encode();
        let err = AlpParams::decode(&encoded, 2).expect_err("row count mismatch");
        assert_eq!(
            err,
            LoomDecodeError::MalformedAlpParams("row count mismatch")
        );
    }

    #[test]
    fn rejects_invalid_output_type() {
        let mut encoded = sample_params(AlpOutputType::Float32).encode();
        encoded[8] = 99;

        let err = AlpParams::decode(&encoded, 3).expect_err("output type mismatch");
        assert_eq!(
            err,
            LoomDecodeError::MalformedAlpParams("invalid output type")
        );
    }

    #[test]
    fn rejects_invalid_validity_byte() {
        let mut encoded = sample_params(AlpOutputType::Float32).encode();
        let last = encoded.len() - 1;
        encoded[last] = 3;

        let err = AlpParams::decode(&encoded, 3).expect_err("validity byte must fail");
        assert_eq!(
            err,
            LoomDecodeError::MalformedAlpParams("invalid validity byte")
        );
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut encoded = sample_params(AlpOutputType::Float32).encode();
        encoded.push(0);

        let err = AlpParams::decode(&encoded, 3).expect_err("trailing bytes must fail");
        assert_eq!(err, LoomDecodeError::MalformedAlpParams("trailing bytes"));
    }
}
