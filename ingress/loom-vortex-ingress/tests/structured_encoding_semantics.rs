use loom_vortex_ingress::{
    semantic_row_from_vortex_coverage, validate_semantic_compatibility_row,
    VortexEmissionDisposition, VortexEncodingCoverage, VortexLoweringDisposition,
    VortexReaderEmissionKind, VortexReaderSupport, VortexSemanticSupport,
};

fn canonicalized_structured_encoding(array_encoding: &str, shape_id: &str) {
    let coverage = VortexEncodingCoverage {
        dtype_kind: "primitive".to_string(),
        nullable: Some(false),
        root_layout_encoding: array_encoding.to_string(),
        layout_class: "primitive-or-leaf".to_string(),
        array_encoding: array_encoding.to_string(),
        has_splits: false,
        has_statistics: false,
        reader_support: VortexReaderSupport::Accepted,
        emission_kind: VortexReaderEmissionKind::Lmp1,
        emission_disposition: VortexEmissionDisposition::CanonicalRaw,
        lowering_disposition: VortexLoweringDisposition::InterpreterOnly,
        notes: vec!["oracle rows verified through Vortex scan".to_string()],
    };

    let row = semantic_row_from_vortex_coverage(shape_id, &coverage);

    assert_eq!(row.support, VortexSemanticSupport::AcceptedInterpreter);
    assert_eq!(row.emitted_loom_shape, "LMC1(LMP1)/canonical-raw");
    assert!(row.deferral_reason.starts_with("structured-"));
    assert!(validate_semantic_compatibility_row(&row).is_empty());
}

#[test]
fn dictionary_semantics_are_structured_or_canonicalized() {
    canonicalized_structured_encoding("dictionary", "dictionary-i32");
}

#[test]
fn run_end_semantics_are_structured_or_canonicalized() {
    canonicalized_structured_encoding("run-end", "run-end-i32");
}

#[test]
fn bitpack_semantics_are_structured_or_canonicalized() {
    canonicalized_structured_encoding("bitpack", "bitpack-i32");
}

#[test]
fn for_semantics_are_structured_or_canonicalized() {
    canonicalized_structured_encoding("frame-of-reference", "for-i32");
}
