//! Fail-closed sidecar routing decision logic — Phase 50.
//!
//! This module implements the 4-gate routing that decides whether a host file
//! should be decoded through the Loom verifiable-native track or fall back to
//! the host's own native reader. The routing is exhaustive: every code path
//! returns either [`SidecarRoutingDecision::LoomNative`] or
//! [`SidecarRoutingDecision::HostNativeReader`] with a specific reason.
//!
//! # Gate order
//!
//! ```text
//! Gate 1: engine_integrated?  → no → HostNativeReader(EngineNotIntegrated)
//! Gate 2: sidecar present?    → no → HostNativeReader(NoSidecarPresent)
//! Gate 3: all hashes match?   → no → HostNativeReader(HashMismatch)
//! Gate 4: encodings supported? → no → HostNativeReader(EncodingUnsupported)
//! All pass → LoomNative { sidecar, verified_bindings }
//! ```
//!
//! This mirrors the existing `runtime_abi::decide_runtime_execution` pattern
//! for consistency. The sidecar routing handles **sidecar-level** decisions
//! (engine integration, sidecar presence, hash match, encoding support).
//! Within-Loom decisions (verifier acceptance, lowering disposition, etc.)
//! remain the responsibility of `runtime_abi`.

use std::fmt;

use crate::sidecar::{ChunkBinding, HashVerificationResult, SidecarOverlay};

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input to the sidecar routing gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarRoutingInput {
    /// Whether the calling engine has integrated Loom scan.
    pub engine_integrated: bool,
    /// The sidecar overlay, if present in the host file.
    pub sidecar: Option<SidecarOverlay>,
    /// Per-granule hash verification results.
    pub hash_verification: Vec<HashVerificationResult>,
    /// Whether the encodings in the sidecar IR are supported by this Loom runtime.
    pub encoding_supported: bool,
}

/// Result of verifying one granule's content-hash binding against host data.
///
/// Re-exported from [`crate::sidecar::HashVerificationResult`] where
/// hash-related types live together with hash computation.
pub use crate::sidecar::HashVerificationResult;

// ---------------------------------------------------------------------------
// Decision types
// ---------------------------------------------------------------------------

/// The routing decision: Loom-native track or host-native reader fallback.
///
/// Every code path returns one of these two variants — there is no "maybe"
/// or "partial" state. The decision is exhaustive and honest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarRoutingDecision {
    /// Take the Loom verifiable-native track.
    LoomNative {
        sidecar: SidecarOverlay,
        verified_bindings: Vec<ChunkBinding>,
    },
    /// Fall back to the host's own native reader.
    HostNativeReader {
        reason: HostNativeReaderReason,
        diagnostics: Vec<SidecarDiagnostic>,
    },
}

/// Why the routing gate fell back to the host native reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostNativeReaderReason {
    /// The calling engine has not integrated Loom scan.
    EngineNotIntegrated,
    /// No Loom sidecar overlay was found in the host file.
    NoSidecarPresent,
    /// At least one content-hash binding did not match the host data.
    HashMismatch,
    /// The L2Core IR in the sidecar contains encodings not supported by this
    /// Loom runtime.
    EncodingUnsupported,
}

impl fmt::Display for HostNativeReaderReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineNotIntegrated => write!(f, "EngineNotIntegrated"),
            Self::NoSidecarPresent => write!(f, "NoSidecarPresent"),
            Self::HashMismatch => write!(f, "HashMismatch"),
            Self::EncodingUnsupported => write!(f, "EncodingUnsupported"),
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic types
// ---------------------------------------------------------------------------

/// Stable diagnostic code — mirrors [`HostNativeReaderReason`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarDiagnosticCode {
    EngineNotIntegrated,
    NoSidecarPresent,
    HashMismatch,
    EncodingUnsupported,
}

impl fmt::Display for SidecarDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineNotIntegrated => write!(f, "EngineNotIntegrated"),
            Self::NoSidecarPresent => write!(f, "NoSidecarPresent"),
            Self::HashMismatch => write!(f, "HashMismatch"),
            Self::EncodingUnsupported => write!(f, "EncodingUnsupported"),
        }
    }
}

/// A stable, typed diagnostic produced by the routing gate.
///
/// Every routing failure logs at least one diagnostic — no silent fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarDiagnostic {
    pub code: SidecarDiagnosticCode,
    /// JSONPath-style location of the issue (e.g., `"$.engine"`,
    /// `"$.sidecar"`, `"$.hash.col_a"`).
    pub path: String,
    pub message: String,
}

impl SidecarDiagnostic {
    pub fn new(code: SidecarDiagnosticCode, path: &str, message: &str) -> Self {
        Self {
            code,
            path: path.to_string(),
            message: message.to_string(),
        }
    }
}

impl fmt::Display for SidecarDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.path, self.message)
    }
}

// ---------------------------------------------------------------------------
// Routing function
// ---------------------------------------------------------------------------

/// Decide whether to route through the Loom verifiable-native track or fall
/// back to the host's own native reader.
///
/// The decision is **exhaustive**: every input combination produces exactly one
/// [`SidecarRoutingDecision`] with a typed reason. There is no partial or
/// "maybe" state. This is the central safety guarantee of the repositioning.
pub fn decide_sidecar_routing(input: SidecarRoutingInput) -> SidecarRoutingDecision {
    // Gate 1: Is the engine Loom-integrated?
    if !input.engine_integrated {
        return SidecarRoutingDecision::HostNativeReader {
            reason: HostNativeReaderReason::EngineNotIntegrated,
            diagnostics: vec![SidecarDiagnostic::new(
                SidecarDiagnosticCode::EngineNotIntegrated,
                "$.engine",
                "calling engine has not integrated Loom scan — fall back to host native reader",
            )],
        };
    }

    // Gate 2: Is a sidecar present?
    let sidecar = match input.sidecar {
        Some(s) => s,
        None => {
            return SidecarRoutingDecision::HostNativeReader {
                reason: HostNativeReaderReason::NoSidecarPresent,
                diagnostics: vec![SidecarDiagnostic::new(
                    SidecarDiagnosticCode::NoSidecarPresent,
                    "$.sidecar",
                    "no Loom sidecar found in host file",
                )],
            };
        }
    };

    // Gate 3: Do all content-hashes match?
    let mut diagnostics = Vec::new();
    let mismatches: Vec<_> = input
        .hash_verification
        .iter()
        .filter(|r| !r.matches)
        .collect();
    if !mismatches.is_empty() {
        for m in &mismatches {
            diagnostics.push(SidecarDiagnostic::new(
                SidecarDiagnosticCode::HashMismatch,
                &format!("$.hash.{}", m.granule_id),
                &format!(
                    "content-hash mismatch for granule {}: expected {}, recomputed {}",
                    m.granule_id, m.binding.content_hash, m.recomputed_hash,
                ),
            ));
        }
        return SidecarRoutingDecision::HostNativeReader {
            reason: HostNativeReaderReason::HashMismatch,
            diagnostics,
        };
    }

    // Gate 4: Are the encodings supported?
    if !input.encoding_supported {
        return SidecarRoutingDecision::HostNativeReader {
            reason: HostNativeReaderReason::EncodingUnsupported,
            diagnostics: vec![SidecarDiagnostic::new(
                SidecarDiagnosticCode::EncodingUnsupported,
                "$.sidecar.ir",
                "L2Core IR contains encodings not supported by this Loom runtime",
            )],
        };
    }

    // All gates passed — take the Loom-native track.
    let verified_bindings: Vec<ChunkBinding> = input
        .hash_verification
        .into_iter()
        .map(|r| r.binding)
        .collect();

    SidecarRoutingDecision::LoomNative {
        sidecar,
        verified_bindings,
    }
}
