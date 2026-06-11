//! Dictionary-over-FSST fixture tests against real Vortex arrays.

use arrow::array::{Array, StringArray};
use arrow_schema::DataType;
use loom_ffi::l1_model::{decode_layout_to_array_data, LayoutDescription};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use vortex_array::arrays::{DictArray, PrimitiveArray, VarBinArray};
use vortex_array::dtype::{DType, Nullability};
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;

use loom_fixtures::oracle;
use loom_fixtures::vortex_reader;

#[test]
fn dict_over_fsst_matches_vortex_oracle() {
    let values = VarBinArray::from_iter(
        [Some("alpha"), Some("beta"), Some("gamma")],
        DType::Utf8(Nullability::Nullable),
    );
    let compressor = vortex_fsst::fsst_train_compressor(&values);
    let mut ctx = vortex_array::LEGACY_SESSION.create_execution_ctx();
    let fsst_values =
        vortex_fsst::fsst_compress(&values, values.len(), values.dtype(), &compressor, &mut ctx);
    let codes = PrimitiveArray::from_iter([1i32, 0, 2, 1]);
    let dict = DictArray::try_new(codes.into_array(), fsst_values.into_array())
        .expect("DictArray::try_new failed");

    let (oracle_values, oracle_nulls) = oracle::decode_utf8_oracle(&dict.as_array().clone());
    let node = vortex_reader::from_dict_array(&dict);
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: node,
        row_count: oracle_values.len(),
    };
    let registry = L2KernelRegistry::default_for_mvp0();

    let data = decode_layout_to_array_data(&desc, &registry).expect("Loom dict FSST decode failed");
    let decoded = StringArray::from(data);

    assert_eq!(decoded.len(), oracle_values.len());
    for row in 0..decoded.len() {
        assert_eq!(
            decoded.is_null(row),
            oracle_nulls[row],
            "null mismatch at {row}"
        );
        if let Some(expected) = &oracle_values[row] {
            assert_eq!(decoded.value(row), expected, "value mismatch at {row}");
        }
    }
}
