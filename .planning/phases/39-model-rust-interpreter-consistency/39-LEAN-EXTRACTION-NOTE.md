# Phase 39 Lean Extraction Note

**Status:** Deferred for Phase 39
**Date:** 2026-06-09

Lean extraction was evaluated as optional additive evidence for the Phase 39
oracle path.

Decision: deferred extraction for this phase.

reason: the current Phase 38 Lean model is embedded in a single proof/checker
file with `#eval` correspondence output and no existing Lake/extraction package
boundary. Introducing extraction now would add toolchain and packaging work that
does not materially improve the Phase 39 per-run differential validation gate.

The Rust transcription remains acceptable for Phase 39 because:

- it is visibly separate from production execution code;
- it is documented as a differential oracle only;
- it emits stable trace rows that can be compared exactly;
- the release gate can fail closed on reference/production divergence.

Extraction can be revisited later if verified checker lineage or compiled-model
oracle evidence becomes a milestone requirement.
