---
quick_id: 260608-waw
slug: add-external-source-ingress-and-lance-ph
status: executing
created: "2026-06-08T15:15:29.794Z"
---

# Quick Plan: Add External Source Ingress and Lance Phases

## Objective

Update the roadmap so Phase 26 becomes an external source ingress contract,
Phase 27 becomes Lance dataset binding/ingress, and the previous Phase 26-28
items move later in numeric order.

## Scope

- Update `.planning/ROADMAP.md` overview, phase list, phase details, execution
  order, and progress table.
- Update `.planning/STATE.md` high-level phase count and deferred item phase
  labels so planning state stays consistent with the roadmap.
- Do not modify implementation code or existing in-progress script changes.

## Verification

- `rg` checks for the new phase names and shifted old phase names.
- `git diff --check`
