---
phase: quick
plan: 260607-taf
subsystem: docs
tags: [translation, documentation, design]
requires: [design.md]
provides: [README.md, README-zh.md]
affects: []
tech-stack:
  added: []
  patterns: ["bilingual README pair with top-of-file language-switch link"]
key-files:
  created: [README.md, README-zh.md]
  modified: []
decisions:
  - "design.md left untouched as the origin/source document; README-zh.md becomes the maintained Chinese doc"
  - "README-zh.md carries design.md's Chinese text verbatim, confirmed structurally 1:1 with README.md"
  - "Language-switch link (English | 中文) added at the top of both files"
metrics:
  duration: ~8 min
  completed: 2026-06-07
requirements: [TRANSLATE-01]
---

# Phase quick Plan 260607-taf: Translate design.md (Chinese → English) Summary

Produced a faithful, professional English translation of the Loom design document (`design.md`) as repo-root `README.md`, plus a structurally 1:1 Chinese counterpart `README-zh.md`, with cross-linking language-switch lines.

## What Was Built

- **README.md** — Faithful English translation of `design.md` covering all 15 sections (0–14). Preserves heading levels (H1 title, `##` section headings, bold sub-headers 5.1–5.4), the §2 three-IR-axes table, the §2 ASCII relay-pipeline fenced block (inline Chinese comments translated, identifiers kept), the §9 ABI fenced block (`schema` / `decode_batch` / `statistics` signatures, comment translated to `// optional`), all blockquotes in §6/§7/§13, and the §12 comparison matrix with its ✓/✗/△ marks and translated column headers. Terminology applied per the plan glossary; all proper nouns kept verbatim. A `**English** | [中文](README-zh.md)` line sits at the very top.
- **README-zh.md** — The Chinese version, carrying `design.md`'s Chinese content unchanged, confirmed structurally identical to README.md (same section order, headings, tables, fenced code blocks, matrix marks). A `[English](README.md) | **中文**` line sits at the very top.
- **design.md** — Left byte-identical / untouched (verified via `git status`).

## Tasks Completed

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Write README.md — English translation | 0d86944 | README.md |
| 2 | Write README-zh.md — Chinese mirror | 5f8b8e7 | README-zh.md |

## Verification

Structural parity confirmed across all three files:

| File | Sections (`## N.`) | ✓ | ✗ | △ |
| ---- | :--: | :--: | :--: | :--: |
| README.md | 15 | 15 | 13 | 4 |
| README-zh.md | 15 | 15 | 13 | 4 |
| design.md | 15 | 15 | 13 | 4 |

- Both READMEs exist at repo root; each contains `## 14.` and the cross-link to the other file.
- §12 comparison-matrix row/column count and ✓/✗/△ marks identical across all three files.
- `design.md` untouched (clean `git status`).
- Cross-links resolve: README.md → README-zh.md and README-zh.md → README.md.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None. Both deliverables are complete prose translations with no placeholders.

## Self-Check: PASSED

- FOUND: README.md
- FOUND: README-zh.md
- FOUND commit: 0d86944 (README.md)
- FOUND commit: 5f8b8e7 (README-zh.md)
