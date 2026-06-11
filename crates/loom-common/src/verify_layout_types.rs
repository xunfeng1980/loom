//! Production-core layout verification types and the `verify_layout` function.
//!
//! Extracted from `loom-container::verifier` — zero dependency on
//! the legacy container packaging layer.
//! `verify_table` and `verify_container` remain in `loom-container::verifier`.

use std::fmt;

use arrow_schema::DataType;

use crate::alp_params::AlpParams;
use crate::fsst_params::FsstParams;
use crate::l1_model::bitpack;
use crate::l1_model::{LayoutDescription, LayoutNode};
use crate::l2_kernel_registry::L2KernelRegistry;
use loom_ir_core::error::LoomDecodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationCode {
    UnsupportedType,
    UnsupportedLayoutType,
    BufferTooShort,
    CountMismatch,
    ValidityMismatch,
    InvalidBitWidth,
    InvalidRunEnd,
    InsufficientRunValues,
    InvalidDictionaryCode,
    UnknownKernel,
    MalformedKernelParams,
    ContainerShape,
    TableShape,
}

impl VerificationCode {
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationCode::UnsupportedType => "unsupported-type",
            VerificationCode::UnsupportedLayoutType => "unsupported-layout-type",
            VerificationCode::BufferTooShort => "buffer-too-short",
            VerificationCode::CountMismatch => "count-mismatch",
            VerificationCode::ValidityMismatch => "validity-mismatch",
            VerificationCode::InvalidBitWidth => "invalid-bit-width",
            VerificationCode::InvalidRunEnd => "invalid-run-end",
            VerificationCode::InsufficientRunValues => "insufficient-run-values",
            VerificationCode::InvalidDictionaryCode => "invalid-dictionary-code",
            VerificationCode::UnknownKernel => "unknown-kernel",
            VerificationCode::MalformedKernelParams => "malformed-kernel-params",
            VerificationCode::ContainerShape => "container-shape",
            VerificationCode::TableShape => "table-shape",
        }
    }
}

impl fmt::Display for VerificationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationDiagnostic {
    pub code: VerificationCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerificationReport {
    diagnostics: Vec<VerificationDiagnostic>,
}

impl VerificationReport {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[VerificationDiagnostic] {
        &self.diagnostics
    }

    pub fn first_error(&self) -> Option<LoomDecodeError> {
        let diagnostic = self.diagnostics.first()?;
        Some(LoomDecodeError::VerifierFailed {
            code: diagnostic.code.as_str().to_string(),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        })
    }

    pub fn push(
        &mut self,
        code: VerificationCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(VerificationDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }

    pub fn is_clean_and<F>(&self, condition: F) -> bool
    where
        F: FnOnce() -> bool,
    {
        self.is_ok() && condition()
    }
}

pub fn verify_layout(desc: &LayoutDescription, registry: &L2KernelRegistry) -> VerificationReport {
    let mut report = VerificationReport::default();
    if !is_supported_data_type(&desc.data_type) {
        report.push(
            VerificationCode::UnsupportedType,
            "$.data_type",
            format!("unsupported output type {:?}", desc.data_type),
        );
        return report;
    }

    let actual_count = verify_node(&desc.root, &desc.data_type, "$.root", registry, &mut report);
    if let Some(actual_count) = actual_count {
        if actual_count != desc.row_count {
            report.push(
                VerificationCode::CountMismatch,
                "$.row_count",
                format!(
                    "layout row_count {} does not match root count {actual_count}",
                    desc.row_count
                ),
            );
        }
    }
    report
}

fn verify_node(
    node: &LayoutNode,
    data_type: &DataType,
    path: &str,
    registry: &L2KernelRegistry,
    report: &mut VerificationReport,
) -> Option<usize> {
    match node {
        LayoutNode::Raw {
            data,
            elem_size,
            count,
        } => verify_raw(data, *elem_size, *count, data_type, path, report),
        LayoutNode::BitPack {
            values_buf,
            bit_width,
            offset,
            count,
            validity,
            all_null,
        } => verify_bitpack(
            values_buf,
            *bit_width,
            *offset,
            *count,
            validity.as_deref(),
            *all_null,
            data_type,
            path,
            report,
        ),
        LayoutNode::FrameOfReference { inner, .. } => {
            if !matches!(data_type, DataType::Int32 | DataType::Int64) {
                report.push(
                    VerificationCode::UnsupportedLayoutType,
                    path,
                    format!("FrameOfReference cannot produce {:?}", data_type),
                );
            }
            verify_node(inner, data_type, &format!("{path}.inner"), registry, report)
        }
        LayoutNode::Dictionary { codes, values } => {
            let codes_type = dictionary_code_data_type(codes);
            let code_count = verify_node(
                codes,
                &codes_type,
                &format!("{path}.codes"),
                registry,
                report,
            );
            let values_count = verify_node(
                values,
                data_type,
                &format!("{path}.values"),
                registry,
                report,
            );
            verify_dictionary_codes(codes, values_count, &format!("{path}.codes"), report);
            code_count
        }
        LayoutNode::RunEnd {
            run_ends,
            values,
            count,
        } => {
            let run_count = verify_node(
                run_ends,
                &DataType::Int64,
                &format!("{path}.run_ends"),
                registry,
                report,
            );
            let value_count = verify_node(
                values,
                data_type,
                &format!("{path}.values"),
                registry,
                report,
            );
            if let (Some(run_count), Some(value_count)) = (run_count, value_count) {
                if value_count < run_count {
                    report.push(
                        VerificationCode::InsufficientRunValues,
                        format!("{path}.values"),
                        format!("values has {value_count} rows but run_ends has {run_count} rows"),
                    );
                }
            }
            verify_run_ends(run_ends, *count, &format!("{path}.run_ends"), report);
            Some(*count)
        }
        LayoutNode::KernelEscape {
            kernel_id,
            params,
            count,
        } => {
            if registry.get(*kernel_id).is_none() {
                report.push(
                    VerificationCode::UnknownKernel,
                    format!("{path}.kernel_id"),
                    format!("unknown L2 kernel id {kernel_id}"),
                );
            } else {
                verify_kernel_params(*kernel_id, params, *count, data_type, path, report);
            }
            Some(*count)
        }
    }
}

fn verify_kernel_params(
    kernel_id: u32,
    params: &[u8],
    count: usize,
    data_type: &DataType,
    path: &str,
    report: &mut VerificationReport,
) {
    match kernel_id {
        0 => {
            if !matches!(data_type, DataType::Utf8) {
                report.push(
                    VerificationCode::UnsupportedLayoutType,
                    path,
                    format!("FSST kernel produces Utf8, not {:?}", data_type),
                );
            }
            if let Err(err) = FsstParams::decode(params, count) {
                report.push(
                    VerificationCode::MalformedKernelParams,
                    format!("{path}.params"),
                    err.to_string(),
                );
            }
        }
        1 => {
            if !matches!(data_type, DataType::Float32 | DataType::Float64) {
                report.push(
                    VerificationCode::UnsupportedLayoutType,
                    path,
                    format!("ALP kernel produces Float32/Float64, not {:?}", data_type),
                );
            }
            match AlpParams::decode(params, count) {
                Ok(alp_params) => {
                    if &alp_params.output_type.to_data_type() != data_type {
                        report.push(
                            VerificationCode::UnsupportedLayoutType,
                            format!("{path}.params.output_type"),
                            format!(
                                "ALP params output type {} does not match layout type {:?}",
                                alp_params.output_type.as_str(),
                                data_type
                            ),
                        );
                    }
                }
                Err(err) => report.push(
                    VerificationCode::MalformedKernelParams,
                    format!("{path}.params"),
                    err.to_string(),
                ),
            }
        }
        _ => {}
    }
}

fn verify_raw(
    data: &[u8],
    elem_size: u8,
    count: usize,
    data_type: &DataType,
    path: &str,
    report: &mut VerificationReport,
) -> Option<usize> {
    if !raw_elem_size_supported(data_type, elem_size) {
        report.push(
            VerificationCode::UnsupportedLayoutType,
            format!("{path}.elem_size"),
            format!("Raw elem_size {elem_size} cannot produce {:?}", data_type),
        );
        return Some(count);
    }

    let needed = count.checked_mul(elem_size as usize);
    match needed {
        Some(needed) if data.len() >= needed => {}
        Some(needed) => report.push(
            VerificationCode::BufferTooShort,
            format!("{path}.data"),
            format!("raw buffer needs {needed} bytes, got {}", data.len()),
        ),
        None => report.push(
            VerificationCode::BufferTooShort,
            format!("{path}.data"),
            format!("raw buffer length overflows: count {count}, elem_size {elem_size}"),
        ),
    }
    Some(count)
}

#[allow(clippy::too_many_arguments)]
fn verify_bitpack(
    values_buf: &[u8],
    bit_width: u8,
    offset: u16,
    count: usize,
    validity: Option<&[bool]>,
    all_null: bool,
    data_type: &DataType,
    path: &str,
    report: &mut VerificationReport,
) -> Option<usize> {
    if let Some(validity) = validity {
        if validity.len() != count {
            report.push(
                VerificationCode::ValidityMismatch,
                format!("{path}.validity"),
                format!("validity has {} rows but count is {count}", validity.len()),
            );
        }
    }

    let Some(t_bits) = integer_bits(data_type) else {
        report.push(
            VerificationCode::UnsupportedLayoutType,
            path,
            format!("BitPack cannot produce {:?}", data_type),
        );
        return Some(count);
    };

    if bit_width == 0 {
        report.push(
            VerificationCode::InvalidBitWidth,
            format!("{path}.bit_width"),
            "bit_width must be at least 1",
        );
        return Some(count);
    }

    if all_null {
        return Some(count);
    }

    if bit_width as usize > t_bits {
        report.push(
            VerificationCode::InvalidBitWidth,
            format!("{path}.bit_width"),
            format!("bit_width {bit_width} exceeds {t_bits}-bit output type"),
        );
        return Some(count);
    }

    if (offset as usize).checked_add(count).is_none() {
        report.push(
            VerificationCode::CountMismatch,
            format!("{path}.count"),
            format!("offset {offset} plus count {count} overflows"),
        );
        return Some(count);
    }

    if let Err(err) = bitpack::unpack_all(
        values_buf,
        bit_width as usize,
        t_bits,
        offset as usize,
        count,
    ) {
        let code = match err {
            LoomDecodeError::BufferTooShort { .. } => VerificationCode::BufferTooShort,
            LoomDecodeError::BitWidthExceedsType { .. } => VerificationCode::InvalidBitWidth,
            LoomDecodeError::UnsupportedWidth(_) => VerificationCode::UnsupportedLayoutType,
            _ => VerificationCode::CountMismatch,
        };
        report.push(code, path, err.to_string());
    }
    Some(count)
}

fn verify_dictionary_codes(
    codes: &LayoutNode,
    values_count: Option<usize>,
    path: &str,
    report: &mut VerificationReport,
) {
    let Some(values_count) = values_count else {
        return;
    };
    let Some(codes) = raw_integer_values(codes) else {
        return;
    };
    for (idx, code) in codes.into_iter().enumerate() {
        if code < 0 || code as usize >= values_count {
            report.push(
                VerificationCode::InvalidDictionaryCode,
                format!("{path}[{idx}]"),
                format!("dictionary code {code} is outside values length {values_count}"),
            );
        }
    }
}

fn verify_run_ends(
    run_ends: &LayoutNode,
    count: usize,
    path: &str,
    report: &mut VerificationReport,
) {
    let Some(run_ends) = raw_integer_values(run_ends) else {
        return;
    };
    let mut previous = 0usize;
    for (idx, run_end) in run_ends.iter().copied().enumerate() {
        if run_end <= previous as i64 {
            report.push(
                VerificationCode::InvalidRunEnd,
                format!("{path}[{idx}]"),
                format!("run end {run_end} is not greater than previous {previous}"),
            );
            return;
        }
        let current = run_end as usize;
        if current > count {
            report.push(
                VerificationCode::InvalidRunEnd,
                format!("{path}[{idx}]"),
                format!("run end {current} exceeds count {count}"),
            );
            return;
        }
        previous = current;
    }
    if previous != count {
        report.push(
            VerificationCode::InvalidRunEnd,
            path,
            format!("final run end {previous} does not equal count {count}"),
        );
    }
}

fn raw_integer_values(node: &LayoutNode) -> Option<Vec<i64>> {
    let LayoutNode::Raw {
        data,
        elem_size,
        count,
    } = node
    else {
        return None;
    };
    let needed = count.checked_mul(*elem_size as usize)?;
    if data.len() < needed {
        return None;
    }

    let mut out = Vec::with_capacity(*count);
    for i in 0..*count {
        let start = i * *elem_size as usize;
        let bytes = &data[start..start + *elem_size as usize];
        let value = match elem_size {
            1 => i8::from_le_bytes([bytes[0]]) as i64,
            2 => i16::from_le_bytes(bytes.try_into().ok()?) as i64,
            4 => i32::from_le_bytes(bytes.try_into().ok()?) as i64,
            8 => i64::from_le_bytes(bytes.try_into().ok()?),
            _ => return None,
        };
        out.push(value);
    }
    Some(out)
}

fn is_supported_data_type(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Boolean
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
            | DataType::Utf8
    )
}

fn raw_elem_size_supported(data_type: &DataType, elem_size: u8) -> bool {
    matches!(
        (data_type, elem_size),
        (DataType::Boolean, 1)
            | (DataType::Int32, 1 | 2 | 4)
            | (DataType::Int64, 1 | 2 | 4 | 8)
            | (DataType::Float32, 4)
            | (DataType::Float64, 8)
    )
}

fn integer_bits(data_type: &DataType) -> Option<usize> {
    match data_type {
        DataType::Int32 => Some(32),
        DataType::Int64 => Some(64),
        _ => None,
    }
}

fn dictionary_code_data_type(codes: &LayoutNode) -> DataType {
    match codes {
        LayoutNode::Raw { elem_size: 8, .. } => DataType::Int64,
        _ => DataType::Int32,
    }
}
