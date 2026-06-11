use loom_vortex_ingress::{
    semantic_row_from_vortex_coverage, validate_semantic_compatibility_row,
    VortexEmissionDisposition, VortexEncodingCoverage, VortexLoweringDisposition,
    VortexReaderEmissionKind, VortexReaderSupport, VortexSemanticSupport,
    VortexSemanticVerifierClass,
};

fn nullable_unsupported(dtype_kind: &str, shape_id: &str) {
    let coverage = VortexEncodingCoverage {
        dtype_kind: dtype_kind.to_string(),
        nullable: Some(true),
        root_layout_encoding: "primitive".to_string(),
        layout_class: "primitive-or-leaf".to_string(),
        array_encoding: "primitive".to_string(),
        has_splits: false,
        has_statistics: false,
        reader_support: VortexReaderSupport::Unsupported,
        emission_kind: VortexReaderEmissionKind::None,
        emission_disposition: VortexEmissionDisposition::None,
        lowering_disposition: VortexLoweringDisposition::FailClosedDeferred,
        notes: vec!["nullable primitive validity bitmap is not emitted yet".to_string()],
    };

    let row = semantic_row_from_vortex_coverage(shape_id, &coverage);

    assert_eq!(row.support, VortexSemanticSupport::Unsupported);
    assert_eq!(row.emitted_loom_shape, "none");
    assert_eq!(
        row.verifier_class,
        VortexSemanticVerifierClass::UnsupportedNoEmission
    );
    assert_eq!(row.deferral_reason, "nullable-validity-emission-deferred");
    assert!(validate_semantic_compatibility_row(&row).is_empty());
}

#[test]
fn nullable_i32_semantics_are_accepted_or_explicitly_unsupported() {
    nullable_unsupported("primitive/i32", "nullable-i32");
}

#[test]
fn nullable_i64_semantics_are_accepted_or_explicitly_unsupported() {
    nullable_unsupported("primitive/i64", "nullable-i64");
}

#[test]
fn nullable_f32_semantics_are_accepted_or_explicitly_unsupported() {
    nullable_unsupported("primitive/f32", "nullable-f32");
}

#[test]
fn nullable_f64_semantics_are_accepted_or_explicitly_unsupported() {
    nullable_unsupported("primitive/f64", "nullable-f64");
}
