use loom_vortex_ingress::{
    phase42_vortex_verified_native_coverage_report, validate_semantic_compatibility_row,
    VortexSemanticNativeClass, VortexSemanticSupport, VortexSemanticVerifierClass,
};

fn row(shape_id: &str) -> loom_vortex_ingress::VortexSemanticCompatibilityRow {
    phase42_vortex_verified_native_coverage_report()
        .rows
        .into_iter()
        .find(|row| row.shape_id == shape_id)
        .unwrap_or_else(|| panic!("missing Phase 42 Vortex row {shape_id}"))
}

#[test]
fn phase42_vortex_report_has_stable_rows_and_no_diagnostics() {
    let report = phase42_vortex_verified_native_coverage_report();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    let ids = report
        .rows
        .iter()
        .map(|row| row.shape_id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.contains(&"vortex-lmc2-fixed-width-primitive"));
    assert!(ids.contains(&"vortex-lmc2-utf8"));
    assert!(ids.contains(&"vortex-lmc2-struct-table"));
    assert!(ids.contains(&"vortex-canonical-dictionary-i32"));
    assert!(ids.contains(&"vortex-canonical-run-end-i32"));
    assert!(ids.contains(&"vortex-canonical-bitpack-i32"));
    assert!(ids.contains(&"vortex-canonical-for-i32"));
    assert!(ids.contains(&"vortex-nullable-validity-deferred"));
}

#[test]
fn native_vortex_row_requires_execution_engine_and_lineage_evidence() {
    let primitive = row("vortex-lmc2-fixed-width-primitive");

    assert_eq!(primitive.support, VortexSemanticSupport::AcceptedNative);
    assert_eq!(
        primitive.native_class,
        VortexSemanticNativeClass::ExecutionEngineValidated
    );
    assert!(primitive.emitted_loom_shape.starts_with("LMC2(LMA1)"));
    assert!(primitive
        .evidence_notes
        .iter()
        .any(|note| note.contains("native-arrow-semantic-codegen-output")));
    assert!(primitive
        .evidence_notes
        .iter()
        .any(|note| note.contains("verified-lineage-record")));
    assert!(validate_semantic_compatibility_row(&primitive).is_empty());
}

#[test]
fn canonical_vortex_rows_do_not_claim_original_structured_native_support() {
    for shape_id in [
        "vortex-canonical-dictionary-i32",
        "vortex-canonical-run-end-i32",
        "vortex-canonical-bitpack-i32",
        "vortex-canonical-for-i32",
    ] {
        let row = row(shape_id);
        assert!(row.emitted_loom_shape.contains("canonical-raw"));
        assert_ne!(row.support, VortexSemanticSupport::AcceptedStructured);
        assert_ne!(
            row.native_class,
            VortexSemanticNativeClass::ExecutionEngineValidated
        );
        assert!(row.deferral_reason.starts_with("structured-"));
        assert!(validate_semantic_compatibility_row(&row).is_empty());
    }
}

#[test]
fn deferred_vortex_rows_emit_no_artifact_and_no_positive_verifier_claim() {
    let nullable = row("vortex-nullable-validity-deferred");

    assert_eq!(nullable.support, VortexSemanticSupport::Unsupported);
    assert_eq!(nullable.emitted_loom_shape, "none");
    assert_eq!(
        nullable.verifier_class,
        VortexSemanticVerifierClass::UnsupportedNoEmission
    );
    assert_eq!(nullable.native_class, VortexSemanticNativeClass::Deferred);
    assert_eq!(
        nullable.deferral_reason,
        "nullable-validity-emission-deferred"
    );
}
