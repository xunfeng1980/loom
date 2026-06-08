use arrow_schema::DataType;
use loom_core::artifact_verifier::{
    apply_solver_discharge, verify_artifact, verify_artifact_with_l2_core,
    ArtifactLoweringDiagnostic, ArtifactLoweringReadiness, ArtifactVerificationDiagnostic,
    ArtifactVerificationFacts, ArtifactVerificationOptions, ArtifactVerificationReport,
    ArtifactVerificationStage, ArtifactVerificationStatus, ConstraintDischargeStatus,
};
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr,
};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::encode_layout_payload;
use loom_core::solver::{
    SolverBackendInfo, SolverDischargeReport, SolverObligationResult, SolverRawResult,
};

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

fn accepted_report_with_constraints(ids: &[&str]) -> ArtifactVerificationReport {
    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.constraint_ids = ids.iter().map(|id| (*id).to_string()).collect();
    facts.constraint_status = ConstraintDischargeStatus::CollectedOnly;
    facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
        Some("textual-mlir"),
        ArtifactLoweringDiagnostic::new(
            "constraints-not-discharged",
            "$.facts.constraint_status",
            "lowering readiness requires discharged solver-backed constraints",
        ),
    );
    ArtifactVerificationReport::accepted(facts)
}

fn solver_report(ids: &[&str], raw: SolverRawResult) -> SolverDischargeReport {
    let backend = SolverBackendInfo::bitwuzla(Some("/opt/homebrew/bin/bitwuzla"), true, 5_000);
    let results = ids
        .iter()
        .map(|id| SolverObligationResult::new(*id, backend.clone(), raw))
        .collect();
    SolverDischargeReport::from_results(results)
}

#[test]
fn discharged_solver_report_updates_constraint_status_and_lowering_readiness() {
    let report = accepted_report_with_constraints(&["c0", "c1"]);
    let applied =
        apply_solver_discharge(report, solver_report(&["c0", "c1"], SolverRawResult::Unsat));

    assert_eq!(applied.status(), ArtifactVerificationStatus::Accepted);
    assert!(applied.diagnostics().is_empty());
    let facts = applied.facts().expect("accepted facts");
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
fn failed_unknown_timeout_error_and_skipped_do_not_discharge() {
    let cases = [
        (
            SolverRawResult::Sat,
            ConstraintDischargeStatus::Failed,
            "solver-discharge-failed",
        ),
        (
            SolverRawResult::Unknown,
            ConstraintDischargeStatus::Unknown,
            "solver-discharge-unknown",
        ),
        (
            SolverRawResult::Timeout,
            ConstraintDischargeStatus::Failed,
            "solver-discharge-timed-out",
        ),
        (
            SolverRawResult::Error,
            ConstraintDischargeStatus::Failed,
            "solver-discharge-error",
        ),
        (
            SolverRawResult::Skipped,
            ConstraintDischargeStatus::Skipped,
            "solver-discharge-skipped",
        ),
    ];

    for (raw, expected_status, expected_code) in cases {
        let applied = apply_solver_discharge(
            accepted_report_with_constraints(&["c0"]),
            solver_report(&["c0"], raw),
        );
        let facts = applied.facts().expect("accepted facts remain visible");
        assert_eq!(facts.constraint_status, expected_status);
        assert!(!facts.lowering_ready.ready);
        assert!(applied
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == expected_code));
    }
}

#[test]
fn obligation_id_mismatch_prevents_discharge() {
    let applied = apply_solver_discharge(
        accepted_report_with_constraints(&["c0", "c1"]),
        solver_report(&["c0", "different"], SolverRawResult::Unsat),
    );

    let facts = applied.facts().expect("accepted facts remain visible");
    assert_eq!(facts.constraint_status, ConstraintDischargeStatus::Failed);
    assert!(applied
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "solver-obligation-mismatch"));
    assert!(!applied.is_ok());
    assert!(!facts.lowering_ready.ready);
}

#[test]
fn rejected_and_unsupported_reports_remain_without_facts() {
    let diagnostic = ArtifactVerificationDiagnostic::new(
        ArtifactVerificationStage::Container,
        "container-shape",
        "$.container",
        "malformed",
    );
    let rejected = ArtifactVerificationReport::rejected(vec![diagnostic.clone()]);
    let unsupported = ArtifactVerificationReport::unsupported(vec![diagnostic]);

    let rejected = apply_solver_discharge(rejected, solver_report(&["c0"], SolverRawResult::Unsat));
    let unsupported =
        apply_solver_discharge(unsupported, solver_report(&["c0"], SolverRawResult::Unsat));

    assert_eq!(rejected.status(), ArtifactVerificationStatus::Rejected);
    assert!(rejected.facts().is_none());
    assert_eq!(
        unsupported.status(),
        ArtifactVerificationStatus::Unsupported
    );
    assert!(unsupported.facts().is_none());
}

#[test]
fn collected_only_constraints_block_artifact_lowering_readiness() {
    let bytes = wrapped_i32_layout(4);
    let program = sample_l2core_program();
    let options = ArtifactVerificationOptions {
        compute_lowering_readiness: true,
        lowering_backend: Some("textual-mlir".to_string()),
        ..Default::default()
    };

    let report = verify_artifact_with_l2_core(&bytes, &registry(), &program, &options);

    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let facts = report.facts().expect("accepted facts");
    assert_eq!(
        facts.constraint_status,
        ConstraintDischargeStatus::CollectedOnly
    );
    assert!(!facts.lowering_ready.ready);
    assert!(facts
        .lowering_ready
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "constraints-not-discharged"));
}

#[test]
fn accepted_structural_artifact_without_l2_core_stays_not_required() {
    let bytes = wrapped_i32_layout(4);
    let report = verify_artifact(&bytes, &registry(), &Default::default());

    let applied = apply_solver_discharge(report, solver_report(&[], SolverRawResult::Unsat));
    let facts = applied.facts().expect("accepted facts");
    assert_eq!(
        facts.constraint_status,
        ConstraintDischargeStatus::NotRequired
    );
    assert!(facts.solver_report.is_some());
}
