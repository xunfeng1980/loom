//! First-pass structural verifier for MVP0 layouts and table payloads.
//!
//! This verifier is deliberately scoped to the current MVP0 data model. It
//! validates structural invariants before decode and reports stable diagnostic
//! codes plus paths, while leaving value-dependent checks that require full
//! materialization to the existing typed decode errors.

use std::collections::HashSet;
use std::fmt;

use arrow_schema::DataType;

use crate::alp_params::AlpParams;
use crate::error::LoomDecodeError;
use crate::fsst_params::FsstParams;
use crate::l1_model::bitpack;
use crate::l1_model::{LayoutDescription, LayoutNode};
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::table_codec::TableDescription;

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

    fn push(
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

pub fn verify_table(table: &TableDescription, registry: &L2KernelRegistry) -> VerificationReport {
    let mut report = VerificationReport::default();
    if table.columns.is_empty() {
        report.push(
            VerificationCode::TableShape,
            "$.columns",
            "table has no columns",
        );
    }

    let mut names = HashSet::new();
    for (idx, column) in table.columns.iter().enumerate() {
        let path = format!("$.columns[{idx}]");
        if column.name.is_empty() {
            report.push(
                VerificationCode::TableShape,
                format!("{path}.name"),
                "column name is empty",
            );
        }
        if !names.insert(column.name.as_str()) {
            report.push(
                VerificationCode::TableShape,
                format!("{path}.name"),
                format!("duplicate column name '{}'", column.name),
            );
        }
        if column.layout.row_count != table.row_count {
            report.push(
                VerificationCode::CountMismatch,
                format!("{path}.layout.row_count"),
                format!(
                    "column row_count {} does not match table row_count {}",
                    column.layout.row_count, table.row_count
                ),
            );
        }

        let column_report = verify_layout(&column.layout, registry);
        for diagnostic in column_report.diagnostics() {
            report.push(
                diagnostic.code,
                format!("{path}.layout{}", diagnostic.path.trim_start_matches('$')),
                diagnostic.message.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alp_params::{AlpOutputType, AlpParams};

    fn registry() -> L2KernelRegistry {
        L2KernelRegistry::default_for_mvp0()
    }

    fn report_for(root: LayoutNode, data_type: DataType, row_count: usize) -> VerificationReport {
        verify_layout(
            &LayoutDescription {
                data_type,
                root,
                row_count,
            },
            &registry(),
        )
    }

    fn assert_code(report: &VerificationReport, code: VerificationCode) {
        assert!(
            report.diagnostics().iter().any(|d| d.code == code),
            "expected {code}, got {:?}",
            report.diagnostics()
        );
    }

    #[test]
    fn raw_byte_mismatch_fails() {
        let report = report_for(
            LayoutNode::Raw {
                data: vec![1, 0],
                elem_size: 4,
                count: 1,
            },
            DataType::Int32,
            1,
        );
        assert_code(&report, VerificationCode::BufferTooShort);
    }

    #[test]
    fn bitpack_invalid_width_fails() {
        let report = report_for(
            LayoutNode::BitPack {
                values_buf: vec![],
                bit_width: 65,
                offset: 0,
                count: 1,
                validity: None,
                all_null: false,
            },
            DataType::Int64,
            1,
        );
        assert_code(&report, VerificationCode::InvalidBitWidth);
    }

    #[test]
    fn bitpack_zero_width_fails() {
        let report = report_for(
            LayoutNode::BitPack {
                values_buf: vec![],
                bit_width: 0,
                offset: 0,
                count: 1,
                validity: None,
                all_null: false,
            },
            DataType::Int32,
            1,
        );
        assert_code(&report, VerificationCode::InvalidBitWidth);
    }

    #[test]
    fn bitpack_validity_len_mismatch_fails() {
        let report = report_for(
            LayoutNode::BitPack {
                values_buf: vec![],
                bit_width: 1,
                offset: 0,
                count: 2,
                validity: Some(vec![true]),
                all_null: true,
            },
            DataType::Int32,
            2,
        );
        assert_code(&report, VerificationCode::ValidityMismatch);
    }

    #[test]
    fn for_over_boolean_fails() {
        let report = report_for(
            LayoutNode::FrameOfReference {
                reference: 1,
                inner: Box::new(LayoutNode::Raw {
                    data: vec![1],
                    elem_size: 1,
                    count: 1,
                }),
            },
            DataType::Boolean,
            1,
        );
        assert_code(&report, VerificationCode::UnsupportedLayoutType);
    }

    #[test]
    fn dictionary_kernel_codes_fail_as_non_integer_codes() {
        let report = report_for(
            LayoutNode::Dictionary {
                codes: Box::new(LayoutNode::KernelEscape {
                    kernel_id: 0,
                    params: vec![],
                    count: 1,
                }),
                values: Box::new(LayoutNode::Raw {
                    data: vec![1, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
            },
            DataType::Int32,
            1,
        );
        assert_code(&report, VerificationCode::UnsupportedLayoutType);
    }

    #[test]
    fn dictionary_raw_out_of_range_code_fails() {
        let report = report_for(
            LayoutNode::Dictionary {
                codes: Box::new(LayoutNode::Raw {
                    data: vec![2, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
                values: Box::new(LayoutNode::Raw {
                    data: vec![1, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                }),
            },
            DataType::Int32,
            1,
        );
        assert_code(&report, VerificationCode::InvalidDictionaryCode);
    }

    #[test]
    fn non_monotonic_raw_run_ends_fail() {
        let report = report_for(
            LayoutNode::RunEnd {
                run_ends: Box::new(LayoutNode::Raw {
                    data: vec![2, 0, 0, 0, 1, 0, 0, 0],
                    elem_size: 4,
                    count: 2,
                }),
                values: Box::new(LayoutNode::Raw {
                    data: vec![1, 0],
                    elem_size: 1,
                    count: 2,
                }),
                count: 2,
            },
            DataType::Boolean,
            2,
        );
        assert_code(&report, VerificationCode::InvalidRunEnd);
    }

    #[test]
    fn unknown_kernel_fails() {
        let report = report_for(
            LayoutNode::KernelEscape {
                kernel_id: 42,
                params: vec![],
                count: 0,
            },
            DataType::Utf8,
            0,
        );
        assert_code(&report, VerificationCode::UnknownKernel);
    }

    #[test]
    fn malformed_fsst_params_fails() {
        let report = report_for(
            LayoutNode::KernelEscape {
                kernel_id: 0,
                params: vec![],
                count: 0,
            },
            DataType::Utf8,
            0,
        );
        assert_code(&report, VerificationCode::MalformedKernelParams);
    }

    #[test]
    fn valid_alp_float32_passes() {
        let params = AlpParams {
            output_type: AlpOutputType::Float32,
            decimal_exponent: -2,
            mantissas: vec![125, -25],
            validity: Some(vec![true, false]),
        }
        .encode();
        let report = report_for(
            LayoutNode::KernelEscape {
                kernel_id: 1,
                params,
                count: 2,
            },
            DataType::Float32,
            2,
        );

        assert!(report.is_ok(), "{:?}", report.diagnostics());
    }

    #[test]
    fn malformed_alp_params_fails() {
        let report = report_for(
            LayoutNode::KernelEscape {
                kernel_id: 1,
                params: vec![],
                count: 0,
            },
            DataType::Float32,
            0,
        );

        assert_code(&report, VerificationCode::MalformedKernelParams);
    }

    #[test]
    fn alp_output_type_mismatch_fails() {
        let params = AlpParams {
            output_type: AlpOutputType::Float64,
            decimal_exponent: -2,
            mantissas: vec![125],
            validity: None,
        }
        .encode();
        let report = report_for(
            LayoutNode::KernelEscape {
                kernel_id: 1,
                params,
                count: 1,
            },
            DataType::Float32,
            1,
        );

        assert_code(&report, VerificationCode::UnsupportedLayoutType);
    }

    #[test]
    fn fsst_under_float_fails() {
        let report = report_for(
            LayoutNode::KernelEscape {
                kernel_id: 0,
                params: vec![],
                count: 0,
            },
            DataType::Float32,
            0,
        );

        assert_code(&report, VerificationCode::UnsupportedLayoutType);
    }

    #[test]
    fn table_duplicate_names_fail() {
        let table = TableDescription {
            row_count: 1,
            columns: vec![table_column("x", 1), table_column("x", 1)],
        };
        let report = verify_table(&table, &registry());
        assert_code(&report, VerificationCode::TableShape);
    }

    #[test]
    fn table_row_count_mismatch_fail() {
        let table = TableDescription {
            row_count: 2,
            columns: vec![table_column("x", 1)],
        };
        let report = verify_table(&table, &registry());
        assert_code(&report, VerificationCode::CountMismatch);
    }

    #[test]
    fn table_nested_path_uses_column_index() {
        let table = TableDescription {
            row_count: 1,
            columns: vec![TableDescriptionColumnBuilder::new("x")
                .root(LayoutNode::Raw {
                    data: vec![1],
                    elem_size: 4,
                    count: 1,
                })
                .build()],
        };
        let report = verify_table(&table, &registry());
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.path.starts_with("$.columns[0].layout.root")),
            "{:?}",
            report.diagnostics()
        );
    }

    fn table_column(name: &str, row_count: usize) -> crate::table_codec::TableColumn {
        TableDescriptionColumnBuilder::new(name)
            .row_count(row_count)
            .build()
    }

    struct TableDescriptionColumnBuilder {
        name: String,
        row_count: usize,
        root: LayoutNode,
    }

    impl TableDescriptionColumnBuilder {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                row_count: 1,
                root: LayoutNode::Raw {
                    data: vec![1, 0, 0, 0],
                    elem_size: 4,
                    count: 1,
                },
            }
        }

        fn row_count(mut self, row_count: usize) -> Self {
            self.row_count = row_count;
            self
        }

        fn root(mut self, root: LayoutNode) -> Self {
            self.root = root;
            self
        }

        fn build(self) -> crate::table_codec::TableColumn {
            crate::table_codec::TableColumn {
                name: self.name,
                layout: LayoutDescription {
                    data_type: DataType::Int32,
                    root: self.root,
                    row_count: self.row_count,
                },
            }
        }
    }
}
