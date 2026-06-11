use loom_ffi::arrow_buffer_lowering::{
    lower_arrow_buffers_to_standard_mlir, plan_arrow_buffers_from_decode_dialect,
};
use loom_ffi::artifact_types::{
    ArtifactVerificationFacts, ArtifactVerificationReport,
};
use loom_ffi::l2_core::{L2DataType, OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_ffi::production_native_lowering::check_production_lowering_support;
use loom_ffi::pipeline::{
    validate_production_standard_mlir, MlirValidationOptions, ProductionMlirArtifact,
};
use loom_ffi::report::MeliorBackendDiagnosticCode;

fn output(builder_id: &str, data_type: L2DataType) -> OutputSchemaFact {
    OutputSchemaFact {
        builder_id: builder_id.to_string(),
        arrow_type: data_type,
        nullable: false,
    }
}

fn accepted_table() -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("LMT1 table".to_string());
    facts.row_count_bound = Some(4);
    facts.constraints_discharged = false;
    facts.l2_core = Some(VerifiedArtifactFacts {
        artifact_version: 1,
        required_features: vec!["test.production".to_string()],
        optional_features: vec![],
        accepted_feature_set: vec!["test.production".to_string()],
        input_ranges: Vec::new(),
        output_schema: vec![
            output("id", L2DataType::Int64),
            output("score", L2DataType::Float64),
        ],
        row_count_bound: Some(4),
        loop_bounds: Vec::new(),
        resource_bounds: ResourceBudget::bounded_rows(4),
        builder_event_types: Vec::new(),
        capability_summary: Vec::new(),
        constraint_ids: vec!["c0".to_string()],
        proof_obligation_ids: vec!["p0".to_string()],
        kloom_discharged: true,
    });
    ArtifactVerificationReport::accepted(facts)
}

fn production_artifact() -> ProductionMlirArtifact {
    let report = accepted_table();
    let support = check_production_lowering_support(&report);
    let buffers = plan_arrow_buffers_from_decode_dialect(support.facts().expect("facts"));
    let table = buffers.table().expect("table");
    let mlir_text = lower_arrow_buffers_to_standard_mlir(table).expect("standard MLIR should emit");
    ProductionMlirArtifact {
        entry_symbol: "loom_decode_build_buffers".to_string(),
        mlir_text,
        row_count: table.row_count,
        column_count: table.columns.len(),
        artifact_summary: "phase=20;backend=standard-mlir;columns=2".to_string(),
    }
}

#[test]
fn production_standard_mlir_validation_is_skip_aware() {
    let artifact = production_artifact();
    let report = validate_production_standard_mlir(
        &artifact,
        MlirValidationOptions {
            require_compatible_toolchain: false,
        },
    );

    assert!(
        report.is_ok(),
        "unexpected diagnostics: {:?}",
        report.diagnostics
    );
    if report
        .toolchain
        .as_ref()
        .map(|facts| facts.compatible)
        .unwrap_or(false)
    {
        assert!(report.supported);
    }
    assert_eq!(
        report.entry_symbol.as_deref(),
        Some("loom_decode_build_buffers")
    );
    assert_eq!(report.row_count, Some(4));
}

#[test]
fn production_standard_mlir_rejects_malformed_shape_before_toolchain() {
    let mut artifact = production_artifact();
    artifact.entry_symbol = "wrong_symbol".to_string();
    let report = validate_production_standard_mlir(
        &artifact,
        MlirValidationOptions {
            require_compatible_toolchain: false,
        },
    );

    assert!(!report.is_ok());
    assert_eq!(
        report.diagnostics[0].code,
        MeliorBackendDiagnosticCode::MlirVerificationFailed
    );
}
