//! KernelEscape routing tests through the public Phase-4 decode helper.

use arrow_schema::DataType;
use loom_core::error::LoomDecodeError;
use loom_core::l1_model::{decode_layout_to_array_data, LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;

#[test]
fn kernel_escape_zero_returns_empty_utf8_array() {
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: LayoutNode::KernelEscape {
            kernel_id: 0,
            params: vec![],
            count: 0,
        },
        row_count: 0,
    };
    let registry = L2KernelRegistry::default_for_mvp0();
    let data = decode_layout_to_array_data(&desc, &registry).expect("kernel id 0 should decode");

    assert_eq!(data.data_type(), &DataType::Utf8);
    assert_eq!(data.len(), 0);
}

#[test]
fn kernel_escape_unknown_id_returns_typed_error() {
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: LayoutNode::KernelEscape {
            kernel_id: 99,
            params: vec![],
            count: 0,
        },
        row_count: 0,
    };
    let registry = L2KernelRegistry::default_for_mvp0();

    assert!(matches!(
        decode_layout_to_array_data(&desc, &registry),
        Err(LoomDecodeError::UnknownKernel(99))
    ));
}
