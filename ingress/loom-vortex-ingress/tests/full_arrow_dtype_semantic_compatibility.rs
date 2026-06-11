use std::sync::Arc;
use std::sync::LazyLock;

use arrow_array::RecordBatch;
use arrow_schema::Schema;
use loom_core::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_core::arrow_semantic_codec::{
    decode_arrow_semantic_container_payload, encode_arrow_semantic_container_payload,
};
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceEmissionDisposition, SourceEmissionKind,
    SourceIngressAcceptedArtifact, SourceIngressReport, SourceIngressStatus,
    SourceLoweringDisposition, SourceOracleEvidence, SourceOracleStrategy,
};
use loom_vortex_ingress::{
    reader_facts_from_vortex_buffer, source_facts_from_vortex_reader_facts,
    source_report_from_vortex_ingress_report,
};
use vortex_array::arrays::{StructArray, VarBinArray};
use vortex_array::arrow::ArrowSessionExt;
use vortex_array::dtype::{DType, FieldNames, Nullability};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::validity::Validity;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
use vortex_buffer::buffer;
use vortex_buffer::ByteBuffer;
use vortex_buffer::ByteBufferMut;
use vortex_file::OpenOptionsSessionExt;
use vortex_file::WriteOptionsSessionExt;
use vortex_io::runtime::current::CurrentThreadRuntime;
use vortex_io::runtime::BlockingRuntime;
use vortex_io::session::RuntimeSession;
use vortex_io::session::RuntimeSessionExt;
use vortex_layout::session::LayoutSession;
use vortex_session::VortexSession;

static RUNTIME: LazyLock<CurrentThreadRuntime> = LazyLock::new(CurrentThreadRuntime::new);

fn session() -> VortexSession {
    let session = VortexSession::empty()
        .with::<MemorySession>()
        .with::<ArraySession>()
        .with::<LayoutSession>()
        .with::<ScalarFnSession>()
        .with::<RuntimeSession>()
        .with_handle(RUNTIME.handle());
    vortex_file::register_default_encodings(&session);
    session
}

fn vortex_file_bytes<T: IntoArray>(array: T) -> Vec<u8> {
    let session = session();
    let mut buf = ByteBufferMut::empty();
    RUNTIME
        .block_on(
            session
                .write_options()
                .write(&mut buf, array.into_array().to_array_stream()),
        )
        .expect("write Vortex file");
    buf.as_slice().to_vec()
}

fn struct_vortex_bytes() -> Vec<u8> {
    let ids = buffer![1i32, 2, 3].into_array();
    let names = VarBinArray::from_iter(
        [Some("alpha"), Some("beta"), Some("gamma")],
        DType::Utf8(Nullability::Nullable),
    )
    .into_array();
    let array = StructArray::try_new(
        FieldNames::from(["id", "name"]),
        vec![ids, names],
        3,
        Validity::NonNullable,
    )
    .expect("struct array");
    vortex_file_bytes(array)
}

/// Dev-time oracle: materialize a Vortex buffer into Arrow RecordBatches using the
/// Vortex SDK directly (replaces the removed `vortex_arrow_oracle_batches_from_buffer`).
fn dev_time_vortex_oracle_batches_from_buffer(
    bytes: &[u8],
) -> Result<Vec<RecordBatch>, String> {
    let s = session();
    let file = s
        .open_options()
        .open_buffer(ByteBuffer::copy_from(bytes))
        .map_err(|e| format!("failed to open Vortex buffer: {e}"))?;
    let array = RUNTIME.block_on(async {
        let stream = file
            .scan()
            .map_err(|e| format!("Vortex scan failed: {e}"))?
            .into_array_stream()
            .map_err(|e| format!("Vortex array stream failed: {e}"))?;
        stream
            .read_all()
            .await
            .map_err(|e| format!("Vortex read_all failed: {e}"))
    })?;
    let mut ctx = file.session().create_execution_ctx();
    let field = file
        .session()
        .arrow()
        .to_arrow_field("value", file.dtype())
        .map_err(|e| format!("Arrow field conversion failed: {e}"))?;
    let arrow_array = file
        .session()
        .arrow()
        .execute_arrow(array, Some(&field), &mut ctx)
        .map_err(|e| format!("Arrow execution failed: {e}"))?;
    let batch = RecordBatch::try_new(Arc::new(Schema::new(vec![field])), vec![arrow_array])
        .map_err(|e| format!("RecordBatch build failed: {e}"))?;
    Ok(vec![batch])
}

/// Dev-time packaging helper: replicates the old `emit_source_ingress_lmc2_from_vortex_buffer`
/// using the cfg(test)-gated oracle and the out-of-TCB LMC2 codec.
fn dev_time_emit_lmc2_from_vortex_buffer(
    bytes: &[u8],
) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let reader_facts =
        reader_facts_from_vortex_buffer(bytes).map_err(source_report_from_vortex_ingress_report)?;
    let batches = dev_time_vortex_oracle_batches_from_buffer(bytes).map_err(|msg| {
        let facts = source_facts_from_vortex_reader_facts(&reader_facts);
        SourceIngressReport::unsupported(
            Some(facts),
            loom_source_ingress::SourceDiagnostic::new(
                loom_source_ingress::SourceDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                msg,
            ),
        )
    })?;
    let schema = batches
        .first()
        .map(RecordBatch::schema)
        .ok_or_else(|| {
            let facts = source_facts_from_vortex_reader_facts(&reader_facts);
            SourceIngressReport::unsupported(
                Some(facts),
                loom_source_ingress::SourceDiagnostic::new(
                    loom_source_ingress::SourceDiagnosticCode::UnsupportedConversion,
                    "$.oracle",
                    "oracle produced no batches",
                ),
            )
        })?;
    let semantic_batches = batches
        .iter()
        .map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            let facts = source_facts_from_vortex_reader_facts(&reader_facts);
            SourceIngressReport::unsupported(
                Some(facts),
                loom_source_ingress::SourceDiagnostic::new(
                    loom_source_ingress::SourceDiagnosticCode::UnsupportedConversion,
                    "$.oracle",
                    format!("ArrowSemanticBatch build failed: {err}"),
                ),
            )
        })?;
    let payload = ArrowSemanticPayload::try_new(schema, semantic_batches).map_err(|err| {
        let facts = source_facts_from_vortex_reader_facts(&reader_facts);
        SourceIngressReport::unsupported(
            Some(facts),
            loom_source_ingress::SourceDiagnostic::new(
                loom_source_ingress::SourceDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                format!("ArrowSemanticPayload build failed: {err}"),
            ),
        )
    })?;
    let artifact_bytes = encode_arrow_semantic_container_payload(&payload).map_err(|err| {
        let facts = source_facts_from_vortex_reader_facts(&reader_facts);
        SourceIngressReport::unsupported(
            Some(facts),
            loom_source_ingress::SourceDiagnostic::new(
                loom_source_ingress::SourceDiagnosticCode::UnsupportedConversion,
                "$.oracle",
                format!("LMC2 encoding failed: {err}"),
            ),
        )
    })?;

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&artifact_bytes, &registry, &Default::default());
    if verification.status() != ArtifactVerificationStatus::Accepted {
        let facts = source_facts_from_vortex_reader_facts(&reader_facts);
        return Err(SourceIngressReport::unsupported(
            Some(facts),
            loom_source_ingress::SourceDiagnostic::new(
                loom_source_ingress::SourceDiagnosticCode::UnsupportedConversion,
                "$.artifact",
                format!("verification failed: {}", verification.status().as_str()),
            ),
        ));
    }
    let artifact_facts = verification
        .facts()
        .expect("accepted artifact verification exposes facts");
    let artifact_summary = SourceArtifactVerificationSummary::accepted(
        artifact_bytes.len(),
        format!(
            "{} verifier accepted {}",
            artifact_facts.artifact_kind,
            artifact_facts
                .payload_kind
                .as_deref()
                .unwrap_or("unknown payload")
        ),
    );
    let row_count = batches.iter().map(|b| b.num_rows() as u64).sum();
    let mut oracle = SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    oracle.nulls_checked = true;
    oracle
        .notes
        .push("source-native oracle evidence via dev-time Vortex read".to_string());

    let mut source_facts = source_facts_from_vortex_reader_facts(&reader_facts);
    if let Some(coverage) = source_facts.coverage.as_mut() {
        coverage.support = SourceIngressStatus::Accepted;
        coverage.emission_kind = SourceEmissionKind::ArrowSemantic;
        coverage.emission_disposition = SourceEmissionDisposition::SemanticArrow;
        coverage.lowering_disposition = SourceLoweringDisposition::InterpreterOnly;
        coverage
            .notes
            .push("Vortex source materialized as Arrow for LMC2-wrapped LMA1 semantic emission".to_string());
    }

    let report = SourceIngressReport::accepted(
        source_facts,
        SourceEmissionKind::ArrowSemantic,
        SourceEmissionDisposition::SemanticArrow,
        SourceLoweringDisposition::InterpreterOnly,
        artifact_summary,
        oracle,
    )
    .expect("accepted Vortex semantic facts map to an accepted source report");

    Ok(SourceIngressAcceptedArtifact {
        bytes: artifact_bytes,
        report,
    })
}

fn assert_vortex_lmc2_roundtrip(bytes: &[u8]) {
    let accepted = dev_time_emit_lmc2_from_vortex_buffer(bytes)
        .expect("accepted Vortex semantic handoff");
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(
        accepted.report.emission_kind,
        SourceEmissionKind::ArrowSemantic
    );
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::SemanticArrow
    );
    assert_eq!(
        accepted.report.lowering_disposition,
        SourceLoweringDisposition::InterpreterOnly
    );

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_artifact(&accepted.bytes, &registry, &Default::default());
    assert_eq!(verification.status(), ArtifactVerificationStatus::Accepted);
    let facts = verification.facts().expect("LMC2 verifier facts");
    assert_eq!(facts.artifact_kind, "LMC2");
    assert_eq!(
        facts.payload_kind.as_deref(),
        Some("Arrow semantic payload")
    );

    let source = dev_time_vortex_oracle_batches_from_buffer(bytes).expect("Vortex Arrow oracle");
    let decoded = decode_arrow_semantic_container_payload(&accepted.bytes)
        .expect("decode LMC2")
        .to_record_batches()
        .expect("LMC2 batches");
    assert_eq!(decoded, source);
}

#[test]
fn vortex_root_primitive_utf8_and_struct_emit_lmc2() {
    assert_vortex_lmc2_roundtrip(&vortex_file_bytes(buffer![7i32, -1, 42]));
    assert_vortex_lmc2_roundtrip(&vortex_file_bytes(VarBinArray::from_iter(
        [Some("a"), Some("b"), Some("c")],
        DType::Utf8(Nullability::Nullable),
    )));
    assert_vortex_lmc2_roundtrip(&struct_vortex_bytes());
}
