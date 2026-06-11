use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Int32Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::encode_arrow_semantic_container_payload;
use loom_core::artifact_verifier::{
    ArtifactVerificationDiagnostic, ArtifactVerificationFacts, ArtifactVerificationReport,
    ArtifactVerificationStage,
};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, VerifiedArtifactFacts,
};
use loom_core::native_arrow_semantic::{
    verify_native_arrow_semantic_model, verify_native_arrow_semantic_model_output,
};

use loom_core::verified_lineage::{
    build_verified_lineage_record, VerifiedLineageDiagnosticCode, VerifiedLineageEvidenceLayer,
    VerifiedLineageEvidenceStatus, VerifiedLineageTcbAssumption,
};

fn accepted_l2_report() -> ArtifactVerificationReport {
    let program = L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input0".to_string(),
                offset: 0,
                length: 16,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "out0".to_string(),
                arrow_type: DataType::Int32,
                nullable: true,
                max_events: 4,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(4),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::u64(0),
            end: ScalarExpr::u64(4),
            body: vec![],
        }],
    };

    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("L2Core program".to_string());
    facts.row_count_bound = Some(4);
    facts.constraint_ids = vec!["c-input-range".to_string()];
    facts.proof_obligation_ids = vec!["obl-input-range".to_string()];
    facts.constraints_discharged = false;
    facts.l2_core = Some(VerifiedArtifactFacts::for_program(
        &program,
        facts.constraint_ids.clone(),
        facts.proof_obligation_ids.clone(),
        false,
    ));
    ArtifactVerificationReport::accepted(facts)
}

fn arrow_semantic_bytes() -> Vec<u8> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("value", DataType::Int32, true),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
        ],
    )
    .expect("record batch");
    let payload = ArrowSemanticPayload::from_record_batches(&[batch]).expect("payload");
    encode_arrow_semantic_container_payload(&payload).expect("lmc2")
}

#[test]
fn accepted_artifact_record_names_evidence_layers_and_tcb() {
    let report = accepted_l2_report();

    let record = build_verified_lineage_record(&report, None).expect("lineage record");

    assert_eq!(record.version, 1);
    assert_eq!(record.artifact_kind, "LMC1");
    assert!(record.has_evidence_layer(VerifiedLineageEvidenceLayer::RustVerifierStructuralCheck));
    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::LeanModeledSoundnessTheorem),
        Some(VerifiedLineageEvidenceStatus::CorpusValidated)
    );
    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::LeanModeledSoundnessTheorem),
        Some(VerifiedLineageEvidenceStatus::CorpusValidated)
    );
    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::LeanRustVerifierDifferential),
        Some(VerifiedLineageEvidenceStatus::CorpusValidated)
    );
    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::ModelRustInterpreterDifferential),
        Some(VerifiedLineageEvidenceStatus::CorpusValidated)
    );
    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::NativeModelValidation),
        Some(VerifiedLineageEvidenceStatus::NotRun)
    );
    assert!(record.has_tcb_assumption(VerifiedLineageTcbAssumption::RustCompilerStd));
    assert!(record.has_tcb_assumption(VerifiedLineageTcbAssumption::LlvmMlirToolchain));
    assert!(record.has_tcb_assumption(VerifiedLineageTcbAssumption::RustCAbi));
    assert!(record.has_tcb_assumption(VerifiedLineageTcbAssumption::DuckDbHostProcess));
    assert!(record.has_tcb_assumption(VerifiedLineageTcbAssumption::ArrowCDataInterface));
    assert!(record.contains_non_claim("source-data correctness"));
    assert!(record.contains_non_claim("verified compilation"));
}

#[test]
fn rejected_and_undischarged_reports_do_not_produce_positive_lineage() {
    let rejected = ArtifactVerificationReport::rejected(vec![ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::Container,
        "bad-container",
        "$.container",
        "bad container",
    )]);
    let err = build_verified_lineage_record(&rejected, None).expect_err("rejected");
    assert_eq!(err.code, VerifiedLineageDiagnosticCode::ArtifactNotAccepted);

    // Phase A–C: constraints_discharged is always false, but lineage records
    // are still produced for accepted artifacts (evidence only, no gate).
    let accepted = accepted_l2_report();
    let record = build_verified_lineage_record(&accepted, None).expect("lineage for accepted");
    assert_eq!(record.artifact_kind, "LMC1");
    assert!(
        record.has_evidence_layer(VerifiedLineageEvidenceLayer::RustVerifierStructuralCheck)
    );
}

#[test]
fn native_model_validation_success_becomes_per_run_lineage_evidence() {
    let bytes = arrow_semantic_bytes();
    let validation = verify_native_arrow_semantic_model(&bytes);

    let verification = loom_core::artifact_verifier::verify_artifact(
        &bytes,
        &loom_core::l2_kernel_registry::L2KernelRegistry::default_for_mvp0(),
        &Default::default(),
    );
    let record =
        build_verified_lineage_record(&verification, Some(&validation)).expect("lineage record");

    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::NativeModelValidation),
        Some(VerifiedLineageEvidenceStatus::PerRunValidated)
    );
    assert_eq!(
        record.evidence_status(VerifiedLineageEvidenceLayer::LeanModeledSoundnessTheorem),
        Some(VerifiedLineageEvidenceStatus::NotApplicable)
    );
}

#[test]
fn divergent_native_model_validation_is_not_positive_lineage() {
    let bytes = arrow_semantic_bytes();
    let wrong_schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("value", DataType::Int32, true),
    ]));
    let wrong_batch = RecordBatch::try_new(
        wrong_schema,
        vec![
            Arc::new(BooleanArray::from(vec![Some(false), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
        ],
    )
    .expect("wrong batch");
    let validation = verify_native_arrow_semantic_model_output(&bytes, "LMC2", &wrong_batch);
    assert!(!validation.is_validated());

    let verification = loom_core::artifact_verifier::verify_artifact(
        &bytes,
        &loom_core::l2_kernel_registry::L2KernelRegistry::default_for_mvp0(),
        &Default::default(),
    );
    let err =
        build_verified_lineage_record(&verification, Some(&validation)).expect_err("divergence");

    assert_eq!(
        err.code,
        VerifiedLineageDiagnosticCode::NativeModelValidationFailed
    );
}
