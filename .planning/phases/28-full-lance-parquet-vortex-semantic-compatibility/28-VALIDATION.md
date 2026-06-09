---
phase: 30
slug: full-vortex-semantic-compatibility
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-09
---

# Phase 28 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` plus focused bash release gates |
| **Config file** | `Cargo.toml`, crate-local `Cargo.toml`, shell scripts in `scripts/` |
| **Quick run command** | `cargo test -p loom-vortex-ingress semantic_compatibility` |
| **Full suite command** | `bash scripts/vortex-semantic-compatibility-test.sh` |
| **Estimated runtime** | ~28-180 seconds after warm build |

---

## Sampling Rate

- **After every task commit:** Run the narrow cargo test named by the task.
- **After every plan wave:** Run `bash scripts/vortex-semantic-compatibility-test.sh` once it exists; before then, run the relevant `cargo test -p loom-vortex-ingress --test ...` command.
- **Before `$gsd-verify-work`:** `bash scripts/vortex-semantic-compatibility-test.sh` and `bash scripts/mvp0-verify.sh` must be green or the phase cannot close.
- **Max feedback latency:** 180 seconds for focused gate after warm build.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 28-01-01 | 01 | 1 | PHASE-28 | T-28-01 | Matrix rows cannot overclaim accepted/native support | unit | `cargo test -p loom-vortex-ingress --test semantic_compatibility_matrix` | ✅ W0 | ⬜ pending |
| 28-02-01 | 02 | 2 | PHASE-28 | T-28-02 | Drift checks reject missing oracle/verifier/native evidence | unit/script | `bash scripts/vortex-semantic-compatibility-test.sh` | ✅ W0 | ⬜ pending |
| 28-03-01 | 03 | 3 | PHASE-28 | T-28-03 | Nullable semantics preserve values and null positions or remain unsupported | unit | `cargo test -p loom-vortex-ingress --test nullable_semantic_compatibility` | ✅ W0 | ⬜ pending |
| 28-04-01 | 04 | 4 | PHASE-28 | T-28-04 | Structured encoding claims are separated from canonical raw evidence | unit | `cargo test -p loom-vortex-ingress --test structured_encoding_semantics` | ✅ W0 | ⬜ pending |
| 28-05-01 | 05 | 5 | PHASE-28 | T-28-05 | Release gate and report preserve no-overclaim invariants | script | `bash scripts/vortex-semantic-compatibility-test.sh` | ✅ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] Existing Rust test infrastructure covers crate tests.
- [x] Existing shell gate pattern covers focused release scripts.
- [x] Existing `mvp0-verify.sh` release gate can accept Phase 28 wiring.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | PHASE-28 | All phase behaviors should be automated | N/A |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 180s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-09
