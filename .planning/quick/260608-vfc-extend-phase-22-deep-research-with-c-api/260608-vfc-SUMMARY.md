---
quick_id: 260608-vfc
status: complete
date: 2026-06-08
subsystem: planning
tags: [phase-22, research, capi, napi, abi]
requires:
  - .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-DEEP-RESEARCH.md
provides:
  - C API / N-API / natural API design appendix updates
---

# Quick Task 260608-vfc Summary: C API / N-API Design Lessons

Extended `22-DEEP-RESEARCH.md` with Node-API/N-API, `node-addon-api`,
`napi-rs`, and CPython Stable ABI / Limited API lessons.

Key recommendation: Loom should separate the stable low-level C ABI from natural
host-language APIs. `loom_runtime.h` should stay conservative and mechanical;
Rust/C++/DuckDB adapters can be ergonomic wrappers, but must reduce to the same
verifier-gated C/runtime contract.

## Verification

- `rg -n "Node-API|N-API|Natural API|Limited API|Stable ABI|napi-rs|loom-runtime-rs" .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-DEEP-RESEARCH.md` - PASSED
- `git diff --check -- .planning/phases/22-host-native-runtime-abi-and-execution-policy/22-DEEP-RESEARCH.md .planning/quick/260608-vfc-extend-phase-22-deep-research-with-c-api/260608-vfc-PLAN.md .planning/quick/260608-vfc-extend-phase-22-deep-research-with-c-api/260608-vfc-SUMMARY.md` - PASSED

## Self-Check

PASSED
