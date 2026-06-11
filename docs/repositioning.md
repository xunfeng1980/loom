# Loom Repositioning Plan

> This is the final plan that converged over a dozen rounds of analysis. It rests
> on two structural decisions:
> **(1) separate the decode IR from the container; (2) take AnyBlox as a conceptual
> reference but stay engineering-independent, with no Wasm fallback — falling back
> means falling back to the host's own native reader.**
> This document is the baseline for repositioning the project, to be used for
> reconciling against / rewriting the existing roadmap.

---

## 0. One-sentence positioning

**Loom is a formally-verifiable-safe, natively-accelerable "format-evolution abstraction layer"
for columnar decoding — it lives, in the form of a decode-IR sidecar, parasitically on top of
existing formats such as Parquet / Vortex / Lance, so that a new encoding "is written once as IR
and can then be read by any engine that has integrated Loom," and that decode step can be proven
safe and JIT-compiled natively to a speed that Wasm structurally cannot reach.**

It is not a new storage format and does not compete with Parquet/Vortex/Lance for adoption;
it is an optional enhancement layer that hangs on top of them — engines that understand it take
the verifiable + native fast path, engines that don't keep reading with their own host reader.

---

## 1. Decision One: separate the decode IR from the container format

Physically split the two concerns that used to be entangled:

```
┌─────────────────────────────────────────────────────────┐
│ Packaging layer (optional, swappable, mostly out-of-TCB) │
│   sidecar hangs on Parquet / Vortex / Lance              │
│   (optional) a section carrying verification facts /      │
│   lineage                                                │
│   (dev-time) one canonical reference packaging for tests │
├─────────────────────────────────────────────────────────┤
│ Decode-IR layer (single, stable, in-TCB) ★ Loom proper   │
│   L2Core decode IR                                       │
│     · independent codec + content-hash identity          │
│       (packaging-independent)                            │
│     · verifier (fail-closed)                             │
│     · kloom formal verification / differential           │
│       (object of verification = this IR)                 │
│     · Lean models soundness (small-kernel backing,        │
│       models only the executor)                          │
├─────────────────────────────────────────────────────────┤
│ Execution layer (single track)                           │
│   Loom-native (MLIR/LLVM/JIT) — verifiable-safe + wide    │
│   vectors + 64-bit                                       │
│   Fallback = host's own native reader                    │
│   (not a second IR-execution path)                       │
└─────────────────────────────────────────────────────────┘
```

**Why separate:**
- **The object of verification is single and stable**: kloom / verifier / Lean all lock onto
  the decode IR, independent of packaging shape. However packaging evolves, the formal assets
  do not become invalid.
- **The IR must be single and packaging-independent**: this is the hard constraint of the whole
  system. Once split, it becomes a physical fact — the IR is an independent artifact, and cannot
  grow into multiple variants just because it "lives in different packagings."
- **TCB minimization**: in-TCB = decode IR + verifier. The packaging's section directory /
  feature flags / lineage all live in the out-of-TCB packaging layer.
- **The frozen-ABI object is clear**: freeze the IR's semantics and its input/output contract;
  do **not** freeze the packaging. Packaging can keep evolving as long as the IR it produces is
  unchanged — downstream is unaffected.

**Therefore the new artifact that must land (formerly the Phase 17 deferred item):**
- **An independent L2Core IR codec + identity**: an artifact that can be independently serialized,
  independently hashed, independently verified, and independently distributed. Without it,
  "separation" is only conceptual.

**Fate of the container:**
- The container is **demoted from a "core deliverable" to two things**:
  1. an **optional "Loom verification + lineage section"** hung inside the host packaging
     (out-of-TCB evidence);
  2. a **dev-time canonical reference packaging** for kloom / verifier tests.
- No longer maintained as a user-facing, adoption-seeking independent top-level format
  (that would repeat Vortex's adoption dilemma).

---

## 2. Decision Two: reference AnyBlox, stay engineering-independent, no Wasm fallback

### 2.1 Relationship to AnyBlox: conceptual reference, engineering independence

**What we take (the ideas):**
- Problem definition: the **N×M dilemma / format ossification** — a new encoding cannot be adopted
  because "every engine has to reimplement it"; the fix is to insert an abstraction layer
  (analogous to LLVM IR for compilers).
- Direction call: AnyBlox's paper §2.4 proposes a "statically-verified decoder DSL" as the way
  out, spec'd as "restrict expressiveness → verifier/compiler small enough to audit → yet
  expressive enough to not limit extensibility," and judges it "promising but needs further
  research," then **actively shelves it** and chooses general-purpose Wasm.
- **Loom = de-shelving that shelved future work of AnyBlox** (a formally-verified loom decode IR),
  and narrowing the battlefield to "columnar decode," making that open bet
  (restricted vs. expressive-enough) winnable.

**What we don't take (the engineering):**
- We do not inherit AnyBlox's interface (`decode_batch` contract / idempotence constraints /
  state page, etc.) → Loom IR is designed in **its own optimal shape**, not bent to fit someone
  else's function signature by expanding L2Core.
- We do not free-ride on AnyBlox's already-integrated engines → adoption comes from Loom's own
  scan integration.
- We do not use AnyBlox's runtime — the safety claim is not built on someone else's runtime.

### 2.2 No Wasm fallback (cut)

Every candidate reason for a Wasm fallback, rejected one by one:
- **Browser scenario** → pseudo-need (columnar big-data consumers are all server-side engines);
  if it ever matters, it's the job of a separate loom-web, the core stays clean.
- **Cross-architecture portability** → already solved by LLVM, Wasm adds nothing incremental.
- **Sandbox safety** → contradicts Loom's verifiable safety (it would mean distrusting your own
  verification and relying on a sandbox to backstop).
- **Riding AnyBlox adoption** → already decided not to free-ride; and on the Wasm track Loom has
  no unique value (no native speed, verifiable safety is meaningless to a sandbox), which would
  dilute the moat.

→ **Conclusion: Loom has exactly one execution track — Loom-native (verifiable + wide vectors +
  64-bit).** No contradiction of "want verifiable yet fall back to a sandbox," no burden of
  "two execution implementations of the same IR + equivalence differential."

### 2.3 Fallback = host's own native reader

The fallback semantics change from "execute the IR a weaker way" to "**don't execute the IR, fall
back to the data's original format**":

```
Reading a host file that carries a Loom sidecar:
  ├─ engine has integrated Loom scan ∧ content-hash check passes ∧ encoding is Loom-supported
  │     → Loom-native track: verifiable-safe + native speed ✓ (gets all the unique value)
  └─ otherwise (no Loom / hash mismatch / encoding unsupported)
        → fall back to host's own native reader, read as ordinary Parquet/Vortex/Lance
```

**Advantages:**
- Self-consistent with the sidecar essence: if the enhancement fails, treat it as absent and fall
  back to the host's original way of reading.
- No need for a second execution implementation of the IR → no second execution path, no
  equivalence-checking burden.
- Zero risk to host users: Loom is a pure-upside optional layer; worst case it is ignored.

**Premise (the core discipline that must hold):**
> **Loom must be a "sidecar overlay," not a "format replacement."**
> Even when a host file carries a Loom sidecar, an engine without Loom must still be able to read
> it as ordinary Parquet/Vortex/Lance. What Loom adds is a strippable overlay; it must never
> re-encode the data into a form only Loom understands — otherwise "fall back to host native
> reader" breaks.

---

## 3. Safety model (under the sidecar)

Because Loom no longer owns the data (the data lives in the host format), the safety claim is
precisely:

> **Either: this data is consistent with the bytes Loom verified (content-hash check passes) and
> the decode IR has been verified safe → take the Loom verifiable-native track;
> or: fail-closed, fall back to the host's native reader.**

- The content-hash binds host data at **column-chunk / fragment granularity**; an independent
  rewrite of the host invalidates only the corresponding granule's sidecar, the rest still
  accelerates.
- Safety rests on "**verifiable consistency + graceful degradation**," not on "owning the whole
  file boundary."

Three trust seams (recorded honestly, not hidden):
- Model ↔ real Rust implementation: kloom / differential **verification** (coverage-driven,
  not a universal proof).
- IR ↔ real MLIR/LLVM/CPU toolchain: **permanent TCB** (translation validation, not verified
  compilation).
- We only ever prove **safety + well-formedness**, never **correctness** (correctness is checked
  against an oracle).

---

## 4. Formal-verification layering (load-bearing vs. evidence — honest boundary)

| Layer | Role | Trust root | Load-bearing? |
|---|---|---|---|
| Rust verifier (fail-closed) + language-level restriction (total-function) | Block malicious/malformed input, rule out arbitrary code execution | in-TCB | **Yes (production load-bearing)** |
| kloom (independent K spec-oracle) + differential | Find "native impl deviates from spec" bugs; an independent second implementation covers the N=1 gap | out-of-TCB (CI/offline) | Evidence, not load-bearing |
| Lean models soundness | Universal proof of the design (models only the executor, small-kernel backing) | out-of-TCB | Evidence, not load-bearing |

- **Safety comes from restriction, not from proof**: L2Core is non-Turing / total-function;
  overflow / out-of-bounds / arbitrary code are inexpressible at the language level → no SMT is
  needed to prove "no bad states."
- kloom's value is **statistical independence** (K's and Rust's bugs are uncorrelated); the
  differential corpus can be fed by sampling real Parquet/Vortex/Lance files to widen coverage —
  but it needs a "host-native-reader ground truth" three-way reconciliation to catch common-mode
  errors.
- Lean / kloom **never gate production facts**; production facts are decided only by the in-TCB
  verifier's accept decision.

---

## 5. Value proposition: two orthogonal axes

| Axis | Content | Relative to whom |
|---|---|---|
| **Evolution** (deeper) | The decode IR is the abstraction layer that decouples "encoding innovation from engines": write a new encoding once as IR and every Loom-integrated consumer can read it, no engine reimplementation → N×M becomes N+M | Solves the same proposition as AnyBlox |
| **Capability** (the moat) | ① Formally-verifiable safety (dare to execute a third-party sidecar of unknown provenance); ② true server-side native (wide vectors / 64-bit) that Wasm structurally cannot reach | What AnyBlox's Wasm cannot give |

- The **evolution axis** is the entry ticket (AnyBlox has it too); the **capability axis** is
  Loom's unique reason to exist.
- The trade-off is clear-eyed: Loom trades the **restriction** of total-function for verifiable +
  native; the cost is covering only "columnar encodings expressible as a total function." This is
  the core bet: **in the narrow domain of columnar decode, total-function expressiveness is
  enough.**
- Encodings that can't be expressed → don't force it; backstopped by the host's native reader
  (not Wasm).

---

## 6. Capability ultimately delivered at the usage level

> **Once an engine "integrates Loom scan once," with no further updates it can read all future new
> encodings expressed in L2Core IR; it can safely execute a piece of decode logic it has not
> reviewed; and on server-side big data it can still get native speed.**

- N×M → N+M: M "per-encoding engine changes" are compressed into "the engine integrates scan once
  + each encoding is written once as IR."
- Boundary (honest): only holds for engines that **have integrated Loom scan**. A genuinely old
  engine with no integration falls back to its own host native reader (reading the
  non-Loom-optimized version) — this is not a defect, it's the mathematical boundary of an
  abstraction layer (LLVM also can't save a backend that doesn't accept LLVM IR).

---

## 7. Host priority (value is asymmetric — decide with data, don't pre-bet)

| Host | Loom's incremental value | Priority |
|---|---|---|
| **Parquet** | More advanced encodings + verifiable-safe decode (Parquet's encodings are old, the increment is clearest) | **Do first, do thoroughly** |
| **Vortex** | Verifiable safety + native speed (Vortex encodings are already advanced; Loom adds safety/execution, not compression) | Next; think through "why would a Vortex user want this" |
| **Lance** | Random-access / vector scenarios may not match Loom's sequential decode IR (unless random-access semantics are solved first) | Question mark; data decides |

"Fallback = host native reader" makes mounting on any host **zero-risk** (worst case: ignored), so
we can confidently mount, and decide focus from real usage data, rather than pre-betting on three
symmetric hosts.

---

## 8. Key prerequisites / open items (must be settled before landing)

1. **Independent L2Core IR codec + content-hash identity** — the real work of Decision One; not in
   the conceptual split, but in landing this artifact (formerly Phase 17 deferred).
2. **The sidecar must be overlay, not replacement** — the entire premise for "fall back to host
   native reader" to hold.
3. **Content-hash binding granularity** — column-chunk/fragment level; the check overhead must not
   cancel the acceleration gain.
4. **Three hosts = one IR + three thin adapters** — must never split into three IRs; the adapters
   only handle "how to mount into / extract from the host + how to bind the hash to host data."
   (Existing loom-parquet/lance/vortex-ingress should degrade into these three thin adapters.)
5. **The core IR is designed server-side-optimal** — never add constraints back onto it for any
   edge exit (browser/compat); degradation is the fallback side's responsibility, not a shackle
   on the IR.
6. **Continuous validation of the core bet** — "in the narrow domain of columnar decode,
   total-function is enough" must be continuously validated by real-encoding coverage (the
   fraction of encodings that can / cannot be expressed in L2Core).

---

## 9. Reconciliation notes against existing code/roadmap

- Remove / demote: LMC2 / LMA1 as a top-level format → demote to an optional lineage section +
  reference packaging.
- Promote to core: L2Core decode IR + independent codec + identity.
- Already in place, reusable: kloom (contrib/, independent K spec-oracle), the verifier, the Lean
  soundness model, the TracedOutputBuilder differential infrastructure.
- Already cut and stays cut: Wasm fallback (never in the code; confirm it is not introduced);
  SMT (already removed, consistent with "safety comes from restriction").
- Existing ingress crates → degrade into thin host adapters (mounting + hash binding).

---

## Roadmap mapping (this milestone)

- **Phase 49 — Independent L2Core Decode IR Codec and Content-Hash Identity** (Decision One): make
  the decode IR an artifact that can be independently serialized, hashed, verified, and
  distributed, decoupled from any container. The dependency root for everything below.
- **Phase 50 — Sidecar Overlay Model and Host-Native Reader Fallback** (Decision Two): Loom becomes
  a strippable sidecar overlay; content-hash binds host data at column-chunk/fragment granularity
  to the Phase 49 IR identity; fail-closed routing to the verifiable-native track or the host's own
  native reader. Existing ingress crates degrade into thin host adapters.
