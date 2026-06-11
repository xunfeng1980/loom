//! First-pass structural verifier for MVP0 layouts and table payloads.
//!
//! This verifier is deliberately scoped to the current MVP0 data model. It
//! validates structural invariants before decode and reports stable diagnostic
//! codes plus paths, while leaving value-dependent checks that require full
//! materialization to the existing typed decode errors.
//!
//! Plan 52-01: types, VerificationReport, VerificationCode, VerificationDiagnostic,
//! and `verify_layout` are extracted to `loom-common::verify_layout_types`.
//! `verify_table` and `verify_container` remain here because they depend on
//! container_codec and table_codec.

use std::collections::HashSet;

use crate::container_codec::{
    decode_container, decode_layout_payload_maybe_container, decode_table_payload_maybe_container,
    extract_wrapped_payload, WrappedPayload,
};
use loom_ir_core::error::LoomDecodeError;
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::table_codec::TableDescription;

// Types extracted to loom-common (plan 52-01)
pub use loom_common::verify_layout_types::*;

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

pub fn verify_container(bytes: &[u8], registry: &L2KernelRegistry) -> VerificationReport {
    let mut report = VerificationReport::default();
    match decode_container(bytes) {
        Ok(_) => {}
        Err(err) => {
            report.push(
                VerificationCode::ContainerShape,
                container_error_path(&err),
                err.to_string(),
            );
            return report;
        }
    }

    match extract_wrapped_payload(bytes) {
        Ok(WrappedPayload::Layout(_)) => match decode_layout_payload_maybe_container(bytes) {
            Ok(desc) => {
                let layout_report = verify_layout(&desc, registry);
                for diagnostic in layout_report.diagnostics() {
                    report.push(
                        diagnostic.code,
                        format!("$.payload{}", diagnostic.path.trim_start_matches('$')),
                        diagnostic.message.clone(),
                    );
                }
            }
            Err(err) => report.push(
                VerificationCode::ContainerShape,
                "$.sections.layout_payload",
                err.to_string(),
            ),
        },
        Ok(WrappedPayload::Table(_)) => match decode_table_payload_maybe_container(bytes) {
            Ok(table) => {
                let table_report = verify_table(&table, registry);
                for diagnostic in table_report.diagnostics() {
                    report.push(
                        diagnostic.code,
                        format!("$.payload{}", diagnostic.path.trim_start_matches('$')),
                        diagnostic.message.clone(),
                    );
                }
            }
            Err(err) => report.push(
                VerificationCode::ContainerShape,
                "$.sections.table_payload",
                err.to_string(),
            ),
        },
        Err(err) => report.push(
            VerificationCode::ContainerShape,
            container_error_path(&err),
            err.to_string(),
        ),
    }

    report
}

fn container_error_path(err: &LoomDecodeError) -> &'static str {
    match err {
        LoomDecodeError::MalformedContainer(reason) if reason.contains("feature") => {
            "$.required_features"
        }
        LoomDecodeError::MalformedContainer(reason) if reason.contains("version") => "$.version",
        LoomDecodeError::MalformedContainer(reason) if reason.contains("magic") => "$.magic",
        LoomDecodeError::MalformedContainer(reason) if reason.contains("section") => "$.sections",
        LoomDecodeError::MalformedContainer(reason) if reason.contains("header") => "$.header",
        LoomDecodeError::MalformedContainer(_) => "$.container",
        _ => "$.container",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_schema::DataType;
    use crate::alp_params::{AlpOutputType, AlpParams};
    use crate::container_codec::{wrap_layout_payload, wrap_table_payload};
    use crate::l1_model::{LayoutDescription, LayoutNode};
    use crate::layout_codec::encode_layout_payload;
    use crate::table_codec::{encode_table_payload, TableColumn};

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

    fn raw_i32_desc() -> LayoutDescription {
        LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Raw {
                data: [1i32, 2, 3]
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect(),
                elem_size: 4,
                count: 3,
            },
            row_count: 3,
        }
    }

    fn simple_table() -> TableDescription {
        TableDescription {
            row_count: 3,
            columns: vec![TableColumn {
                name: "value".to_string(),
                layout: raw_i32_desc(),
            }],
        }
    }

    #[test]
    fn valid_layout_container_passes() {
        let raw = encode_layout_payload(&raw_i32_desc());
        let wrapped = wrap_layout_payload(&raw).expect("wrap layout");

        let report = verify_container(&wrapped, &registry());

        assert!(report.is_ok(), "{:?}", report.diagnostics());
    }

    #[test]
    fn valid_table_container_passes() {
        let raw = encode_table_payload(&simple_table()).expect("encode table");
        let wrapped = wrap_table_payload(&raw).expect("wrap table");

        let report = verify_container(&wrapped, &registry());

        assert!(report.is_ok(), "{:?}", report.diagnostics());
    }

    #[test]
    fn container_unknown_required_feature_fails() {
        let raw = encode_layout_payload(&raw_i32_desc());
        let mut wrapped = wrap_layout_payload(&raw).expect("wrap layout");
        let required_features_offset = 4 + 2 + 2;
        let unknown_required = 1u64 << 63;
        wrapped[required_features_offset..required_features_offset + 8]
            .copy_from_slice(&unknown_required.to_le_bytes());

        let report = verify_container(&wrapped, &registry());

        assert_code(&report, VerificationCode::ContainerShape);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.path == "$.required_features"),
            "{:?}",
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
