---
quick_id: 260608-wwx
slug: refine-phase-27-lance-archival-readabili
status: executing
created: "2026-06-08T15:41:54.985Z"
---

# Quick Plan: Refine Phase 27 Lance Archival Readability

## Objective

Revise Phase 27 so the Lance integration target is archival readability: a
verifiable, long-lived Loom artifact for Lance datasets that preserves readable
schema, fragment, and column data across Lance reader-version drift.

## Scope

- Update Phase 27 references in `.planning/ROADMAP.md`.
- Update matching Phase 27 state/deferred references in `.planning/STATE.md`.
- Do not change implementation code.

## Verification

- `rg` checks for the new Phase 27 wording.
- `git diff --check`.
