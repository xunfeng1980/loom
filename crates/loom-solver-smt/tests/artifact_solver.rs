use arrow_schema::DataType;
use loom_core::artifact_verifier::{
    ArtifactVerificationOptions, ArtifactVerificationStatus, ConstraintDischargeStatus,
};
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr,
};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::encode_layout_payload;
use loom_core::solver::SolverBackendKind;
use loom_solver_smt::{
    discover_backend, verify_artifact_with_l2_core_and_bitwuzla, SolverRunOptions,
};

fn registry() -> L2KernelRegistry {
    L2KernelRegistry::default_for_mvp0()
}

fn wrapped_i32_layout(row_count: usize) -> Vec<u8> {
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: LayoutNode::Raw {
            data: (0..row_count as i32)
                .flat_map(|value| value.to_le_bytes())
                .collect(),
            elem_size: 4,
            count: row_count,
        },
        row_count,
    };
    let payload = encode_layout_payload(&desc);
    wrap_layout_payload(&payload).expect("valid layout should wrap")
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
fn artifact_solver_helper_discharges_constraints_when_bitwuzla_is_available() {
    if !discover_backend(SolverBackendKind::Bitwuzla).available {
        return;
    }

    let artifact_options = ArtifactVerificationOptions {
        compute_lowering_readiness: true,
        lowering_backend: Some("textual-mlir".to_string()),
        ..Default::default()
    };
    let solver_options = SolverRunOptions {
        strict: true,
        timeout_ms: 5_000,
        path_override: None,
    };

    let report = verify_artifact_with_l2_core_and_bitwuzla(
        &wrapped_i32_layout(4),
        &registry(),
        &sample_l2core_program(),
        &artifact_options,
        &solver_options,
    );

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    assert!(
        report.diagnostics().is_empty(),
        "{:#?}",
        report.diagnostics()
    );
    let facts = report.facts().expect("accepted facts");
    assert_eq!(
        facts.constraint_status,
        ConstraintDischargeStatus::Discharged
    );
    assert!(facts
        .solver_report
        .as_ref()
        .expect("solver report")
        .is_successful());
    assert!(facts.lowering_ready.ready);
}

#[test]
fn artifact_solver_helper_preserves_structural_rejection() {
    let report = verify_artifact_with_l2_core_and_bitwuzla(
        b"LMC1",
        &registry(),
        &sample_l2core_program(),
        &Default::default(),
        &SolverRunOptions::default(),
    );

    assert_eq!(report.status(), ArtifactVerificationStatus::Rejected);
    assert!(report.facts().is_none());
}
