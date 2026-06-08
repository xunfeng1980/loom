//! ALP Float32/Float64 fixture coverage.
//!
//! Vortex 0.74.0 does not expose a public ALP array API in this repo's pinned
//! dependency set. These tests therefore use Loom-owned synthetic ALP params
//! for the kernel path and Vortex primitive float arrays as row-value oracle
//! truth for the same decoded values and null flags.

use arrow::array::{Array, Float32Array, Float64Array};
use arrow_schema::DataType;
use loom_core::alp_params::{AlpOutputType, AlpParams};
use loom_core::l1_model::{decode_layout_to_array_data, LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_fixtures::oracle::{decode_f32_oracle, decode_f64_oracle};
use vortex_array::arrays::PrimitiveArray;
use vortex_array::IntoArray;

#[test]
fn alp_float32_matches_known_values_and_vortex_primitive_oracle() {
    let expected = [Some(1.25f32), Some(-2.5), Some(0.0), Some(1.25), None];
    let desc = alp_desc(
        DataType::Float32,
        AlpParams {
            output_type: AlpOutputType::Float32,
            decimal_exponent: -2,
            mantissas: vec![125, -250, 0, 125, -250],
            validity: Some(expected.iter().map(Option::is_some).collect()),
        },
    );

    let registry = L2KernelRegistry::default_for_mvp0();
    let data = decode_layout_to_array_data(&desc, &registry).expect("decode ALP Float32");
    let actual = Float32Array::from(data);
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.null_count(), 1);

    let oracle_input = PrimitiveArray::from_option_iter(expected).into_array();
    let (oracle_values, oracle_nulls) = decode_f32_oracle(&oracle_input);
    assert_f32_rows(&actual, &expected, &oracle_values, &oracle_nulls);
}

#[test]
fn alp_float64_matches_known_values_and_vortex_primitive_oracle() {
    let expected = [Some(10.125f64), Some(-3.5), Some(0.0), None, Some(10.125)];
    let desc = alp_desc(
        DataType::Float64,
        AlpParams {
            output_type: AlpOutputType::Float64,
            decimal_exponent: -3,
            mantissas: vec![10125, -3500, 0, -3500, 10125],
            validity: Some(expected.iter().map(Option::is_some).collect()),
        },
    );

    let registry = L2KernelRegistry::default_for_mvp0();
    let data = decode_layout_to_array_data(&desc, &registry).expect("decode ALP Float64");
    let actual = Float64Array::from(data);
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.null_count(), 1);

    let oracle_input = PrimitiveArray::from_option_iter(expected).into_array();
    let (oracle_values, oracle_nulls) = decode_f64_oracle(&oracle_input);
    assert_f64_rows(&actual, &expected, &oracle_values, &oracle_nulls);
}

fn alp_desc(data_type: DataType, params: AlpParams) -> LayoutDescription {
    let count = params.mantissas.len();
    LayoutDescription {
        data_type,
        root: LayoutNode::KernelEscape {
            kernel_id: 1,
            params: params.encode(),
            count,
        },
        row_count: count,
    }
}

fn assert_f32_rows(
    actual: &Float32Array,
    expected: &[Option<f32>],
    oracle_values: &[f32],
    oracle_nulls: &[bool],
) {
    assert_eq!(oracle_nulls, null_flags(expected));
    for row in 0..actual.len() {
        if oracle_nulls[row] {
            assert!(actual.is_null(row), "row {row} should be null");
        } else {
            assert_eq!(actual.value(row), expected[row].unwrap());
            assert_eq!(actual.value(row), oracle_values[row]);
        }
    }
}

fn assert_f64_rows(
    actual: &Float64Array,
    expected: &[Option<f64>],
    oracle_values: &[f64],
    oracle_nulls: &[bool],
) {
    assert_eq!(oracle_nulls, null_flags(expected));
    for row in 0..actual.len() {
        if oracle_nulls[row] {
            assert!(actual.is_null(row), "row {row} should be null");
        } else {
            assert_eq!(actual.value(row), expected[row].unwrap());
            assert_eq!(actual.value(row), oracle_values[row]);
        }
    }
}

fn null_flags<T>(rows: &[Option<T>]) -> Vec<bool> {
    rows.iter().map(Option::is_none).collect()
}
