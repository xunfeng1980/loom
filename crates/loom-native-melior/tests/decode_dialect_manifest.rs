use std::collections::BTreeSet;
use std::fs;

use loom_core::decode_dialect::DecodeDialectOp;
use loom_native_melior::decode_dialect_manifest::{
    decode_dialect_manifest, manifest_entry, ods_source_paths, DecodeDialectDisposition,
};

fn textual_ops() -> Vec<&'static str> {
    vec![
        DecodeDialectOp::Module.as_str(),
        DecodeDialectOp::Kernel.as_str(),
        DecodeDialectOp::InputSlice.as_str(),
        DecodeDialectOp::Column.as_str(),
        DecodeDialectOp::Builder.as_str(),
        DecodeDialectOp::Finish.as_str(),
        DecodeDialectOp::ForRows.as_str(),
        DecodeDialectOp::BitUnpack.as_str(),
        DecodeDialectOp::ForDelta.as_str(),
        DecodeDialectOp::ValidityAllValid.as_str(),
        DecodeDialectOp::ValidityCopy.as_str(),
    ]
}

#[test]
fn manifest_matches_textual_decode_dialect_names() {
    let manifest_names = decode_dialect_manifest()
        .iter()
        .map(|entry| entry.textual_name)
        .collect::<BTreeSet<_>>();
    let textual_names = textual_ops().into_iter().collect::<BTreeSet<_>>();

    assert_eq!(manifest_names, textual_names);
}

#[test]
fn ods_records_exist_in_source_files() {
    let [dialect_path, ops_path] = ods_source_paths();
    let ops_source = fs::read_to_string(ops_path).expect("ODS ops source should be readable");
    let dialect_source =
        fs::read_to_string(dialect_path).expect("ODS dialect source should be readable");

    assert!(dialect_source.contains("def LoomDecode_Dialect"));
    assert!(dialect_source.contains("not a substitute for Loom artifact verification"));

    for entry in decode_dialect_manifest() {
        assert!(
            ops_source.contains(entry.ods_record),
            "missing ODS record {} for {}",
            entry.ods_record,
            entry.textual_name
        );
        assert!(
            ops_source.contains(entry.textual_name),
            "missing textual op name {} in ODS source",
            entry.textual_name
        );
    }
}

#[test]
fn manifest_distinguishes_supported_guarded_and_interpreter_paths() {
    assert_eq!(
        manifest_entry("loom.decode.validity_all_valid").map(|entry| entry.disposition),
        Some(DecodeDialectDisposition::NativeSupported)
    );
    assert_eq!(
        manifest_entry("loom.decode.bit_unpack").map(|entry| entry.disposition),
        Some(DecodeDialectDisposition::DeclaredGuarded)
    );
    assert_eq!(
        manifest_entry("loom.decode.for_delta").map(|entry| entry.disposition),
        Some(DecodeDialectDisposition::DeclaredGuarded)
    );
    assert_eq!(
        manifest_entry("loom.decode.validity_copy").map(|entry| entry.disposition),
        Some(DecodeDialectDisposition::InterpreterOnly)
    );
    assert_eq!(DecodeDialectDisposition::Deferred.as_str(), "deferred");
}

#[test]
fn manifest_covers_expected_primitive_kernel_surface() {
    let bitpack = manifest_entry("loom.decode.bit_unpack").expect("bitpack manifest");
    assert!(bitpack.attributes.contains(&"bit_width"));

    let for_delta = manifest_entry("loom.decode.for_delta").expect("FOR manifest");
    assert!(for_delta.attributes.contains(&"reference"));
}

#[test]
fn default_manifest_tests_do_not_require_mlir_tblgen() {
    for path in ods_source_paths() {
        assert!(
            path.exists(),
            "expected ODS source path to exist: {}",
            path.display()
        );
    }
}
