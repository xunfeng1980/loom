---- MODULE LoomVerifierPipeline ----
EXTENDS Naturals, TLC

\* Phase 13 TLA+ scaffold for lifecycle invariants only.
\* This model is not the L2Core type-soundness proof; Lean/Rocq owns that layer.

CONSTANTS Raw, Parsed, Verified, Rejected, Lowerable, Lowered, Invalidated

VARIABLES
  state,
  verifierAccepted,
  requiredFeaturesAccepted,
  resourceBounded,
  verifiedFactsPresent

States == {Raw, Parsed, Verified, Rejected, Lowerable, Lowered, Invalidated}

Init ==
  /\ state = Raw
  /\ verifierAccepted = FALSE
  /\ requiredFeaturesAccepted = FALSE
  /\ resourceBounded = FALSE
  /\ verifiedFactsPresent = FALSE

Parse ==
  /\ state = Raw
  /\ state' = Parsed
  /\ UNCHANGED <<verifierAccepted, requiredFeaturesAccepted, resourceBounded, verifiedFactsPresent>>

VerifyOk ==
  /\ state = Parsed
  /\ state' = Verified
  /\ verifierAccepted' = TRUE
  /\ requiredFeaturesAccepted' = TRUE
  /\ resourceBounded' = TRUE
  /\ verifiedFactsPresent' = TRUE

Reject ==
  /\ state \in {Parsed, Verified, Lowerable}
  /\ state' = Rejected
  /\ verifierAccepted' = FALSE
  /\ requiredFeaturesAccepted' = FALSE
  /\ resourceBounded' = FALSE
  /\ verifiedFactsPresent' = FALSE

MakeLowerable ==
  /\ state = Verified
  /\ state' = Lowerable
  /\ verifierAccepted
  /\ requiredFeaturesAccepted
  /\ resourceBounded
  /\ verifiedFactsPresent
  /\ UNCHANGED <<verifierAccepted, requiredFeaturesAccepted, resourceBounded, verifiedFactsPresent>>

Lower ==
  /\ state = Lowerable
  /\ state' = Lowered
  /\ verifierAccepted
  /\ requiredFeaturesAccepted
  /\ resourceBounded
  /\ verifiedFactsPresent
  /\ UNCHANGED <<verifierAccepted, requiredFeaturesAccepted, resourceBounded, verifiedFactsPresent>>

Invalidate ==
  /\ state \in {Verified, Lowerable, Lowered, Rejected}
  /\ state' = Invalidated
  /\ verifierAccepted' = FALSE
  /\ requiredFeaturesAccepted' = FALSE
  /\ resourceBounded' = FALSE
  /\ verifiedFactsPresent' = FALSE

Next ==
  \/ Parse
  \/ VerifyOk
  \/ Reject
  \/ MakeLowerable
  \/ Lower
  \/ Invalidate

Spec ==
  Init /\ [][Next]_<<state, verifierAccepted, requiredFeaturesAccepted, resourceBounded, verifiedFactsPresent>>

LoweredImpliesVerified ==
  state = Lowered =>
    /\ verifierAccepted
    /\ requiredFeaturesAccepted
    /\ resourceBounded
    /\ verifiedFactsPresent

====

