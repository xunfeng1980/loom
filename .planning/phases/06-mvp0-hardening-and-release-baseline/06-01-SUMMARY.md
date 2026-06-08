---
phase: 06-mvp0-hardening-and-release-baseline
plan: "01"
subsystem: docs
tags: [docs, planning, baseline]
requirements_completed: [BASE-01, DOC-01, DOC-02]
completed: 2026-06-08
---

# Phase 06-01: Planning-State and README Consistency Summary

Phase 06-01 aligned project documentation with the actual MVP0 state.

## Accomplishments

- Added Phase 6 to the roadmap, requirements traceability, project state, and project overview.
- Updated `.planning/PROJECT.md` so Phase 4/5 capabilities are listed as validated rather than active.
- Updated `.planning/STATE.md` so Phase 6 is the current focus and stale Phase 4/5 API concerns are no longer blockers.
- Added baseline hardening requirements: BASE-01, DOC-01, DOC-02, VERIFY-04, and BUILD-01.
- Added a Vortex / AnyBlox / F3 positioning note and linked it from README and README-zh.
- Added "Current MVP0 Implementation" sections to README and README-zh with current verification commands.
- Fixed ROADMAP mojibake in current roadmap text.

## Verification

- `rg -n 'Phase 05.*active|Phase 3.*Last updated|Pending \|' .planning/PROJECT.md .planning/STATE.md` - PASS, no stale active-state matches.
- `rg -n 'Ã|Â' .planning/ROADMAP.md` - PASS, no matches.
- `rg -n 'POSITIONING|Current MVP0|当前 MVP0|mvp0-verify|duckdb-smoke-test' README.md README-zh.md .planning/ROADMAP.md .planning/REQUIREMENTS.md .planning/PROJECT.md .planning/STATE.md` - PASS.
- `git diff --check` - PASS.

## Notes

Phase 6 still needs the one-command release gate before VERIFY-04 and BUILD-01 can be marked complete.
