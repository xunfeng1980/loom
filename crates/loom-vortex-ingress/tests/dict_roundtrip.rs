//! Dictionary fixture tests against real Vortex DictArray construction.

use std::sync::LazyLock;

use arrow::array::{Array, Int32Array};
use arrow_schema::DataType;
use loom_ffi::arrow_builder_output::OutputBuilder;
use loom_ffi::l1_model::synthesized_read_loop;
use vortex_array::arrays::{DictArray, PrimitiveArray};
use vortex_array::{IntoArray, VortexSessionExecute};
use vortex_fastlanes::BitPackedData;

use loom_vortex_ingress::oracle;
use loom_vortex_ingress::vortex_reader;

static SESSION: LazyLock<vortex_session::VortexSession> = LazyLock::new(|| {
    let session = vortex_session::VortexSession::empty();
    vortex_fastlanes::initialize(&session);
    session
});

#[test]
fn dict_integer_roundtrip() {
    let mut ctx = SESSION.create_execution_ctx();

    let values = PrimitiveArray::from_iter([10i32, 20, 30]);
    let codes = PrimitiveArray::from_iter([0i32, 1, 2, 1]);
    let codes = BitPackedData::encode(&codes.into_array(), 2, &mut ctx)
        .expect("BitPackedData::encode failed for dict codes");
    let dict = DictArray::try_new(codes.into_array(), values.into_array())
        .expect("DictArray::try_new failed");

    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&dict.as_array().clone());
    assert!(oracle_nulls.iter().all(|&is_null| !is_null));

    let node = vortex_reader::from_dict_array(&dict);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("loom dict decode failed");
    let data = builder.finish();

    assert_eq!(data.len(), oracle_values.len());
    assert_eq!(data.null_count(), 0);
    let decoded = Int32Array::from(data);
    assert_eq!(decoded.values(), oracle_values.as_slice());
}

#[test]
fn nullable_dict_roundtrip() {
    let mut ctx = SESSION.create_execution_ctx();

    let values = PrimitiveArray::from_iter([10i32, 20, 30]);
    let codes = PrimitiveArray::from_option_iter([Some(0i32), None, Some(2), Some(1)]);
    let codes = BitPackedData::encode(&codes.into_array(), 2, &mut ctx)
        .expect("BitPackedData::encode failed for nullable dict codes");
    let dict = DictArray::try_new(codes.into_array(), values.into_array())
        .expect("DictArray::try_new failed");

    let (oracle_values, oracle_nulls) = oracle::decode_i32_oracle(&dict.as_array().clone());
    let node = vortex_reader::from_dict_array(&dict);
    let mut builder = OutputBuilder::new(&DataType::Int32);
    synthesized_read_loop(&node, &mut builder).expect("loom nullable dict decode failed");
    let data = builder.finish();
    let decoded = Int32Array::from(data.clone());

    assert_eq!(data.len(), oracle_values.len());
    for i in 0..data.len() {
        let loom_is_null = decoded.is_null(i);
        assert_eq!(loom_is_null, oracle_nulls[i], "null mismatch at {i}");
        if !loom_is_null {
            assert_eq!(decoded.value(i), oracle_values[i], "value mismatch at {i}");
        }
    }
}
