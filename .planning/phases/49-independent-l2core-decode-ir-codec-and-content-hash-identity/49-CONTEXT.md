# Phase 49 Context: Independent L2Core Decode IR Codec and Content-Hash Identity

**Roadmap phase:** 49 - Repositioning (整理稿) 决定一: decode IR 与 container 分离
**Status:** Complete (2026-06-11)
**Mode:** Implementation — backfilled planning artifacts

## Background

The Loom repositioning (docs/repositioning.md) defines two structural decisions:
- **决定一**: Separate the decode IR from the container format
- **决定二**: Sidecar overlay model with host-native reader fallback

Phase 49 is the first slice — the "real work" of Decision One (§8 item 1). Before this phase,
`L2CoreProgram` existed only as an in-memory AST. Containers (`LMC1`/`LMC2`/`LMP1`/`LMT1`/`LMA1`)
bundled schema + payload + feature flags but never the IR program. The verifier produced
ephemeral, in-memory `VerifiedArtifactFacts` — the verified object had no serialized form and
no stable identity. The IR's identity was implicit and container-entangled.

This phase makes it explicit and independent.

## What This Phase Delivers

1. **Independent L2Core IR codec** — deterministic, versioned binary wire format with `L2IR` magic
   + `u16` version, covering the full 22-construct L2Core↔kloom.k↔Lean sync surface, with zero
   dependency on any container codec
2. **Content-hash identity** — `L2CoreProgram::content_hash()` → `l2ir:<hex>` via FNV-1a over
   canonical codec bytes
3. **Fail-closed parse-and-verify** — `verify_l2_core_bytes` rejects malformed/truncated/
   bad-discriminant wire form before producing any facts

## Dependencies

- Phase 48: kloom v4 spec-oracle + the 22-construct L2Core↔kloom.k↔Lean sync checklist
  (`scripts/l2core-sync-checklist.py`) — the codec must cover exactly that construct surface
- Phase 36/41: verified-lineage, whose digest field holds an MD5 placeholder pending a real
  IR identity
- Independent of Phases 44–47 and MVP2 chain

## Non-goals

- No sidecar overlay model (Phase 50)
- No artifact-level signing/attestation/remote fetch (deferred)
- No new L2Core constructs — codec covers exactly the existing 22-construct verified surface
- No container deletion — `LMC1`/`LMC2`/`LMA1` demotion is Phase 50.1
- No Wasm track, no SMT
- No correctness claims — safety + well-formedness + stable identity only
