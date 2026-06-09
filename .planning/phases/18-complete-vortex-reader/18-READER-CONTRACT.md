# Phase 18 Reader Contract

**Status:** Plan 18-01 contract
**Date:** 2026-06-08
**Scope:** Complete Vortex reader boundary and Loom-owned fact vocabulary

## Scope

Phase 18 defines a complete reader boundary for real Vortex files. The reader
may inspect Vortex container metadata, layout trees, dtypes, segments, stats
presence, support status, and emission intent. It is not arbitrary native decode
support and it is not a production native execution path.

The public boundary is Loom-owned. Public reports and facts must not expose
Vortex Rust types outside `ingress/loom-vortex-ingress`.

## Reader Pipeline

The Phase 18 reader pipeline is:

1. Open real Vortex buffer or local path inside `loom-vortex-ingress`.
2. Extract `VortexReaderFacts` from file metadata, root dtype, layout tree, and
   segment map.
3. Classify the input as `accepted`, `unsupported`, or `rejected`.
4. Emit `LMC1` / `LMT1` bytes only for explicitly supported shapes.
5. Route emitted bytes through Phase 17 `verify_artifact` before treating them
   as accepted Loom artifacts.

The rule is: unsupported files emit no partial `.loom` bytes. Rejected inputs
produce stable diagnostics and no facts-based trust token.

## Facts Model

`VortexReaderFacts` records:

- source kind and Vortex file version,
- row count,
- root dtype fact and root layout encoding,
- layout facts,
- dtype facts,
- segment facts,
- statistics presence,
- approximate footer byte size,
- support classification,
- emission kind.

`VortexReaderLayoutFact` records path, encoding id, dtype summary, row count,
child count, child type/name, child row offset, segment ids, and metadata byte
length.

`VortexReaderSegmentFact` records segment id/index, byte range, byte length,
alignment, ordered-after-previous classification, and overlap classification.

## Support Classification

The reader uses three support states:

- `accepted`: the file is valid and the current Loom reader slice can emit a
  verifier-routed artifact.
- `unsupported`: the file is valid Vortex, but the current Loom reader cannot
  emit a complete Loom artifact for it.
- `rejected`: the input cannot be opened as valid Vortex.

Accepted means "eligible to emit and verify"; it does not mean native lowering
or engine execution is allowed.

## Dependency Boundary

`vortex-file` and `vortex-layout` file-reader APIs are isolated to
`ingress/loom-vortex-ingress`. `loom-core` and `loom-ffi` must remain free of
`vortex` and `fastlanes` dependency-tree matches.

The ingress crate may depend on `loom-core` to wrap/verify supported output
artifacts, but the dependency direction must not invert.

## Vortex Scan Oracle

Vortex scan is oracle evidence only. It may be used by tests to compare emitted
Loom rows against Vortex's own scan result for supported fixtures. It must not
become the implementation path for Loom decode and must not bypass the Loom
artifact verifier.

## Artifact Verification Handoff

Any emitted `LMC1` / `LMT1` artifact must pass Phase 17 `verify_artifact` before
it is treated as an accepted Loom artifact. Reader facts are handoff evidence,
not independent proof of correctness.

Later solver-backed verification may consume the facts, but Phase 18 does not
claim solver discharge.

## Runtime Guards

Static reader facts are not enough for all semantic checks. Value-dependent
properties that require materialized rows remain runtime semantic guards and
oracle/equivalence evidence.

Supported emission paths must fail closed on unsupported dtype, validity,
layout, segment, stats, overflow, or verification conditions.

## Non-Goals

Phase 18 does not implement:

- solver discharge;
- production MLIR/native lowering;
- `melior`, LLVM, JIT, or engine execution;
- object-store Vortex reads;
- remote catalog/table binding;
- compression codec expansion beyond explicitly selected supported slices;
- full formal proof depth.
