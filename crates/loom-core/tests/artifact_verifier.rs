use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Int32Array, StringArray};
use arrow_schema::DataType;
use arrow_schema::{Field, Schema};
use loom_core::arrow_semantic::ArrowSemanticPayload;
use loom_core::arrow_semantic_codec::{
    encode_arrow_semantic_container_payload, encode_arrow_semantic_payload,
};
use loom_core::artifact_verifier::{
    verify_artifact, verify_artifact_with_l2_core, ArtifactLoweringDiagnostic,
    ArtifactLoweringReadiness, ArtifactVerificationDiagnostic, ArtifactVerificationFacts,
    ArtifactVerificationOptions, ArtifactVerificationReport, ArtifactVerificationStage,
    ArtifactVerificationStatus,
};
use loom_core::l2_core::L2DataType;
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload, Feature};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, ScalarValue,
};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};

fn registry() -> L2KernelRegistry {
    L2KernelRegistry::default_for_mvp0()
}

fn raw_i32_desc(row_count: usize) -> LayoutDescription {
    LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: (0..row_count as i32)
                .flat_map(|value| value.to_le_bytes())
                .collect(),
            elem_size: 4,
            count: row_count,
        },
        row_count,
    }
}

fn wrapped_i32_layout(row_count: usize) -> Vec<u8> {
    let payload = encode_layout_payload(&raw_i32_desc(row_count));
    wrap_layout_payload(&payload).expect("valid layout should wrap")
}

fn wrapped_i32_table(row_count: usize) -> Vec<u8> {
    let table = TableDescription {
        row_count,
        columns: vec![TableColumn {
            name: "value".to_string(),
            layout: raw_i32_desc(row_count),
        }],
    };
    let payload = encode_table_payload(&table).expect("valid table should encode");
    wrap_table_payload(&payload).expect("valid table should wrap")
}

fn arrow_semantic_payload() -> ArrowSemanticPayload {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ok", DataType::Boolean, true),
        Field::new("id", DataType::Int32, true),
        Field::new("name", DataType::Utf8, true),
    ]));
    let batch = arrow_array::RecordBatch::try_new(
        schema,
        vec![
            Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(1), None, Some(3)])) as ArrayRef,
            Arc::new(StringArray::from(vec![Some("alpha"), None, Some("beta")])) as ArrayRef,
        ],
    )
    .expect("record batch");
    ArrowSemanticPayload::from_record_batches(&[batch]).expect("arrow semantic payload")
}

fn mutate_required_features(bytes: &mut [u8], required_features: u64) {
    bytes[8..16].copy_from_slice(&required_features.to_le_bytes());
}

fn lmc2_payload_offset(bytes: &[u8]) -> usize {
    usize::try_from(u64::from_le_bytes(
        bytes[32..40].try_into().expect("payload offset bytes"),
    ))
    .expect("payload offset fits")
}

fn assert_lmc2_rejection(report: &ArtifactVerificationReport) {
    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.path.starts_with("$.lmc2")));
    assert!(!report.diagnostics().iter().any(|diagnostic| diagnostic
        .message
        .contains("artifact container does not contain a supported LMP1 or LMT1 payload")));
}

fn find_section_entry(bytes: &[u8], kind: u16) -> usize {
    let section_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let mut pos = 28usize;
    for _ in 0..section_count {
        let entry_kind = u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap());
        if entry_kind == kind {
            return pos;
        }
        pos += 28;
    }
    panic!("section kind {kind} not found in test fixture")
}

fn sample_l2core_program() -> L2CoreProgram {
    L2CoreProgram {
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
                arrow_type: L2DataType::Int32,
                nullable: true,
                max_events: 4,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(4),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::u64(0),
            end: ScalarExpr::u64(4),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: "input0".to_string(),
                    offset: ScalarExpr::Add(
                        Box::new(ScalarExpr::var("i")),
                        Box::new(ScalarExpr::u64(0)),
                    ),
                    width: ScalarExpr::u64(4),
                    bind: "value".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: "out0".to_string(),
                    value: ScalarExpr::var("value"),
                },
            ],
        }],
    }
}

#[test]
fn accepted_report_exposes_facts() {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.container_version = Some(1);
    facts.required_features = vec!["single_column_lmp1".to_string()];
    facts.payload_kind = Some("LMP1 layout".to_string());
    facts.constraints_discharged = false;

    let report = ArtifactVerificationReport::accepted(facts);

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    assert!(report.is_ok());
    let facts = report.facts().expect("accepted reports expose facts");
    assert_eq!(facts.artifact_kind, "LMC1");
    assert_eq!(facts.container_version, Some(1));
    assert!(!facts.constraints_discharged);
}

#[test]
fn rejected_and_unsupported_reports_hide_facts() {
    let diagnostic = ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::Container,
        "container-shape",
        "$.container",
        "malformed container",
    );

    let rejected = ArtifactVerificationReport::rejected(vec![diagnostic.clone()]);
    assert_eq!(rejected.status(), ArtifactVerificationStatus::Rejected);
    assert!(!rejected.is_ok());
    assert!(rejected.facts().is_none());
    assert!(rejected.into_facts().is_none());

    let unsupported = ArtifactVerificationReport::unsupported(vec![diagnostic]);
    assert_eq!(
        unsupported.status(),
        ArtifactVerificationStatus::Unsupported
    );
    assert!(!unsupported.is_ok());
    assert!(unsupported.facts().is_none());
    assert!(unsupported.into_facts().is_none());
}

#[test]
fn diagnostic_preserves_stage_code_path_and_message() {
    let diagnostic = ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::L1Structural,
        "count-mismatch",
        "$.payload.row_count",
        "row count mismatch",
    );

    assert_eq!(diagnostic.stage, ArtifactVerificationStage::L1Structural);
    assert_eq!(diagnostic.stage.as_str(), "l1-structural");
    assert_eq!(diagnostic.code, "count-mismatch");
    assert_eq!(diagnostic.path, "$.payload.row_count");
    assert_eq!(diagnostic.message, "row count mismatch");
}

#[test]
fn enum_display_strings_are_stable() {
    assert_eq!(ArtifactVerificationStage::Container.as_str(), "container");
    assert_eq!(ArtifactVerificationStage::Manifest.as_str(), "manifest");
    assert_eq!(ArtifactVerificationStage::L2Core.as_str(), "l2core");
    assert_eq!(
        ArtifactVerificationStage::LoweringReadiness.as_str(),
        "lowering-readiness"
    );

    assert_eq!(ArtifactVerificationStatus::Accepted.as_str(), "accepted");
    assert_eq!(ArtifactVerificationStatus::Rejected.as_str(), "rejected");
    assert_eq!(
        ArtifactVerificationStatus::Unsupported.as_str(),
        "unsupported"
    );


}

#[test]
fn lowering_readiness_defaults_to_not_ready() {
    let default_readiness = ArtifactLoweringReadiness::default();
    assert!(!default_readiness.ready);
    assert!(default_readiness.backend.is_none());
    assert!(default_readiness.diagnostics.is_empty());

    let readiness = ArtifactLoweringReadiness::with_diagnostic(
        Some("textual-mlir"),
        ArtifactLoweringDiagnostic::new(
            "missing-l2core-facts",
            "$.facts.l2_core",
            "lowering requires L2Core facts",
        ),
    );
    assert!(!readiness.ready);
    assert_eq!(readiness.backend.as_deref(), Some("textual-mlir"));
    assert_eq!(readiness.diagnostics[0].code, "missing-l2core-facts");
}

#[test]
fn verify_artifact_accepts_lmc1_layout() {
    let bytes = wrapped_i32_layout(3);
    let report = verify_artifact(&bytes, &registry(), &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report
        .facts()
        .expect("accepted artifact should expose facts");
    assert_eq!(facts.artifact_kind, "LMC1");
    assert_eq!(facts.container_version, Some(1));
    assert!(facts
        .required_features
        .iter()
        .any(|feature| feature == "single_column_lmp1"));
    assert_eq!(facts.payload_kind.as_deref(), Some("LMP1 layout"));
    assert!(facts.schema_section_present);
    assert!(!facts.lowering_ready.ready);
}

#[test]
fn verify_artifact_accepts_lmc1_table() {
    let bytes = wrapped_i32_table(3);
    let report = verify_artifact(&bytes, &registry(), &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report
        .facts()
        .expect("accepted artifact should expose facts");
    assert!(facts
        .required_features
        .iter()
        .any(|feature| feature == "table_lmt1"));
    assert_eq!(facts.payload_kind.as_deref(), Some("LMT1 table"));
    assert!(facts.schema_section_present);
    assert!(!facts.lowering_ready.ready);
}

#[test]
fn verify_artifact_accepts_lmc2_arrow_semantic_container() {
    let payload = arrow_semantic_payload();
    let direct = encode_arrow_semantic_payload(&payload).expect("encode direct LMA1");
    let wrapped = encode_arrow_semantic_container_payload(&payload).expect("encode wrapped LMC2");

    let direct_report = verify_artifact(&direct, &registry(), &Default::default());
    assert_eq!(direct_report.status(), ArtifactVerificationStatus::Accepted);
    let direct_facts = direct_report.facts().expect("direct LMA1 facts");
    assert_eq!(direct_facts.artifact_kind, "LMA1");
    assert_eq!(
        direct_facts.payload_kind.as_deref(),
        Some("Arrow semantic payload")
    );
    assert_eq!(direct_facts.row_count_bound, Some(3));

    let options = ArtifactVerificationOptions {
        compute_lowering_readiness: true,
        lowering_backend: Some("textual-mlir".to_string()),
        ..Default::default()
    };
    let wrapped_report = verify_artifact(&wrapped, &registry(), &options);
    assert_eq!(
        wrapped_report.status(),
        ArtifactVerificationStatus::Accepted
    );
    let facts = wrapped_report.facts().expect("wrapped LMC2 facts");
    assert_eq!(facts.artifact_kind, "LMC2");
    assert_eq!(facts.container_version, Some(1));
    assert!(facts
        .required_features
        .iter()
        .any(|feature| feature == "arrow_semantic_lma1"));
    assert_eq!(
        facts.payload_kind.as_deref(),
        Some("Arrow semantic payload")
    );
    assert!(facts.schema_section_present);
    assert_eq!(facts.row_count_bound, Some(3));
    assert!(!facts.lowering_ready.ready);
    assert!(facts
        .lowering_ready
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "arrow-semantic-lowering-deferred"));
}

#[test]
fn verify_artifact_rejects_malformed_lmc2_container() {
    let payload = arrow_semantic_payload();
    let valid = encode_arrow_semantic_container_payload(&payload).expect("valid LMC2");

    let truncated = verify_artifact(b"LMC2", &registry(), &Default::default());
    assert_lmc2_rejection(&truncated);
    assert!(truncated
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "arrow-semantic-container"));

    let mut unsupported_version = valid.clone();
    unsupported_version[4..6].copy_from_slice(&2u16.to_le_bytes());
    let report = verify_artifact(&unsupported_version, &registry(), &Default::default());
    assert_lmc2_rejection(&report);
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unsupported version")));

    let mut unknown_required_feature = valid.clone();
    mutate_required_features(&mut unknown_required_feature, 1u64 << 9);
    let report = verify_artifact(&unknown_required_feature, &registry(), &Default::default());
    assert_lmc2_rejection(&report);
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown required feature")));

    let mut missing_payload = valid.clone();
    missing_payload[28..30].copy_from_slice(&2u16.to_le_bytes());
    missing_payload[30..32].copy_from_slice(&0u16.to_le_bytes());
    let report = verify_artifact(&missing_payload, &registry(), &Default::default());
    assert_lmc2_rejection(&report);
    assert!(report.diagnostics().iter().any(|diagnostic| diagnostic
        .message
        .contains("missing arrow semantic payload")));

    let mut malformed_inner = valid.clone();
    let payload_offset = lmc2_payload_offset(&malformed_inner);
    malformed_inner[payload_offset..payload_offset + 4].copy_from_slice(b"NOPE");
    let report = verify_artifact(&malformed_inner, &registry(), &Default::default());
    assert_lmc2_rejection(&report);
    assert!(report
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.message.contains("malformed inner LMA1 payload")));
}

#[test]
fn verify_artifact_rejects_truncated_container_without_facts() {
    let report = verify_artifact(b"LMC1", &registry(), &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    let first = report.first_error().expect("diagnostic");
    assert_eq!(first.stage, ArtifactVerificationStage::Container);
    assert_eq!(first.code, "container-shape");
}

#[test]
fn verify_artifact_rejects_unknown_required_features_without_facts() {
    let mut bytes = wrapped_i32_layout(2);
    mutate_required_features(&mut bytes, Feature::SingleColumnLmp1.mask() | (1u64 << 63));

    let report = verify_artifact(&bytes, &registry(), &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    let first = report.first_error().expect("diagnostic");
    assert_eq!(first.stage, ArtifactVerificationStage::Container);
    assert!(first.message.contains("unknown required feature"));
}

#[test]
fn verify_artifact_rejects_bad_section_shape_without_facts() {
    let mut bytes = wrapped_i32_layout(2);
    let layout_entry = find_section_entry(&bytes, 2);
    bytes[layout_entry + 4..layout_entry + 12].copy_from_slice(&u64::MAX.to_le_bytes());

    let report = verify_artifact(&bytes, &registry(), &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    let first = report.first_error().expect("diagnostic");
    assert_eq!(first.stage, ArtifactVerificationStage::Container);
}

#[test]
fn verify_artifact_maps_structural_rejection_without_facts() {
    let invalid_desc = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: vec![1, 0],
            elem_size: 4,
            count: 1,
        },
        row_count: 1,
    };
    let payload = encode_layout_payload(&invalid_desc);
    let bytes = wrap_layout_payload(&payload).expect("container wrapping should still succeed");

    let report = verify_artifact(&bytes, &registry(), &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    let first = report.first_error().expect("diagnostic");
    assert_eq!(first.stage, ArtifactVerificationStage::L1Structural);
    assert_eq!(first.code, "buffer-too-short");
}

#[test]
fn verify_artifact_with_l2_core_fuses_verified_facts() {
    let bytes = wrapped_i32_layout(4);
    let program = sample_l2core_program();

    let report = verify_artifact_with_l2_core(&bytes, &registry(), &program, &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report
        .facts()
        .expect("accepted artifact should expose facts");
    assert_eq!(facts.row_count_bound, Some(4));
    assert!(facts.l2_core.is_some());
    assert!(!facts.constraint_ids.is_empty());
    assert!(!facts.constraints_discharged);
    assert!(facts
        .constraint_ids
        .iter()
        .any(|id| id.contains("read-in-range")));
    assert!(facts
        .proof_obligation_ids
        .iter()
        .any(|id| id == "VERIFIER-10"));
}

#[test]
fn verify_artifact_with_l2_core_rejects_invalid_program_without_facts() {
    let bytes = wrapped_i32_layout(4);
    let mut program = sample_l2core_program();
    program
        .capabilities
        .retain(|capability| !matches!(capability, Capability::InputSlice(_)));

    let report = verify_artifact_with_l2_core(&bytes, &registry(), &program, &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    let first = report.first_error().expect("diagnostic");
    assert_eq!(first.stage, ArtifactVerificationStage::L2Core);
    assert_eq!(first.code, "missing-input-capability");
}

#[test]
fn verify_artifact_with_l2_core_maps_output_type_mismatch() {
    let bytes = wrapped_i32_layout(4);
    let mut program = sample_l2core_program();
    program.body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: ScalarExpr::Const(ScalarValue::Bool(true)),
    }];

    let report = verify_artifact_with_l2_core(&bytes, &registry(), &program, &Default::default());

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
    let first = report.first_error().expect("diagnostic");
    assert_eq!(first.stage, ArtifactVerificationStage::L2Core);
    assert_eq!(first.code, "output-type-mismatch");
}

#[test]
fn verify_artifact_without_l2_core_is_not_lowering_ready() {
    let bytes = wrapped_i32_layout(4);
    let options = ArtifactVerificationOptions {
        compute_lowering_readiness: true,
        lowering_backend: Some("textual-mlir".to_string()),
        ..Default::default()
    };

    let report = verify_artifact(&bytes, &registry(), &options);

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report
        .facts()
        .expect("accepted artifact should expose facts");
    assert!(!facts.lowering_ready.ready);
    assert_eq!(
        facts.lowering_ready.backend.as_deref(),
        Some("textual-mlir")
    );
    assert_eq!(
        facts.lowering_ready.diagnostics[0].code,
        "missing-l2core-facts"
    );
}

#[test]
fn verify_artifact_with_l2_core_accepted_program_is_lowering_ready() {
    // Phase A–C: production verify stays oracle-free. Lowering readiness is
    // determined by acceptance + supported-shape-has-a-rule, not by
    // constraints_discharged (which is always false until Phase D).
    let bytes = wrapped_i32_layout(4);
    let program = sample_l2core_program();
    let options = ArtifactVerificationOptions {
        compute_lowering_readiness: true,
        lowering_backend: Some("textual-mlir".to_string()),
        ..Default::default()
    };

    let report = verify_artifact_with_l2_core(&bytes, &registry(), &program, &options);

    let facts = report
        .facts()
        .expect("accepted artifact should expose facts");
    assert!(!facts.constraints_discharged);
    assert!(facts.lowering_ready.ready);
    assert_eq!(
        facts.lowering_ready.backend.as_deref(),
        Some("textual-mlir")
    );
    assert!(facts.lowering_ready.diagnostics.is_empty());
}

#[test]
fn verify_artifact_with_l2_core_keeps_unsupported_shape_not_ready() {
    let bytes = wrapped_i32_layout(4);
    let mut program = sample_l2core_program();
    program.optional_features.push("debug.extra".to_string());
    let options = ArtifactVerificationOptions {
        compute_lowering_readiness: true,
        lowering_backend: Some("textual-mlir".to_string()),
        ..Default::default()
    };

    let report = verify_artifact_with_l2_core(&bytes, &registry(), &program, &options);

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report
        .facts()
        .expect("accepted artifact should expose facts");
    assert!(!facts.lowering_ready.ready);
    assert_eq!(
        facts.lowering_ready.backend.as_deref(),
        Some("textual-mlir")
    );
    assert!(facts
        .lowering_ready
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported-feature"));
}
