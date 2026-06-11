//! FSST fixture tests against real Vortex FSST construction.

use arrow::array::{Array, StringArray};
use arrow_schema::DataType;
use loom_ffi::l1_model::{decode_layout_to_array_data, LayoutDescription};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use vortex_array::arrays::VarBinArray;
use vortex_array::dtype::{DType, Nullability};
use vortex_array::VortexSessionExecute;
use vortex_fsst::FSSTArray;

use loom_vortex_ingress::oracle;
use loom_vortex_ingress::vortex_reader;

fn make_fsst(rows: &[Option<&str>], training_rows: &[Option<&str>]) -> FSSTArray {
    let values = VarBinArray::from_iter(rows.iter().copied(), DType::Utf8(Nullability::Nullable));
    let training = VarBinArray::from_iter(
        training_rows.iter().copied(),
        DType::Utf8(Nullability::Nullable),
    );
    let compressor = vortex_fsst::fsst_train_compressor(&training);
    let mut ctx = vortex_array::LEGACY_SESSION.create_execution_ctx();

    vortex_fsst::fsst_compress(&values, values.len(), values.dtype(), &compressor, &mut ctx)
}

fn assert_loom_matches_vortex(fsst: &FSSTArray) {
    let (oracle_values, oracle_nulls) = oracle::decode_utf8_oracle(&fsst.as_array().clone());
    let node = vortex_reader::from_fsst_array(fsst);
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: node,
        row_count: oracle_values.len(),
    };
    let registry = L2KernelRegistry::default_for_mvp0();

    let data = decode_layout_to_array_data(&desc, &registry).expect("Loom FSST decode failed");
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

#[test]
fn fsst_edge_cases_match_vortex_oracle() {
    let rows = [
        Some(""),
        Some("aaaaaaaa"),
        Some("abcdefgh"),
        Some("escape-heavy-xyz-123-!@#"),
        Some("short"),
    ];
    let training_rows = [Some("aaaaaaaa"), Some("aaaaaaaa")];
    let fsst = make_fsst(&rows, &training_rows);

    assert_loom_matches_vortex(&fsst);
}

#[test]
fn fsst_nulls_match_vortex_oracle() {
    let rows = [Some("alpha"), None, Some(""), Some("beta"), None];
    let fsst = make_fsst(&rows, &rows);

    assert_loom_matches_vortex(&fsst);
}
