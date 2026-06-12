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
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
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

// Note: HashVerificationResult is defined in sidecar.rs and imported above
// via `use crate::sidecar::HashVerificationResult`.

// ---------------------------------------------------------------------------
// Decision types
// ---------------------------------------------------------------------------

/// The routing decision: Loom-native track or host-native reader fallback.
///
/// Every code path returns one of these two variants — there is no "maybe"
/// or "partial" state. The decision is exhaustive and honest.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
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
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidecar::ChunkBinding;

    // -- test helpers -------------------------------------------------------

    fn make_binding(
        granule_id: &str,
        content_hash: &str,
        matches: bool,
    ) -> (ChunkBinding, HashVerificationResult) {
        let binding = ChunkBinding {
            granule_id: granule_id.to_string(),
            host_data_range: (0, 1024),
            content_hash: content_hash.to_string(),
            ir_identity: "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        };
        let recomputed_hash = if matches {
            content_hash.to_string()
        } else {
            "blake3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string()
        };
        let hv = HashVerificationResult {
            granule_id: granule_id.to_string(),
            binding: binding.clone(),
            recomputed_hash,
            matches,
        };
        (binding, hv)
    }

    fn make_sidecar() -> SidecarOverlay {
        SidecarOverlay {
            ir_bytes: vec![0x4C, 0x32, 0x49, 0x52, 0x01, 0x00], // L2IR magic + v1
            bindings: vec![],
        }
    }

    fn make_input(
        engine: bool,
        sidecar: Option<SidecarOverlay>,
        hash_results: Vec<HashVerificationResult>,
        encoding_ok: bool,
    ) -> SidecarRoutingInput {
        SidecarRoutingInput {
            engine_integrated: engine,
            sidecar,
            hash_verification: hash_results,
            encoding_supported: encoding_ok,
        }
    }

    // -- positive tests (LoomNative) ---------------------------------------

    #[test]
    fn test_all_gates_pass_loom_native() {
        let (b1, hv1) = make_binding("col_a", "blake3:1111111111111111111111111111111111111111111111111111111111111111", true);
        let sidecar = make_sidecar();
        let input = make_input(true, Some(sidecar.clone()), vec![hv1], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::LoomNative {
                sidecar: s,
                verified_bindings,
            } => {
                assert_eq!(s.ir_bytes, sidecar.ir_bytes);
                assert_eq!(verified_bindings.len(), 1);
                assert_eq!(verified_bindings[0].granule_id, b1.granule_id);
            }
            other => panic!("expected LoomNative, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_bindings_loom_native() {
        let sidecar = make_sidecar();
        let input = make_input(true, Some(sidecar.clone()), vec![], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::LoomNative {
                sidecar: s,
                verified_bindings,
            } => {
                assert_eq!(s.ir_bytes, sidecar.ir_bytes);
                assert!(verified_bindings.is_empty());
            }
            other => panic!("expected LoomNative with empty bindings, got {other:?}"),
        }
    }

    #[test]
    fn test_no_mismatches_multi_binding_loom_native() {
        let (b1, hv1) = make_binding("col_a", "blake3:1111111111111111111111111111111111111111111111111111111111111111", true);
        let (b2, hv2) = make_binding("col_b", "blake3:2222222222222222222222222222222222222222222222222222222222222222", true);
        let (b3, hv3) = make_binding("col_c", "blake3:3333333333333333333333333333333333333333333333333333333333333333", true);
        let sidecar = make_sidecar();
        let input = make_input(true, Some(sidecar.clone()), vec![hv1, hv2, hv3], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::LoomNative {
                verified_bindings, ..
            } => {
                assert_eq!(verified_bindings.len(), 3);
                let ids: Vec<&str> = verified_bindings.iter().map(|b| b.granule_id.as_str()).collect();
                assert_eq!(ids, vec![b1.granule_id.as_str(), b2.granule_id.as_str(), b3.granule_id.as_str()]);
            }
            other => panic!("expected LoomNative with 3 bindings, got {other:?}"),
        }
    }

    // -- negative tests (HostNativeReader) ---------------------------------

    #[test]
    fn test_engine_not_integrated_falls_back() {
        let (_b, hv) = make_binding("col_a", "blake3:1111111111111111111111111111111111111111111111111111111111111111", true);
        let input = make_input(false, Some(make_sidecar()), vec![hv], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::HostNativeReader {
                reason,
                diagnostics,
            } => {
                assert_eq!(reason, HostNativeReaderReason::EngineNotIntegrated);
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].path, "$.engine");
                assert_eq!(diagnostics[0].code, SidecarDiagnosticCode::EngineNotIntegrated);
            }
            other => panic!("expected HostNativeReader(EngineNotIntegrated), got {other:?}"),
        }
    }

    #[test]
    fn test_no_sidecar_falls_back() {
        let (_b, hv) = make_binding("col_a", "blake3:1111111111111111111111111111111111111111111111111111111111111111", true);
        let input = make_input(true, None, vec![hv], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::HostNativeReader {
                reason,
                diagnostics,
            } => {
                assert_eq!(reason, HostNativeReaderReason::NoSidecarPresent);
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].path, "$.sidecar");
                assert_eq!(diagnostics[0].code, SidecarDiagnosticCode::NoSidecarPresent);
            }
            other => panic!("expected HostNativeReader(NoSidecarPresent), got {other:?}"),
        }
    }

    #[test]
    fn test_hash_mismatch_falls_back() {
        let (_b, hv) = make_binding("col_x", "blake3:expected_hash000100000000000000000000000000000000000000000000000000", false);
        let sidecar = make_sidecar();
        let input = make_input(true, Some(sidecar), vec![hv], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::HostNativeReader {
                reason,
                diagnostics,
            } => {
                assert_eq!(reason, HostNativeReaderReason::HashMismatch);
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, SidecarDiagnosticCode::HashMismatch);
                assert!(diagnostics[0].path.contains("col_x"), "path should reference granule_id");
            }
            other => panic!("expected HostNativeReader(HashMismatch), got {other:?}"),
        }
    }

    #[test]
    fn test_multiple_hash_mismatches_all_diagnosed() {
        let (_b1, hv1) = make_binding("col_a", "blake3:1111111111111111111111111111111111111111111111111111111111111111", false);
        let (_b2, hv2) = make_binding("col_b", "blake3:2222222222222222222222222222222222222222222222222222222222222222", true);
        let (_b3, hv3) = make_binding("col_c", "blake3:3333333333333333333333333333333333333333333333333333333333333333", false);
        let sidecar = make_sidecar();
        let input = make_input(true, Some(sidecar), vec![hv1, hv2, hv3], true);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::HostNativeReader {
                reason,
                diagnostics,
            } => {
                assert_eq!(reason, HostNativeReaderReason::HashMismatch);
                // Only mismatched granules get diagnostics
                assert_eq!(diagnostics.len(), 2);
                let paths: Vec<&str> = diagnostics.iter().map(|d| d.path.as_str()).collect();
                assert!(paths.iter().any(|p| p.contains("col_a")));
                assert!(paths.iter().any(|p| p.contains("col_c")));
                assert!(!paths.iter().any(|p| p.contains("col_b")));
            }
            other => panic!("expected HostNativeReader(HashMismatch) with 2 diagnostics, got {other:?}"),
        }
    }

    #[test]
    fn test_encoding_unsupported_falls_back() {
        let (_b, hv) = make_binding("col_a", "blake3:1111111111111111111111111111111111111111111111111111111111111111", true);
        let sidecar = make_sidecar();
        let input = make_input(true, Some(sidecar), vec![hv], false);

        let decision = decide_sidecar_routing(input);
        match decision {
            SidecarRoutingDecision::HostNativeReader {
                reason,
                diagnostics,
            } => {
                assert_eq!(reason, HostNativeReaderReason::EncodingUnsupported);
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, SidecarDiagnosticCode::EncodingUnsupported);
                assert_eq!(diagnostics[0].path, "$.sidecar.ir");
            }
            other => panic!("expected HostNativeReader(EncodingUnsupported), got {other:?}"),
        }
    }
}
