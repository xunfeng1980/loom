---
phase: 30-starrocks-duckdb-dual-query-surface
plan: 04
status: complete
completed_at: "2026-06-09T05:35:00Z"
implementation_commit: bbdd2fc
type: summary
---

# 30-04 Summary: Negative Matrix And Optional Runtime Smoke

## Result

Plan 30-04 is complete.

Added Phase 30 fail-closed coverage for descriptor identity/result drift,
Phase 29 binding drift, sidecar-only/manifest-only/stale evidence paths, forged
oracle evidence, and unsupported query features. The focused gate now runs the
negative matrix, checks descriptor JSON markers, scans public surfaces for
runtime/API creep, and handles optional StarRocks runtime smoke explicitly.

## Key Behavior

- Mutated descriptor table UUID, schema ID, snapshot ID, artifact hash, row
  count, projection, status, and expected-result digest are rejected before
  accepted descriptor evidence can be used.
- Mutated Phase 29 sidecar/evidence paths fail closed with no accepted binding
  bytes and no query-surface trust root.
- Joins, freeform SQL, external table DDL, remote catalog, credentials, nested
  fields, nullable expansion, distributed execution, and predicate pushdown are
  typed unsupported requests.
- `LOOM_STARROCKS_RUNTIME_SMOKE=1` is opt-in only. Missing `STARROCKS_*` inputs
  fail clearly; default skipped runtime smoke is explicitly not accepted
  StarRocks runtime evidence.

## Verification

```bash
cargo fmt --check
cargo test -p loom-dual-query-surface --test query_surface_negative
cargo test -p loom-dual-query-surface --test dependency_boundary
bash -n scripts/dual-query-surface-test.sh
bash scripts/dual-query-surface-test.sh
bash -c 'set +e; LOOM_STARROCKS_RUNTIME_SMOKE=1 bash scripts/dual-query-surface-test.sh >/tmp/loom-starrocks-missing-env.out 2>&1; status=$?; test "$status" -ne 0; rg -q "STARROCKS_" /tmp/loom-starrocks-missing-env.out'
git diff --check
```

All verification commands passed.

## Handoff

Plan 30-05 should wire the focused Phase 30 gate into the main release gate and
write the final report. StarRocks runtime remains optional and non-canonical
unless an operator provides a live cluster and env inputs.
