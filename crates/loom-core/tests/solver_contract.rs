use loom_core::solver::{
    SolverBackendInfo, SolverBackendKind, SolverBitWidthPolicy, SolverDischargeReport,
    SolverObligation, SolverObligationKind, SolverObligationResult, SolverObligationStatus,
    SolverQuerySemantics, SolverRawResult, SolverTheory,
};

#[test]
fn backend_declarations_are_stable() {
    assert_eq!(SolverBackendKind::Z3.as_str(), "z3");
    assert_eq!(SolverBackendKind::Cvc5.as_str(), "cvc5");
    assert_eq!(SolverBackendKind::Bitwuzla.as_str(), "bitwuzla");
}

#[test]
fn solver_theory_and_width_policy_are_stable() {
    assert_eq!(SolverTheory::QfBv.as_str(), "QF_BV");
    assert_eq!(SolverTheory::QfLia.as_str(), "QF_LIA");

    assert_eq!(SolverBitWidthPolicy::Offset64.as_str(), "offset64");
    assert_eq!(SolverBitWidthPolicy::Offset64.bits(), 64);
    assert_eq!(SolverBitWidthPolicy::Fixed(17).as_str(), "fixed");
    assert_eq!(SolverBitWidthPolicy::Fixed(17).bits(), 17);
}

#[test]
fn required_qfbv_obligation_uses_bad_state_unsat_semantics() {
    let obligation = SolverObligation::required_qfbv(
        "ob.bounds.0001",
        SolverObligationKind::Bounds,
        "l2core",
        "$.body[0]",
        vec!["read.bounds.0001".to_string()],
    );

    assert_eq!(obligation.theory, SolverTheory::QfBv);
    assert_eq!(obligation.bit_width_policy, SolverBitWidthPolicy::Offset64);
    assert_eq!(
        obligation.query_semantics,
        SolverQuerySemantics::BadStateUnsat
    );
    assert!(obligation.required);
    assert_eq!(obligation.kind.as_str(), "bounds");
}

#[test]
fn bitwuzla_backend_metadata_is_representable() {
    let backend = SolverBackendInfo::bitwuzla(Some("/opt/homebrew/bin/bitwuzla"), true, 2500);

    assert_eq!(backend.kind, SolverBackendKind::Bitwuzla);
    assert_eq!(backend.kind.as_str(), "bitwuzla");
    assert_eq!(backend.path.as_deref(), Some("/opt/homebrew/bin/bitwuzla"));
    assert!(backend.strict);
    assert_eq!(backend.timeout_ms, 2500);
}

#[test]
fn raw_bad_state_results_map_to_fail_closed_statuses() {
    assert_eq!(
        SolverObligationStatus::from_bad_state_result(SolverRawResult::Unsat),
        SolverObligationStatus::Discharged
    );
    assert_eq!(
        SolverObligationStatus::from_bad_state_result(SolverRawResult::Sat),
        SolverObligationStatus::Failed
    );
    assert_eq!(
        SolverObligationStatus::from_bad_state_result(SolverRawResult::Unknown),
        SolverObligationStatus::Unknown
    );
    assert_eq!(
        SolverObligationStatus::from_bad_state_result(SolverRawResult::Timeout),
        SolverObligationStatus::TimedOut
    );
    assert_eq!(
        SolverObligationStatus::from_bad_state_result(SolverRawResult::Error),
        SolverObligationStatus::Error
    );
    assert_eq!(
        SolverObligationStatus::from_bad_state_result(SolverRawResult::Skipped),
        SolverObligationStatus::Skipped
    );
}

#[test]
fn all_required_results_must_discharge_for_success() {
    let backend = SolverBackendInfo::bitwuzla(None::<String>, false, 1000);
    let report = SolverDischargeReport::from_results(vec![
        SolverObligationResult::new("ob.bounds.0001", backend.clone(), SolverRawResult::Unsat),
        SolverObligationResult::new("ob.rows.0002", backend, SolverRawResult::Unsat),
    ]);

    assert!(report.is_successful());
    assert_eq!(report.status, SolverObligationStatus::Discharged);
    assert_eq!(report.required_obligation_count, 2);
    assert_eq!(report.discharged_count, 2);
}

#[test]
fn failed_unknown_or_skipped_results_are_not_successful() {
    for raw in [
        SolverRawResult::Sat,
        SolverRawResult::Unknown,
        SolverRawResult::Timeout,
        SolverRawResult::Error,
        SolverRawResult::Skipped,
    ] {
        let backend = SolverBackendInfo::bitwuzla(None::<String>, false, 1000);
        let report = SolverDischargeReport::from_results(vec![SolverObligationResult::new(
            "ob.bounds.0001",
            backend,
            raw,
        )]);

        assert!(
            !report.is_successful(),
            "{raw:?} must not count as discharged evidence"
        );
    }
}
