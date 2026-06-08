use loom_core::solver::{SmtLibScript, SolverBackendKind, SolverObligationStatus, SolverRawResult};
use loom_solver_smt::{
    declared_backends, discover_all_backends, execute_bitwuzla_script, parse_decisive_result,
    BitwuzlaBackend, SolverCommandBackend, SolverRunOptions,
};

fn script(id: &str, assertion: &str) -> SmtLibScript {
    SmtLibScript::required_qfbv(
        id,
        format!(
            "(set-info :smt-lib-version 2.7)\n(set-logic QF_BV)\n(assert {assertion})\n(check-sat)\n(exit)\n"
        ),
        vec![format!("{id}.obligation")],
    )
}

#[test]
fn declares_all_command_backends() {
    let discoveries = discover_all_backends();
    let kinds = discoveries
        .iter()
        .map(|discovery| discovery.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            SolverBackendKind::Z3,
            SolverBackendKind::Cvc5,
            SolverBackendKind::Bitwuzla
        ]
    );

    let declared = declared_backends(SolverRunOptions::default());
    assert_eq!(declared[0].kind(), SolverBackendKind::Z3);
    assert_eq!(declared[1].kind(), SolverBackendKind::Cvc5);
    assert_eq!(declared[2].kind(), SolverBackendKind::Bitwuzla);
}

#[test]
fn parses_only_decisive_solver_tokens() {
    assert_eq!(
        parse_decisive_result("success\nunsat\n"),
        Some(SolverRawResult::Unsat)
    );
    assert_eq!(parse_decisive_result("sat\n"), Some(SolverRawResult::Sat));
    assert_eq!(
        parse_decisive_result("unknown\n"),
        Some(SolverRawResult::Unknown)
    );
    assert_eq!(parse_decisive_result("success\nmodel follows\n"), None);
}

#[test]
fn missing_bitwuzla_is_explicit_skip_in_normal_mode() {
    let backend = BitwuzlaBackend::new(SolverRunOptions {
        strict: false,
        timeout_ms: 1_000,
        path_override: None,
    });
    if backend.discover().available {
        return;
    }

    let report = backend.execute(&script("missing-normal", "false"));
    assert_eq!(report.status, SolverObligationStatus::Skipped);
    assert_eq!(report.skipped_count, 1);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("bitwuzla binary unavailable")));
}

#[test]
fn missing_bitwuzla_fails_in_strict_mode() {
    let backend = BitwuzlaBackend::new(SolverRunOptions {
        strict: true,
        timeout_ms: 1_000,
        path_override: None,
    });
    if backend.discover().available {
        return;
    }

    let report = backend.execute(&script("missing-strict", "false"));
    assert_eq!(report.status, SolverObligationStatus::Error);
    assert!(!report.is_successful());
}

#[test]
fn bitwuzla_unsat_discharges_when_installed() {
    let backend = BitwuzlaBackend::default();
    if !backend.discover().available {
        return;
    }

    let report = backend.execute(&script("bitwuzla-unsat", "false"));
    assert_eq!(report.status, SolverObligationStatus::Discharged);
    assert!(report.is_successful(), "{report:#?}");
}

#[test]
fn bitwuzla_sat_is_failed_evidence_when_installed() {
    let backend = BitwuzlaBackend::default();
    if !backend.discover().available {
        return;
    }

    let report = execute_bitwuzla_script(
        &script("bitwuzla-sat", "true"),
        &SolverRunOptions::default(),
    );
    assert_eq!(report.status, SolverObligationStatus::Failed);
    assert_eq!(report.failed_count, 1);
    assert!(!report.is_successful());
}
