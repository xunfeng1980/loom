//! Unified artifact-facing verifier report model.
//!
//! Phase 17 starts by defining the report and facts contract before wiring the
//! existing structural and `L2Core` verifiers into one pipeline.

use crate::arrow_semantic_codec::{
    arrow_semantic_container_feature_names, decode_arrow_semantic_container,
    decode_arrow_semantic_payload, is_arrow_semantic_container, is_arrow_semantic_payload,
};
use crate::container_codec::{decode_container, feature_names, ContainerDescription, SectionKind};
use loom_ir_core::full_verifier::verify_l2_core;
use loom_ir_core::l2_core::L2CoreProgram;
use crate::l2_kernel_registry::L2KernelRegistry;
use crate::native_lowering::check_lowering_support;
use crate::verifier::verify_container;

// Types extracted to loom-common (plan 52-01)
pub use loom_common::artifact_types::*;

// Note: the local `verify_artifact` below shadows the loom_common version.
// This local version adds LMC1 container support (path 3) which depends on
// container_codec.

pub fn verify_artifact(
    bytes: &[u8],
    registry: &L2KernelRegistry,
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    if is_arrow_semantic_payload(bytes) {
        return verify_arrow_semantic_artifact(bytes, options);
    }

    if is_arrow_semantic_container(bytes) {
        return verify_arrow_semantic_container_artifact(bytes, options);
    }

    let container = match decode_container(bytes) {
        Ok(container) => container,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::Container,
                    "container-shape",
                    "$.container",
                    err.to_string(),
                ),
            ]);
        }
    };

    let payload_kind = payload_kind(&container);
    let Some(payload_kind) = payload_kind else {
        return ArtifactVerificationReport::unsupported(vec![ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::Manifest,
            "unsupported-payload-kind",
            "$.sections",
            "artifact container does not contain a supported LMP1 or LMT1 payload",
        )]);
    };

    let structural = verify_container(bytes, registry);
    if !structural.is_ok() {
        let diagnostics = structural
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L1Structural,
                    diagnostic.code.as_str(),
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect();
        return ArtifactVerificationReport::rejected(diagnostics);
    }

    let mut facts = ArtifactVerificationFacts::new("LMC1");
    facts.container_version = Some(container.version);
    facts.required_features = feature_names(container.required_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.optional_features = feature_names(container.optional_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.payload_kind = Some(payload_kind.to_string());
    facts.schema_section_present = has_section(&container, SectionKind::Schema);
    facts.kernel_manifest_section_present = has_section(&container, SectionKind::KernelManifest);
    facts.stats_section_present = has_section(&container, SectionKind::Stats);
    if options.compute_lowering_readiness || options.require_l2_core_for_lowering {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(options)),
            ArtifactLoweringDiagnostic::new(
                "missing-l2core-facts",
                "$.facts.l2_core",
                "lowering readiness requires an associated accepted L2Core program",
            ),
        );
    }

    ArtifactVerificationReport::accepted(facts)
}

fn verify_arrow_semantic_container_artifact(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let container = match decode_arrow_semantic_container(bytes) {
        Ok(container) => container,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::Container,
                    "arrow-semantic-container",
                    "$.lmc2",
                    err.to_string(),
                ),
            ]);
        }
    };
    let payload = match decode_arrow_semantic_payload(&container.payload) {
        Ok(payload) => payload,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L1Structural,
                    "arrow-semantic-payload",
                    "$.lmc2.payload",
                    err.to_string(),
                ),
            ]);
        }
    };

    let mut facts = ArtifactVerificationFacts::new("LMC2");
    facts.container_version = Some(container.version);
    facts.required_features = arrow_semantic_container_feature_names(container.required_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.optional_features = arrow_semantic_container_feature_names(container.optional_features)
        .into_iter()
        .map(str::to_string)
        .collect();
    facts.payload_kind = Some("Arrow semantic payload".to_string());
    facts.schema_section_present = true;
    facts.row_count_bound = Some(payload.row_count() as u64);
    if options.compute_lowering_readiness || options.require_l2_core_for_lowering {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(options)),
            ArtifactLoweringDiagnostic::new(
                "arrow-semantic-lowering-deferred",
                "$.facts.lowering_ready",
                "Arrow semantic artifacts are verifier-accepted but not native-lowering ready",
            ),
        );
    }

    facts.tcb_status = Some("out-of-tcb".to_string());
    facts.artifact_role = Some("dev-time-reference-packaging".to_string());

    ArtifactVerificationReport::accepted(facts)
}

fn verify_arrow_semantic_artifact(
    bytes: &[u8],
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let payload = match decode_arrow_semantic_payload(bytes) {
        Ok(payload) => payload,
        Err(err) => {
            return ArtifactVerificationReport::rejected(vec![
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L1Structural,
                    "arrow-semantic-payload",
                    "$.payload",
                    err.to_string(),
                ),
            ]);
        }
    };

    let mut facts = ArtifactVerificationFacts::new("LMA1");
    facts.payload_kind = Some("Arrow semantic payload".to_string());
    facts.schema_section_present = true;
    facts.row_count_bound = Some(payload.row_count() as u64);
    if options.compute_lowering_readiness || options.lowering_backend.is_some() {
        facts.lowering_ready = ArtifactLoweringReadiness::with_diagnostic(
            Some(lowering_backend(options)),
            ArtifactLoweringDiagnostic::new(
                "arrow-semantic-lowering-deferred",
                "$.facts.lowering_ready",
                "Arrow semantic artifacts are verifier-accepted but not native-lowering ready",
            ),
        );
    }

    facts.tcb_status = Some("out-of-tcb".to_string());
    facts.artifact_role = Some("dev-time-reference-packaging".to_string());

    ArtifactVerificationReport::accepted(facts)
}

pub fn verify_artifact_with_l2_core(
    bytes: &[u8],
    registry: &L2KernelRegistry,
    program: &L2CoreProgram,
    options: &ArtifactVerificationOptions,
) -> ArtifactVerificationReport {
    let artifact_report = verify_artifact(bytes, registry, options);
    if artifact_report.status() != ArtifactVerificationStatus::Accepted {
        return artifact_report;
    }
    let mut artifact_facts = artifact_report
        .into_facts()
        .expect("accepted artifact report must contain facts");

    let l2_report = verify_l2_core(program);
    if !l2_report.is_ok() {
        let diagnostics = l2_report
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                ArtifactVerificationDiagnostic::new(
                    ArtifactVerificationStage::L2Core,
                    diagnostic.code.as_str(),
                    diagnostic.path.clone(),
                    diagnostic.message.clone(),
                )
            })
            .collect();
        return ArtifactVerificationReport::rejected(diagnostics);
    }

    let Some(l2_facts) = l2_report.facts().cloned() else {
        return ArtifactVerificationReport::rejected(vec![ArtifactVerificationDiagnostic::new(
            ArtifactVerificationStage::Facts,
            "missing-l2core-facts",
            "$.l2_core.facts",
            "accepted L2Core report did not emit VerifiedArtifactFacts",
        )]);
    };

    artifact_facts.row_count_bound = l2_facts.row_count_bound;
    artifact_facts.constraint_ids = l2_facts.constraint_ids.clone();
    artifact_facts.proof_obligation_ids = l2_facts.proof_obligation_ids.clone();
    // Phase A–C: constraints_discharged stays false (no in-TCB prover).
    // kloom result is recorded as out-of-TCB evidence only.
    artifact_facts.constraints_discharged = false;
    artifact_facts.spec_oracle_trace_validated = l2_facts.kloom_discharged;
    artifact_facts.l2_core = Some(l2_facts);
    if options.compute_lowering_readiness || options.lowering_backend.is_some() {
        artifact_facts.lowering_ready = lowering_readiness_for(program, &l2_report, options);
    }

    ArtifactVerificationReport::accepted(artifact_facts)
}

fn payload_kind(container: &ContainerDescription) -> Option<&'static str> {
    if container
        .sections
        .iter()
        .any(|section| section.kind == SectionKind::LayoutPayload)
    {
        Some("LMP1 layout")
    } else if container
        .sections
        .iter()
        .any(|section| section.kind == SectionKind::TablePayload)
    {
        Some("LMT1 table")
    } else {
        None
    }
}

fn has_section(container: &ContainerDescription, kind: SectionKind) -> bool {
    container
        .sections
        .iter()
        .any(|section| section.kind == kind)
}

fn lowering_readiness_for(
    program: &L2CoreProgram,
    report: &loom_ir_core::full_verifier::FullVerificationReport,
    options: &ArtifactVerificationOptions,
) -> ArtifactLoweringReadiness {
    let backend = lowering_backend(options);
    let support = check_lowering_support(program, report);
    // Phase A–C: lowering readiness = accepted ∧ supported-shape-has-a-rule.
    // No in-TCB constraint discharge is required.
    if support.is_supported() {
        return ArtifactLoweringReadiness::ready(backend);
    }

    let diagnostics = support
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            ArtifactLoweringDiagnostic::new(
                diagnostic.code.as_str(),
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            )
        })
        .collect();
    ArtifactLoweringReadiness {
        ready: false,
        backend: Some(backend),
        diagnostics,
    }
}

fn lowering_backend(options: &ArtifactVerificationOptions) -> String {
    options
        .lowering_backend
        .clone()
        .unwrap_or_else(|| "textual-mlir".to_string())
}
