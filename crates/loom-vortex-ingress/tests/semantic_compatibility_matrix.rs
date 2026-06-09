use loom_vortex_ingress::{
    semantic_report_from_vortex_coverages, semantic_row_from_vortex_coverage,
    validate_semantic_compatibility_row, VortexEmissionDisposition, VortexEncodingCoverage,
    VortexLoweringDisposition, VortexReaderEmissionKind, VortexReaderSupport,
    VortexSemanticCompatibilityRow, VortexSemanticNativeClass, VortexSemanticOracleClass,
    VortexSemanticRuntimeClass, VortexSemanticSupport, VortexSemanticVerifierClass,
};

fn coverage(
    dtype_kind: &str,
    nullable: Option<bool>,
    array_encoding: &str,
    support: VortexReaderSupport,
    emission_kind: VortexReaderEmissionKind,
    emission_disposition: VortexEmissionDisposition,
    lowering_disposition: VortexLoweringDisposition,
) -> VortexEncodingCoverage {
    VortexEncodingCoverage {
        dtype_kind: dtype_kind.to_string(),
        nullable,
        root_layout_encoding: array_encoding.to_string(),
        layout_class: "primitive-or-leaf".to_string(),
        array_encoding: array_encoding.to_string(),
        has_splits: false,
        has_statistics: false,
        reader_support: support,
        emission_kind,
        emission_disposition,
        lowering_disposition,
        notes: vec!["phase28-matrix-row".to_string()],
    }
}

#[test]
fn semantic_row_status_strings_are_stable() {
    assert_eq!(
        VortexSemanticSupport::AcceptedNative.as_str(),
        "accepted-native"
    );
    assert_eq!(
        VortexSemanticSupport::AcceptedCanonicalized.as_str(),
        "accepted-canonicalized"
    );
    assert_eq!(
        VortexSemanticOracleClass::VortexValueAndShape.as_str(),
        "vortex-value-and-shape"
    );
    assert_eq!(
        VortexSemanticVerifierClass::ArtifactVerifierAccepted.as_str(),
        "artifact-verifier-accepted"
    );
    assert_eq!(
        VortexSemanticRuntimeClass::NativeCandidate.as_str(),
        "native-candidate"
    );
    assert_eq!(
        VortexSemanticNativeClass::ExecutionEngineValidated.as_str(),
        "execution-engine-validated"
    );
}

#[test]
fn coverage_mapping_preserves_canonical_raw_boundary() {
    let dict = coverage(
        "primitive",
        Some(false),
        "dictionary",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmp1,
        VortexEmissionDisposition::CanonicalRaw,
        VortexLoweringDisposition::InterpreterOnly,
    );

    let row = semantic_row_from_vortex_coverage("dict-i32", &dict);

    assert_eq!(row.support, VortexSemanticSupport::AcceptedInterpreter);
    assert_eq!(row.emitted_loom_shape, "LMC1(LMP1)/canonical-raw");
    assert_eq!(row.oracle_class, VortexSemanticOracleClass::VortexValueRows);
    assert_eq!(row.deferral_reason, "structured-dictionary-facts-deferred");
    assert!(validate_semantic_compatibility_row(&row).is_empty());
}

#[test]
fn invalid_rows_fail_closed() {
    let structured_overclaim = VortexSemanticCompatibilityRow {
        shape_id: "dict-overclaim".to_string(),
        original_vortex_shape: "primitive:non-null:primitive-or-leaf:dictionary".to_string(),
        emitted_loom_shape: "LMC1(LMP1)/canonical-raw".to_string(),
        support: VortexSemanticSupport::AcceptedStructured,
        oracle_class: VortexSemanticOracleClass::VortexValueRows,
        verifier_class: VortexSemanticVerifierClass::ArtifactVerifierAccepted,
        runtime_class: VortexSemanticRuntimeClass::DuckDbVisible,
        native_class: VortexSemanticNativeClass::InterpreterOnly,
        deferral_reason: String::new(),
        evidence_notes: vec![],
    };
    let diagnostics = validate_semantic_compatibility_row(&structured_overclaim);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.ends_with("canonical-raw-overclaim")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.ends_with("structured-shape-oracle-missing")));

    let missing_native_evidence = VortexSemanticCompatibilityRow {
        shape_id: "native-no-ee".to_string(),
        native_class: VortexSemanticNativeClass::ExecutionEngineValidated,
        support: VortexSemanticSupport::AcceptedNative,
        oracle_class: VortexSemanticOracleClass::VortexValueRows,
        verifier_class: VortexSemanticVerifierClass::ArtifactVerifierAccepted,
        runtime_class: VortexSemanticRuntimeClass::NativeCandidate,
        original_vortex_shape: "primitive:non-null:primitive-or-leaf:primitive".to_string(),
        emitted_loom_shape: "LMC1(LMP1)/canonical-raw".to_string(),
        deferral_reason: String::new(),
        evidence_notes: vec![],
    };
    let diagnostics = validate_semantic_compatibility_row(&missing_native_evidence);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.ends_with("native-evidence-missing")));
}

#[test]
fn phase21_rows_have_semantic_compatibility_entries() {
    let primitive = coverage(
        "primitive",
        Some(false),
        "primitive",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmp1,
        VortexEmissionDisposition::CanonicalRaw,
        VortexLoweringDisposition::ProductionLoweringSupported,
    );
    let table = coverage(
        "struct",
        Some(false),
        "struct",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmt1,
        VortexEmissionDisposition::CanonicalTable,
        VortexLoweringDisposition::ProductionLoweringSupported,
    );
    let dictionary = coverage(
        "primitive",
        Some(false),
        "dictionary",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmp1,
        VortexEmissionDisposition::CanonicalRaw,
        VortexLoweringDisposition::InterpreterOnly,
    );
    let run_end = coverage(
        "primitive",
        Some(false),
        "run-end",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmp1,
        VortexEmissionDisposition::CanonicalRaw,
        VortexLoweringDisposition::InterpreterOnly,
    );
    let bitpack = coverage(
        "primitive",
        Some(false),
        "bitpack",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmp1,
        VortexEmissionDisposition::CanonicalRaw,
        VortexLoweringDisposition::InterpreterOnly,
    );
    let for_encoding = coverage(
        "primitive",
        Some(false),
        "frame-of-reference",
        VortexReaderSupport::Accepted,
        VortexReaderEmissionKind::Lmp1,
        VortexEmissionDisposition::CanonicalRaw,
        VortexLoweringDisposition::InterpreterOnly,
    );
    let nullable = coverage(
        "primitive",
        Some(true),
        "primitive",
        VortexReaderSupport::Unsupported,
        VortexReaderEmissionKind::None,
        VortexEmissionDisposition::None,
        VortexLoweringDisposition::FailClosedDeferred,
    );
    let string = coverage(
        "utf8",
        Some(false),
        "varbin",
        VortexReaderSupport::Unsupported,
        VortexReaderEmissionKind::None,
        VortexEmissionDisposition::None,
        VortexLoweringDisposition::FailClosedDeferred,
    );

    let report = semantic_report_from_vortex_coverages([
        ("primitive-i32", &primitive),
        ("struct-table", &table),
        ("dictionary-i32", &dictionary),
        ("run-end-i32", &run_end),
        ("bitpack-i32", &bitpack),
        ("for-i32", &for_encoding),
        ("nullable-i32", &nullable),
        ("utf8", &string),
    ]);

    assert_eq!(report.rows.len(), 8);
    assert!(report.diagnostics.is_empty());
    assert!(report
        .rows
        .iter()
        .any(|row| row.native_class == VortexSemanticNativeClass::ProductionLoweringSupported));
    assert!(report
        .rows
        .iter()
        .any(|row| row.deferral_reason == "nullable-validity-emission-deferred"));
}
