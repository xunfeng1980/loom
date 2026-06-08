use arrow_schema::DataType;
use loom_core::arrow_buffer_lowering::{
    data_type_name, describe_column, lower_arrow_buffers_to_standard_mlir,
    lower_arrow_raw_copy_to_standard_mlir, plan_arrow_buffers_from_decode_dialect,
    reference_zeroed_value_bytes, ArrowValidityPlan, PrimitiveArrowType,
};
use loom_core::artifact_verifier::{
    ArtifactVerificationFacts, ArtifactVerificationReport, ConstraintDischargeStatus,
};
use loom_core::l2_core::{OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_core::production_native_lowering::{
    check_production_lowering_support, ProductionLoweringDiagnosticCode,
};

fn column(builder_id: &str, arrow_type: DataType, nullable: bool) -> OutputSchemaFact {
    OutputSchemaFact {
        builder_id: builder_id.to_string(),
        arrow_type,
        nullable,
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

fn accepted_report(output_schema: Vec<OutputSchemaFact>) -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("LMT1 table".to_string());
    facts.row_count_bound = Some(4);
    facts.constraint_status = ConstraintDischargeStatus::Discharged;
    facts.l2_core = Some(l2_facts(output_schema));
    ArtifactVerificationReport::accepted(facts)
}

#[test]
fn primitive_type_metadata_is_stable() {
    assert_eq!(PrimitiveArrowType::Int32.as_str(), "int32");
    assert_eq!(PrimitiveArrowType::Int32.byte_width(), 4);
    assert_eq!(PrimitiveArrowType::Int64.byte_width(), 8);
    assert_eq!(PrimitiveArrowType::Float32.mlir_type(), "f32");
    assert_eq!(PrimitiveArrowType::Float64.mlir_type(), "f64");
    assert_eq!(data_type_name(&DataType::Int32), "int32");
}

#[test]
fn plans_primitive_arrow_buffers_for_supported_table() {
    let report = accepted_report(vec![
        column("id", DataType::Int64, false),
        column("score", DataType::Float64, false),
    ]);
    let support = check_production_lowering_support(&report);
    let buffers = plan_arrow_buffers_from_decode_dialect(support.facts().expect("facts"));

    assert!(
        buffers.is_supported(),
        "unexpected diagnostics: {:?}",
        buffers.diagnostics()
    );
    let table = buffers.table().expect("table plan");
    assert_eq!(table.row_count, 4);
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.columns[0].builder_id, "id");
    assert_eq!(
        table.columns[0].primitive.primitive_type,
        PrimitiveArrowType::Int64
    );
    assert_eq!(table.columns[0].primitive.value_buffer_bytes, 32);
    assert_eq!(table.columns[0].validity, ArrowValidityPlan::AllValid);
    assert_eq!(table.columns[0].null_count, Some(0));
    assert_eq!(
        describe_column(&table.columns[1]),
        "score:float64:32bytes:all-valid"
    );
}

#[test]
fn emits_standard_mlir_for_primitive_builder_plan() {
    let report = accepted_report(vec![
        column("id", DataType::Int64, false),
        column("score", DataType::Float64, false),
    ]);
    let support = check_production_lowering_support(&report);
    let buffers = plan_arrow_buffers_from_decode_dialect(support.facts().expect("facts"));
    let mlir = lower_arrow_buffers_to_standard_mlir(buffers.table().expect("table"))
        .expect("supported table should lower to standard MLIR text");

    let expected = "module {\n  func.func @loom_decode_build_buffers(%id: memref<?xi64>, %score: memref<?xf64>, %rows: index) {\n    %c0 = arith.constant 0 : index\n    %c1 = arith.constant 1 : index\n    %z0 = arith.constant 0 : i64\n    %z1 = arith.constant 0.000000e+00 : f64\n    scf.for %row = %c0 to %rows step %c1 {\n      memref.store %z0, %id[%row] : memref<?xi64>\n      memref.store %z1, %score[%row] : memref<?xf64>\n    }\n    return\n  }\n}\n";
    assert_eq!(mlir, expected);
    for marker in ["func.func", "arith.constant", "scf.for", "memref.store"] {
        assert!(mlir.contains(marker), "missing {marker}");
    }
}

#[test]
fn emits_raw_copy_mlir_with_input_and_output_memrefs() {
    let report = accepted_report(vec![
        column("id", DataType::Int32, false),
        column("score", DataType::Float64, false),
    ]);
    let support = check_production_lowering_support(&report);
    let buffers = plan_arrow_buffers_from_decode_dialect(support.facts().expect("facts"));
    let mlir = lower_arrow_raw_copy_to_standard_mlir(buffers.table().expect("table"))
        .expect("supported table should lower to raw-copy MLIR text");

    for marker in [
        "llvm.emit_c_interface",
        "%id_in: memref<?xi32>",
        "%id_out: memref<?xi32>",
        "%score_in: memref<?xf64>",
        "%score_out: memref<?xf64>",
        "memref.load %id_in",
        "memref.store %value_id, %id_out",
        "memref.load %score_in",
        "memref.store %value_score, %score_out",
    ] {
        assert!(mlir.contains(marker), "missing {marker}");
    }
}

#[test]
fn reference_zeroed_bytes_match_value_buffer_length() {
    let report = accepted_report(vec![column("out0", DataType::Float32, false)]);
    let support = check_production_lowering_support(&report);
    let buffers = plan_arrow_buffers_from_decode_dialect(support.facts().expect("facts"));
    let column = &buffers.table().expect("table").columns[0];

    assert_eq!(column.primitive.value_buffer_bytes, 16);
    assert_eq!(reference_zeroed_value_bytes(column), vec![0u8; 16]);
}

#[test]
fn production_gate_rejects_unsupported_buffer_shapes_before_planning() {
    let report = accepted_report(vec![column("out0", DataType::Utf8, false)]);
    let support = check_production_lowering_support(&report);

    assert!(!support.is_supported());
    assert_eq!(
        support.first_error().expect("diagnostic").code,
        ProductionLoweringDiagnosticCode::UnsupportedType
    );
}

#[test]
fn production_gate_rejects_nullable_before_buffer_planning() {
    let report = accepted_report(vec![column("out0", DataType::Int32, true)]);
    let support = check_production_lowering_support(&report);

    assert!(!support.is_supported());
    assert_eq!(
        support.first_error().expect("diagnostic").code,
        ProductionLoweringDiagnosticCode::UnsupportedNullability
    );
}
