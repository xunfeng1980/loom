//! L2 kernel registry and FSST L2 kernel.
//!
//! L2 kernels are total functions that own their output Arrow array. This keeps
//! the L1 read loop focused on builder-backed declarative encodings while
//! preserving a clean dispatch boundary for native L2 implementations.

use std::panic::{catch_unwind, AssertUnwindSafe};

use arrow::array::{Array, Float32Builder, Float64Builder, StringBuilder};
use arrow_data::ArrayData;

use super::alp_params::{AlpOutputType, AlpParams};
use loom_ir_core::error::LoomDecodeError;
use super::fsst_params::FsstParams;

/// Total-function L2 kernel.
pub trait L2Kernel {
    /// Decode kernel-specific params into an Arrow array.
    fn decode(&self, params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError>;
}

/// Registry mapping stable kernel ids to L2 kernels.
pub struct L2KernelRegistry {
    kernels: Vec<Box<dyn L2Kernel>>,
}

impl L2KernelRegistry {
    /// Construct the MVP0 registry. Kernel id 0 is FSST, id 1 is ALP.
    pub fn default_for_mvp0() -> Self {
        Self {
            kernels: vec![Box::new(FsstKernel), Box::new(AlpKernel)],
        }
    }

    /// Return the registered kernel for `id`, if any.
    pub fn get(&self, id: u32) -> Option<&dyn L2Kernel> {
        self.kernels.get(id as usize).map(|k| k.as_ref())
    }
}

/// ALP-style float decompression kernel.
pub struct AlpKernel;

impl L2Kernel for AlpKernel {
    fn decode(&self, params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError> {
        catch_unwind(AssertUnwindSafe(|| decode_alp(params, count)))
            .unwrap_or(Err(LoomDecodeError::AlpKernelFailed("decoder panicked")))
    }
}

fn decode_alp(params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError> {
    if params.is_empty() {
        return Err(LoomDecodeError::MalformedAlpParams("empty params"));
    }

    let params = AlpParams::decode(params, count)?;
    let scale = 10f64.powi(params.decimal_exponent);
    if !scale.is_finite() {
        return Err(LoomDecodeError::AlpKernelFailed("non-finite scale"));
    }

    match params.output_type {
        AlpOutputType::Float32 => {
            let mut builder = Float32Builder::new();
            for row in 0..count {
                if params
                    .validity
                    .as_ref()
                    .is_some_and(|validity| !validity[row])
                {
                    builder.append_null();
                    continue;
                }
                let value = (params.mantissas[row] as f64 * scale) as f32;
                if !value.is_finite() {
                    return Err(LoomDecodeError::AlpKernelFailed(
                        "decoded value is not finite",
                    ));
                }
                builder.append_value(value);
            }
            Ok(builder.finish().into_data())
        }
        AlpOutputType::Float64 => {
            let mut builder = Float64Builder::new();
            for row in 0..count {
                if params
                    .validity
                    .as_ref()
                    .is_some_and(|validity| !validity[row])
                {
                    builder.append_null();
                    continue;
                }
                let value = params.mantissas[row] as f64 * scale;
                if !value.is_finite() {
                    return Err(LoomDecodeError::AlpKernelFailed(
                        "decoded value is not finite",
                    ));
                }
                builder.append_value(value);
            }
            Ok(builder.finish().into_data())
        }
    }
}

/// FSST string decompression kernel.
pub struct FsstKernel;

impl L2Kernel for FsstKernel {
    fn decode(&self, params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError> {
        catch_unwind(AssertUnwindSafe(|| decode_fsst(params, count)))
            .unwrap_or(Err(LoomDecodeError::FsstKernelFailed("decoder panicked")))
    }
}

fn decode_fsst(params: &[u8], count: usize) -> Result<ArrayData, LoomDecodeError> {
    if params.is_empty() {
        return Err(LoomDecodeError::MalformedFsstParams("empty params"));
    }

    let params = FsstParams::decode(params, count)?;
    let decompressor = fsst::Decompressor::new(&params.symbols, &params.symbol_lengths);
    let mut builder = StringBuilder::new();

    for row in 0..count {
        if params
            .validity
            .as_ref()
            .is_some_and(|validity| !validity[row])
        {
            builder.append_null();
            continue;
        }

        let start = usize::try_from(params.codes_offsets[row])
            .map_err(|_| LoomDecodeError::InvalidFsstOffsets("offset does not fit usize"))?;
        let end = usize::try_from(params.codes_offsets[row + 1])
            .map_err(|_| LoomDecodeError::InvalidFsstOffsets("offset does not fit usize"))?;
        let compressed = &params.codes_bytes[start..end];
        let decompressed = decompressor.decompress(compressed);
        let expected_len = usize::try_from(params.uncompressed_lengths[row]).map_err(|_| {
            LoomDecodeError::FsstKernelFailed("uncompressed length does not fit usize")
        })?;
        if decompressed.len() != expected_len {
            return Err(LoomDecodeError::FsstKernelFailed("decoded length mismatch"));
        }

        let value = std::str::from_utf8(&decompressed)
            .map_err(|_| LoomDecodeError::InvalidUtf8 { index: row })?;
        builder.append_value(value);
    }

    Ok(builder.finish().into_data())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Float32Array, Float64Array, StringArray};
    use arrow_schema::DataType;

    fn params_for_rows(rows: &[Option<&[u8]>]) -> Vec<u8> {
        let symbols = vec![fsst::Symbol::from_u8(b'a'), fsst::Symbol::from_u8(b'b')];
        let mut codes_offsets = Vec::with_capacity(rows.len() + 1);
        let mut uncompressed_lengths = Vec::with_capacity(rows.len());
        let mut validity = Vec::with_capacity(rows.len());
        let mut codes_bytes = Vec::new();

        codes_offsets.push(0);
        for row in rows {
            match row {
                Some(bytes) => {
                    validity.push(true);
                    uncompressed_lengths.push(bytes.len() as u64);
                    for byte in *bytes {
                        match *byte {
                            b'a' => codes_bytes.push(0),
                            b'b' => codes_bytes.push(1),
                            other => {
                                codes_bytes.push(fsst::ESCAPE_CODE);
                                codes_bytes.push(other);
                            }
                        }
                    }
                }
                None => {
                    validity.push(false);
                    uncompressed_lengths.push(0);
                }
            }
            codes_offsets.push(codes_bytes.len() as u64);
        }

        FsstParams {
            symbols,
            symbol_lengths: vec![1, 1],
            codes_offsets,
            uncompressed_lengths,
            validity: Some(validity),
            codes_bytes,
        }
        .encode()
    }

    fn decode_strings(params: Vec<u8>, count: usize) -> Result<StringArray, LoomDecodeError> {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry.get(0).expect("FSST kernel must exist");
        let data = kernel.decode(&params, count)?;
        Ok(StringArray::from(data))
    }

    #[test]
    fn default_registry_has_fsst_at_zero() {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry
            .get(0)
            .expect("FSST kernel must be registered at id 0");
        let params = FsstParams {
            symbols: vec![],
            symbol_lengths: vec![],
            codes_offsets: vec![0],
            uncompressed_lengths: vec![],
            validity: None,
            codes_bytes: vec![],
        }
        .encode();

        let data = kernel
            .decode(&params, 0)
            .expect("zero-row decode should succeed");
        assert_eq!(data.data_type(), &DataType::Utf8);
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn default_registry_has_alp_at_one() {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry
            .get(1)
            .expect("ALP kernel must be registered at id 1");
        let params = AlpParams {
            output_type: AlpOutputType::Float32,
            decimal_exponent: -1,
            mantissas: vec![],
            validity: None,
        }
        .encode();

        let data = kernel
            .decode(&params, 0)
            .expect("zero-row decode should succeed");
        assert_eq!(data.data_type(), &DataType::Float32);
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn default_registry_missing_id_returns_none() {
        let registry = L2KernelRegistry::default_for_mvp0();
        assert!(registry.get(2).is_none());
    }

    #[test]
    fn fsst_kernel_decodes_plain_strings() {
        let array = decode_strings(params_for_rows(&[Some(b"aa"), Some(b"bax")]), 2)
            .expect("FSST rows should decode");

        assert_eq!(array.value(0), "aa");
        assert_eq!(array.value(1), "bax");
    }

    #[test]
    fn fsst_kernel_preserves_nulls() {
        let array = decode_strings(params_for_rows(&[Some(b"a"), None, Some(b"b")]), 3)
            .expect("FSST rows should decode");

        assert_eq!(array.value(0), "a");
        assert!(array.is_null(1));
        assert_eq!(array.value(2), "b");
    }

    #[test]
    fn fsst_kernel_rejects_invalid_utf8() {
        let err = decode_strings(params_for_rows(&[Some(&[0xff])]), 1)
            .expect_err("invalid UTF-8 must be rejected");

        assert_eq!(err, LoomDecodeError::InvalidUtf8 { index: 0 });
    }

    #[test]
    fn fsst_kernel_rejects_empty_params() {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry.get(0).expect("FSST kernel must exist");
        let err = kernel.decode(&[], 0).expect_err("empty params are invalid");

        assert_eq!(err, LoomDecodeError::MalformedFsstParams("empty params"));
    }

    #[test]
    fn fsst_kernel_panic_becomes_typed_error() {
        let params = FsstParams {
            symbols: vec![fsst::Symbol::from_u8(b'a')],
            symbol_lengths: vec![1],
            codes_offsets: vec![0, 1],
            uncompressed_lengths: vec![1],
            validity: None,
            codes_bytes: vec![fsst::ESCAPE_CODE],
        }
        .encode();

        let err = decode_strings(params, 1).expect_err("bad code should panic inside fsst-rs");
        assert_eq!(err, LoomDecodeError::FsstKernelFailed("decoder panicked"));
    }

    #[test]
    fn alp_kernel_decodes_float32_values_and_nulls() {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry.get(1).expect("ALP kernel must exist");
        let params = AlpParams {
            output_type: AlpOutputType::Float32,
            decimal_exponent: -2,
            mantissas: vec![125, -250, 0],
            validity: Some(vec![true, false, true]),
        }
        .encode();

        let array = Float32Array::from(kernel.decode(&params, 3).expect("ALP decode"));

        assert_eq!(array.data_type(), &DataType::Float32);
        assert_eq!(array.value(0), 1.25);
        assert!(array.is_null(1));
        assert_eq!(array.value(2), 0.0);
    }

    #[test]
    fn alp_kernel_decodes_float64_values_and_nulls() {
        let registry = L2KernelRegistry::default_for_mvp0();
        let kernel = registry.get(1).expect("ALP kernel must exist");
        let params = AlpParams {
            output_type: AlpOutputType::Float64,
            decimal_exponent: -3,
            mantissas: vec![1250, -2500, 0],
            validity: Some(vec![true, false, true]),
        }
        .encode();

        let array = Float64Array::from(kernel.decode(&params, 3).expect("ALP decode"));

        assert_eq!(array.data_type(), &DataType::Float64);
        assert_eq!(array.value(0), 1.25);
        assert!(array.is_null(1));
        assert_eq!(array.value(2), 0.0);
    }
}
