use loom_ffi::report::MlirToolKind;
use loom_ffi::toolchain::{
    parse_llvm_major_version, probe_toolchain, EXPECTED_MLIR_MAJOR,
};

#[test]
fn parses_known_versions_and_classifies_local_mismatch_shape() {
    assert_eq!(EXPECTED_MLIR_MAJOR, 22);
    assert_eq!(parse_llvm_major_version("21.1.2"), Some(21));
    assert_ne!(
        parse_llvm_major_version("21.1.2"),
        Some(EXPECTED_MLIR_MAJOR)
    );
    assert_eq!(parse_llvm_major_version("22.0.0git"), Some(22));
}

#[test]
fn probe_reports_all_required_tool_slots() {
    let facts = probe_toolchain();
    for kind in [
        MlirToolKind::LlvmConfig,
        MlirToolKind::MlirOpt,
        MlirToolKind::MlirTranslate,
        MlirToolKind::Lli,
    ] {
        assert!(facts.tool(kind).is_some(), "missing tool fact for {kind:?}");
    }
}
