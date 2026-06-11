use arrow_schema::DataType;
use loom_core::arrow_buffer_lowering::plan_arrow_buffers_from_decode_dialect;
use loom_core::artifact_types::{
    ArtifactVerificationFacts, ArtifactVerificationReport,
};
use loom_core::decode_dialect::emit_decode_dialect_text;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_core::{L2DataType, OutputSchemaFact, ResourceBudget, VerifiedArtifactFacts};
use loom_core::production_native_lowering::{
    check_layout_kernel_support, check_production_lowering_support,
    ProductionLoweringDiagnosticCode,
};

fn raw_layout(data_type: DataType, elem_size: u8) -> LayoutDescription {
    LayoutDescription {
        data_type,
        root: LayoutNode::Raw {
            data: vec![0; usize::from(elem_size) * 4],
            elem_size,
            count: 4,
        },
        row_count: 4,
    }
}

fn output(builder_id: &str, data_type: L2DataType) -> OutputSchemaFact {
    OutputSchemaFact {
        builder_id: builder_id.to_string(),
        arrow_type: data_type,
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
        kloom_discharged: true,
    }
}

fn accepted_table() -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.payload_kind = Some("LMT1 table".to_string());
    facts.row_count_bound = Some(4);
    facts.constraints_discharged = false;
    facts.l2_core = Some(l2_facts(vec![
        output("id", L2DataType::Int64),
        output("score", L2DataType::Float64),
    ]));
    ArtifactVerificationReport::accepted(facts)
}

#[test]
fn raw_primitive_kernel_matrix_is_rejected_pending_phase40() {
    for (data_type, elem_size) in [
        (DataType::Int32, 4),
        (DataType::Int64, 8),
        (DataType::Float32, 4),
        (DataType::Float64, 8),
    ] {
        let layout = raw_layout(data_type, elem_size);
        let err = check_layout_kernel_support(&layout)
            .expect_err("raw primitive copy removed from production path");
        assert_eq!(err.code, ProductionLoweringDiagnosticCode::UnsupportedKernel);
        assert!(err.message.contains("Phase 40"));
    }
}

#[test]
fn raw_primitive_shape_mismatch_rejects() {
    let layout = raw_layout(DataType::Int32, 8);
    let err = check_layout_kernel_support(&layout).expect_err("width mismatch should reject");

    assert_eq!(err.code, ProductionLoweringDiagnosticCode::UnsupportedShape);
    assert!(err.message.contains("elem_size"));
}

#[test]
fn bitpack_and_for_are_recognized_but_deferred() {
    let bitpack = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::BitPack {
            values_buf: vec![0; 8],
            bit_width: 3,
            offset: 0,
            count: 4,
            validity: None,
            all_null: false,
        },
        row_count: 4,
    };
    let bitpack_err =
        check_layout_kernel_support(&bitpack).expect_err("bitpack should be explicitly deferred");
    assert_eq!(
        bitpack_err.code,
        ProductionLoweringDiagnosticCode::UnsupportedKernel
    );
    assert!(bitpack_err.message.contains("Phase 21"));

    let for_layout = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::FrameOfReference {
            reference: 10,
            inner: Box::new(LayoutNode::BitPack {
                values_buf: vec![0; 8],
                bit_width: 3,
                offset: 0,
                count: 4,
                validity: None,
                all_null: false,
            }),
        },
        row_count: 4,
    };
    let for_err =
        check_layout_kernel_support(&for_layout).expect_err("FOR should be explicitly deferred");
    assert_eq!(
        for_err.code,
        ProductionLoweringDiagnosticCode::UnsupportedKernel
    );
    assert!(for_err.message.contains("overflow/range facts"));
}

#[test]
fn dictionary_rle_and_kernel_escape_reject_fail_closed() {
    let dictionary = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Dictionary {
            codes: Box::new(LayoutNode::Raw {
                data: vec![0, 0, 0, 0],
                elem_size: 4,
                count: 1,
            }),
            values: Box::new(LayoutNode::Raw {
                data: vec![0, 0, 0, 0],
                elem_size: 4,
                count: 1,
            }),
        },
        row_count: 1,
    };
    assert_eq!(
        check_layout_kernel_support(&dictionary)
            .expect_err("dict should reject")
            .code,
        ProductionLoweringDiagnosticCode::UnsupportedKernel
    );
}

#[test]
fn multi_column_table_lowers_through_dialect_and_buffer_plan() {
    let report = accepted_table();
    let support = check_production_lowering_support(&report);
    assert!(
        support.is_supported(),
        "unexpected diagnostics: {:?}",
        support.diagnostics()
    );
    let facts = support.facts().expect("facts");
    let dialect = emit_decode_dialect_text(facts);
    let buffers = plan_arrow_buffers_from_decode_dialect(facts);

    assert_eq!(dialect.column_count, 2);
    assert!(dialect.text.contains("loom.decode.column @id"));
    assert!(!dialect.text.contains("loom.decode.raw_copy"));
    assert_eq!(buffers.table().expect("table").columns.len(), 2);
}

#[test]
fn accepted_table_is_supported_regardless_of_constraint_status() {
    // Phase A–C: lowering no longer gates on constraints_discharged.
    let report = accepted_table();
    let support = check_production_lowering_support(&report);

    assert!(support.is_supported(), "unexpected diagnostics: {:?}", support.diagnostics());
}
