use arrow_schema::DataType;
use loom_core::artifact_verifier::{
    ArtifactVerificationFacts, ArtifactVerificationReport, ConstraintDischargeStatus,
};
use loom_core::decode_dialect::{arrow_type_name, emit_decode_dialect_text, DecodeDialectOp};
use loom_core::l2_core::{OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_core::production_native_lowering::{
    check_production_lowering_support, lower_to_decode_dialect_text,
};

fn column(builder_id: &str, arrow_type: DataType) -> OutputSchemaFact {
    OutputSchemaFact {
        builder_id: builder_id.to_string(),
        arrow_type,
        nullable: false,
    }
}

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
    }
}

fn accepted_report(
    payload_kind: &str,
    status: ConstraintDischargeStatus,
) -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some(payload_kind.to_string());
    facts.row_count_bound = Some(4);
    facts.constraint_status = status;
    facts.l2_core = Some(l2_facts(vec![column("out0", DataType::Int32)]));
    ArtifactVerificationReport::accepted(facts)
}

fn accepted_table_report() -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("LMT1 table".to_string());
    facts.row_count_bound = Some(4);
    facts.constraint_status = ConstraintDischargeStatus::NotRequired;
    facts.l2_core = Some(l2_facts(vec![
        column("id", DataType::Int64),
        column("score", DataType::Float64),
    ]));
    ArtifactVerificationReport::accepted(facts)
}

#[test]
fn dialect_op_names_are_stable() {
    assert_eq!(DecodeDialectOp::Module.as_str(), "loom.decode.module");
    assert_eq!(DecodeDialectOp::Kernel.as_str(), "loom.decode.kernel");
    assert_eq!(
        DecodeDialectOp::InputSlice.as_str(),
        "loom.decode.input_slice"
    );
    assert_eq!(DecodeDialectOp::Column.as_str(), "loom.decode.column");
    assert_eq!(DecodeDialectOp::Builder.as_str(), "loom.decode.builder");
    assert_eq!(DecodeDialectOp::ForRows.as_str(), "loom.decode.for_rows");
    assert_eq!(DecodeDialectOp::RawCopy.as_str(), "loom.decode.raw_copy");
    assert_eq!(
        DecodeDialectOp::BitUnpack.as_str(),
        "loom.decode.bit_unpack"
    );
    assert_eq!(DecodeDialectOp::ForDelta.as_str(), "loom.decode.for_delta");
    assert_eq!(
        DecodeDialectOp::ValidityAllValid.as_str(),
        "loom.decode.validity_all_valid"
    );
    assert_eq!(
        DecodeDialectOp::ValidityCopy.as_str(),
        "loom.decode.validity_copy"
    );
    assert_eq!(DecodeDialectOp::Finish.as_str(), "loom.decode.finish");
}

#[test]
fn arrow_type_names_are_stable() {
    assert_eq!(arrow_type_name(&DataType::Int32), "int32");
    assert_eq!(arrow_type_name(&DataType::Int64), "int64");
    assert_eq!(arrow_type_name(&DataType::Float32), "float32");
    assert_eq!(arrow_type_name(&DataType::Float64), "float64");
    assert_eq!(arrow_type_name(&DataType::Utf8), "unsupported");
}

#[test]
fn emits_deterministic_single_column_decode_dialect_text() {
    let report = accepted_report("LMP1 layout", ConstraintDischargeStatus::Discharged);
    let artifact = lower_to_decode_dialect_text(&report)
        .expect("discharged single-column report should emit dialect text");

    assert_eq!(artifact.backend, "loom-decode-dialect");
    assert_eq!(artifact.module_name, "loom_artifact");
    assert_eq!(artifact.row_count, 4);
    assert_eq!(artifact.column_count, 1);
    assert!(artifact
        .facts_linkage
        .contains("constraint_status=discharged"));

    let expected = "module {\n  loom.decode.module @loom_artifact {artifact_kind = \"LMC1\", payload_kind = \"LMP1 layout\", rows = 4, constraint_status = \"discharged\", backend = \"loom-decode-dialect\", columns = 1}\n  loom.decode.kernel @decode {rows = 4} {\n    loom.decode.column @out0 {arrow_type = \"int32\", nullable = false}\n    loom.decode.builder @out0 {arrow_type = \"int32\", validity = \"all_valid\"}\n    loom.decode.for_rows %row = 0 to 4 {\n      loom.decode.raw_copy @out0[%row]\n      loom.decode.validity_all_valid @out0[%row]\n    }\n    loom.decode.finish @out0\n  }\n}\n";
    assert_eq!(artifact.text, expected);
}

#[test]
fn emits_deterministic_multi_column_decode_dialect_text() {
    let report = accepted_table_report();
    let artifact = lower_to_decode_dialect_text(&report)
        .expect("not-required table report should emit dialect text");

    assert_eq!(artifact.row_count, 4);
    assert_eq!(artifact.column_count, 2);
    assert!(artifact.text.contains("payload_kind = \"LMT1 table\""));
    assert!(artifact.text.contains("loom.decode.column @id"));
    assert!(artifact.text.contains("loom.decode.column @score"));
    assert!(artifact.text.contains("arrow_type = \"int64\""));
    assert!(artifact.text.contains("arrow_type = \"float64\""));
}

#[test]
fn dialect_emission_requires_production_support() {
    let report = accepted_report("LMP1 layout", ConstraintDischargeStatus::CollectedOnly);
    let err = lower_to_decode_dialect_text(&report)
        .expect_err("collected-only report should reject before dialect text");

    assert!(!err.diagnostics().is_empty());
    assert!(!check_production_lowering_support(&report).is_supported());
}

#[test]
fn direct_emit_uses_production_facts() {
    let report = accepted_report("LMP1 layout", ConstraintDischargeStatus::Discharged);
    let support = check_production_lowering_support(&report);
    let text = emit_decode_dialect_text(support.facts().expect("facts"));

    assert_eq!(text.column_count, 1);
    assert!(text.text.contains("loom.decode.module"));
    assert!(text.text.contains("loom.decode.raw_copy"));
}
