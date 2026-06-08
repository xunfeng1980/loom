use std::panic::{catch_unwind, AssertUnwindSafe};

use arrow_schema::DataType;
use loom_core::container_codec::{
    decode_container, decode_layout_payload_maybe_container, decode_table_payload_maybe_container,
    wrap_layout_payload, Feature,
};
use loom_core::descriptor::descriptor_text_to_payload;
use loom_core::l1_model::{decode_layout_to_array_data, LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{decode_table_to_array_data, TableColumn, TableDescription};
use loom_core::verifier::{verify_container, verify_layout, verify_table, VerificationCode};

fn assert_no_panic<T>(f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).expect("safety contract surface must not panic")
}

fn raw_i32_desc(row_count: usize) -> LayoutDescription {
    LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: (0..row_count as i32)
                .flat_map(|value| value.to_le_bytes())
                .collect(),
            elem_size: 4,
            count: row_count,
        },
        row_count,
    }
}

fn wrapped_i32_payload() -> Vec<u8> {
    let payload = encode_layout_payload(&raw_i32_desc(2));
    wrap_layout_payload(&payload).expect("valid layout should wrap")
}

fn mutate_required_features(bytes: &mut [u8], required_features: u64) {
    bytes[8..16].copy_from_slice(&required_features.to_le_bytes());
}

fn mutate_version(bytes: &mut [u8], version: u16) {
    bytes[4..6].copy_from_slice(&version.to_le_bytes());
}

fn find_section_entry(bytes: &[u8], kind: u16) -> usize {
    let section_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let mut pos = 28usize;
    for _ in 0..section_count {
        let entry_kind = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap());
        if entry_kind == kind {
            return pos;
        }
        pos += 28;
    }
    panic!("section kind {kind} not found in test fixture")
}

fn diagnostic_codes(
    report: &loom_core::verifier::VerificationReport,
) -> Vec<VerificationCode> {
    report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn obl_12_02_container_malformed_bytes_do_not_panic() {
    let registry = L2KernelRegistry::default_for_mvp0();

    let truncated = b"LMC1".to_vec();
    assert_no_panic(|| assert!(decode_container(&truncated).is_err()));
    assert_no_panic(|| assert!(!verify_container(&truncated, &registry).is_ok()));

    let mut unsupported_version = wrapped_i32_payload();
    mutate_version(&mut unsupported_version, 99);
    assert_no_panic(|| assert!(decode_container(&unsupported_version).is_err()));
    assert_no_panic(|| assert!(!verify_container(&unsupported_version, &registry).is_ok()));

    let mut unknown_required = wrapped_i32_payload();
    mutate_required_features(
        &mut unknown_required,
        Feature::SingleColumnLmp1.mask() | (1u64 << 63),
    );
    assert_no_panic(|| assert!(decode_container(&unknown_required).is_err()));
    assert_no_panic(|| assert!(!verify_container(&unknown_required, &registry).is_ok()));

    let mut bad_section = wrapped_i32_payload();
    let layout_entry = find_section_entry(&bad_section, 2);
    bad_section[layout_entry + 4..layout_entry + 12]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert_no_panic(|| assert!(decode_container(&bad_section).is_err()));
    assert_no_panic(|| assert!(!verify_container(&bad_section, &registry).is_ok()));
}

#[test]
fn obl_12_03_raw_payload_parse_failures_do_not_panic() {
    assert_no_panic(|| assert!(decode_layout_payload_maybe_container(b"LMP1").is_err()));
    assert_no_panic(|| assert!(decode_table_payload_maybe_container(b"LMT1").is_err()));
    assert_no_panic(|| assert!(descriptor_text_to_payload("not valid ron").is_err()));
}

#[test]
fn obl_12_04_05_06_verifier_failure_blocks_arrow_output() {
    let registry = L2KernelRegistry::default_for_mvp0();

    let cases = vec![
        (
            "invalid bit width",
            LayoutDescription {
                data_type: DataType::Int64,
                root: LayoutNode::BitPack {
                    bit_width: 65,
                    offset: 0,
                    count: 1,
                    values_buf: vec![],
                    validity: None,
                    all_null: false,
                },
                row_count: 1,
            },
            VerificationCode::InvalidBitWidth,
        ),
        (
            "raw byte length mismatch",
            LayoutDescription {
                data_type: DataType::Int32,
                root: LayoutNode::Raw {
                    data: vec![1, 0],
                    elem_size: 4,
                    count: 1,
                },
                row_count: 1,
            },
            VerificationCode::BufferTooShort,
        ),
        (
            "validity length mismatch",
            LayoutDescription {
                data_type: DataType::Int32,
                root: LayoutNode::BitPack {
                    bit_width: 1,
                    offset: 0,
                    count: 2,
                    values_buf: vec![],
                    validity: Some(vec![true]),
                    all_null: true,
                },
                row_count: 2,
            },
            VerificationCode::ValidityMismatch,
        ),
        (
            "non-monotonic run ends",
            LayoutDescription {
                data_type: DataType::Boolean,
                root: LayoutNode::RunEnd {
                    count: 2,
                    run_ends: Box::new(LayoutNode::Raw {
                        data: [2i64, 1].into_iter().flat_map(i64::to_le_bytes).collect(),
                        elem_size: 8,
                        count: 2,
                    }),
                    values: Box::new(LayoutNode::Raw {
                        data: vec![1, 0],
                        elem_size: 1,
                        count: 2,
                    }),
                },
                row_count: 2,
            },
            VerificationCode::InvalidRunEnd,
        ),
        (
            "unknown kernel",
            LayoutDescription {
                data_type: DataType::Utf8,
                root: LayoutNode::KernelEscape {
                    kernel_id: 42,
                    count: 0,
                    params: vec![],
                },
                row_count: 0,
            },
            VerificationCode::UnknownKernel,
        ),
    ];

    for (name, desc, expected_code) in cases {
        let report = assert_no_panic(|| verify_layout(&desc, &registry));
        assert!(
            diagnostic_codes(&report).contains(&expected_code),
            "{name} should produce {expected_code:?}, got {:?}",
            report.diagnostics()
        );
        let decoded = assert_no_panic(|| decode_layout_to_array_data(&desc, &registry));
        assert!(decoded.is_err(), "{name} must not produce Arrow output");
    }
}

#[test]
fn obl_12_04_05_table_verifier_failure_blocks_arrow_output() {
    let registry = L2KernelRegistry::default_for_mvp0();
    let table = TableDescription {
        row_count: 2,
        columns: vec![TableColumn {
            name: "id".to_string(),
            layout: raw_i32_desc(1),
        }],
    };

    let report = assert_no_panic(|| verify_table(&table, &registry));
    assert!(
        diagnostic_codes(&report).contains(&VerificationCode::CountMismatch),
        "table row mismatch should produce count-mismatch, got {:?}",
        report.diagnostics()
    );

    let decoded = assert_no_panic(|| decode_table_to_array_data(&table, &registry));
    assert!(decoded.is_err(), "invalid table must not produce Arrow output");
}
