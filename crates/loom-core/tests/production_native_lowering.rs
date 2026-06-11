use loom_core::artifact_verifier::{
    ArtifactVerificationFacts, ArtifactVerificationReport,
};
use loom_core::l2_core::{L2DataType, OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_core::production_native_lowering::{
    check_production_lowering_support, is_supported_primitive, ProductionLoweringBackend,
    ProductionLoweringDiagnosticCode, ProductionLoweringShape,
};

fn l2_facts(output_schema: Vec<OutputSchemaFact>) -> VerifiedArtifactFacts {
    VerifiedArtifactFacts {
        artifact_version: 1,
        required_features: vec!["test.production".to_string()],
        optional_features: vec![],
        accepted_feature_set: vec!["test.production".to_string()],
        input_ranges: Vec::new(),
        output_schema,
        row_count_bound: Some(4),
        loop_bounds: Vec::new(),
        resource_bounds: ResourceBudget::bounded_rows(4),
        builder_event_types: Vec::new(),
        capability_summary: Vec::new(),
        constraint_ids: vec!["c0".to_string()],
        proof_obligation_ids: vec!["p0".to_string()],
        kloom_discharged: true,
    }
}

fn column(builder_id: &str, arrow_type: L2DataType, nullable: bool) -> OutputSchemaFact {
    OutputSchemaFact {
        builder_id: builder_id.to_string(),
        arrow_type,
        nullable,
    }
}

fn accepted_report(
    payload_kind: &str,
    output_schema: Vec<OutputSchemaFact>,
) -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some(payload_kind.to_string());
    facts.row_count_bound = Some(4);
    // Phase A–C: constraints_discharged is always false in production.
    facts.constraints_discharged = false;
    facts.l2_core = Some(l2_facts(output_schema));
    ArtifactVerificationReport::accepted(facts)
}

fn first_code(report: &ArtifactVerificationReport) -> ProductionLoweringDiagnosticCode {
    check_production_lowering_support(report)
        .first_error()
        .expect("expected diagnostic")
        .code
}

#[test]
fn backend_and_diagnostic_strings_are_stable() {
    assert_eq!(
        ProductionLoweringBackend::LoomDecodeDialect.as_str(),
        "loom-decode-dialect"
    );
    assert_eq!(
        ProductionLoweringDiagnosticCode::ConstraintsNotDischarged.as_str(),
        "constraints-not-discharged"
    );
    assert_eq!(
        ProductionLoweringDiagnosticCode::UnsupportedNullability.as_str(),
        "unsupported-nullability"
    );
}

#[test]
fn supported_primitive_type_matrix_is_explicit() {
    for data_type in [
        L2DataType::Int32,
        L2DataType::Int64,
        L2DataType::Float32,
        L2DataType::Float64,
    ] {
        assert!(is_supported_primitive(&data_type), "{data_type:?}");
    }
    for data_type in [L2DataType::Boolean, L2DataType::Utf8] {
        assert!(!is_supported_primitive(&data_type), "{data_type:?}");
    }
}

#[test]
fn accepted_single_column_layout_is_supported() {
    let report = accepted_report(
        "LMP1 layout",
        vec![column("out0", L2DataType::Int32, false)],
    );
    let support = check_production_lowering_support(&report);

    assert!(
        support.is_supported(),
        "unexpected diagnostics: {:?}",
        support.diagnostics()
    );
    let facts = support.facts().expect("production facts");
    assert_eq!(facts.backend, ProductionLoweringBackend::LoomDecodeDialect);
    assert_eq!(facts.artifact_kind, "LMC1");
    assert_eq!(facts.payload_kind, "LMP1 layout");
    assert!(!facts.constraints_discharged);
    match &facts.shape {
        ProductionLoweringShape::SingleColumnPrimitive { row_count, column } => {
            assert_eq!(*row_count, 4);
            assert_eq!(column.builder_id, "out0");
            assert_eq!(column.arrow_type, L2DataType::Int32);
            assert!(!column.nullable);
        }
        other => panic!("unexpected shape: {other:?}"),
    }
}

#[test]
fn accepted_table_is_supported() {
    let report = accepted_report(
        "LMT1 table",
        vec![
            column("id", L2DataType::Int64, false),
            column("score", L2DataType::Float64, false),
        ],
    );
    let support = check_production_lowering_support(&report);

    assert!(
        support.is_supported(),
        "unexpected diagnostics: {:?}",
        support.diagnostics()
    );
    let facts = support.facts().expect("production facts");
    match &facts.shape {
        ProductionLoweringShape::PrimitiveTable { row_count, columns } => {
            assert_eq!(*row_count, 4);
            assert_eq!(columns.len(), 2);
            assert_eq!(columns[0].builder_id, "id");
            assert_eq!(columns[1].arrow_type, L2DataType::Float64);
        }
        other => panic!("unexpected shape: {other:?}"),
    }
}

#[test]
fn accepted_program_with_collected_constraints_is_supported() {
    // Phase A–C: lowering no longer gates on constraints_discharged.
    let report = accepted_report(
        "LMP1 layout",
        vec![column("out0", L2DataType::Int32, false)],
    );
    let support = check_production_lowering_support(&report);
    assert!(
        support.is_supported(),
        "unexpected diagnostics: {:?}",
        support.diagnostics()
    );
}

#[test]
fn rejected_reports_reject_before_facts() {
    let report = ArtifactVerificationReport::rejected(Vec::new());
    assert_eq!(
        first_code(&report),
        ProductionLoweringDiagnosticCode::VerifierRejected
    );
}

#[test]
fn missing_facts_reject() {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("LMP1 layout".to_string());
    facts.row_count_bound = Some(4);
    facts.constraints_discharged = false;
    let report = ArtifactVerificationReport::accepted(facts);

    assert_eq!(
        first_code(&report),
        ProductionLoweringDiagnosticCode::MissingL2Facts
    );
}

#[test]
fn missing_row_bound_rejects() {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("LMP1 layout".to_string());
    facts.constraints_discharged = false;
    facts.l2_core = Some(l2_facts(vec![column("out0", L2DataType::Int32, false)]));
    let report = ArtifactVerificationReport::accepted(facts);

    assert_eq!(
        first_code(&report),
        ProductionLoweringDiagnosticCode::MissingRowCountBound
    );
}

#[test]
fn unsupported_payload_type_and_nullability_reject() {
    let payload = accepted_report(
        "LMP2 future",
        vec![column("out0", L2DataType::Int32, false)],
    );
    assert_eq!(
        first_code(&payload),
        ProductionLoweringDiagnosticCode::UnsupportedPayload
    );

    let ty = accepted_report(
        "LMP1 layout",
        vec![column("out0", L2DataType::Utf8, false)],
    );
    assert_eq!(
        first_code(&ty),
        ProductionLoweringDiagnosticCode::UnsupportedType
    );

    let nullable = accepted_report(
        "LMP1 layout",
        vec![column("out0", L2DataType::Int32, true)],
    );
    assert_eq!(
        first_code(&nullable),
        ProductionLoweringDiagnosticCode::UnsupportedNullability
    );
}

#[test]
fn single_column_payload_rejects_multiple_columns() {
    let report = accepted_report(
        "LMP1 layout",
        vec![
            column("id", L2DataType::Int32, false),
            column("score", L2DataType::Float32, false),
        ],
    );

    assert_eq!(
        first_code(&report),
        ProductionLoweringDiagnosticCode::UnsupportedMultiColumnShape
    );
}
