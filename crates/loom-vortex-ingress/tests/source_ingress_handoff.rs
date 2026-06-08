use std::sync::LazyLock;

use arrow::array::{Int32Array, Int64Array};
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::{
    decode_layout_payload_maybe_container, decode_table_payload_maybe_container,
};
use loom_core::l1_model::decode_layout_to_array_data;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::table_codec::decode_table_to_array_data;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnosticCode, SourceEmissionDisposition,
    SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition, SourceOracleStrategy,
};
use loom_vortex_ingress::emit_source_ingress_lmc1_from_vortex_buffer;
use vortex_array::arrays::{StructArray, VarBinArray};
use vortex_array::dtype::{DType, FieldNames, Nullability};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::validity::Validity;
use vortex_array::IntoArray;
use vortex_buffer::buffer;
use vortex_buffer::ByteBufferMut;
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

fn supported_table_bytes() -> Vec<u8> {
    let ids = buffer![1i32, 2, 3].into_array();
    let scores = buffer![10i64, 20, 30].into_array();
    let array = StructArray::try_new(
        FieldNames::from(["id", "score"]),
        vec![ids, scores],
        3,
        Validity::NonNullable,
    )
    .expect("struct array");
    vortex_file_bytes(array)
}

fn unsupported_utf8_bytes() -> Vec<u8> {
    vortex_file_bytes(VarBinArray::from_iter(
        [Some("a"), Some("b"), Some("c")],
        DType::Utf8(Nullability::Nullable),
    ))
}

fn unsupported_table_bytes() -> Vec<u8> {
    let ids = buffer![1i32, 2, 3].into_array();
    let names = VarBinArray::from_iter(
        [Some("a"), Some("b"), Some("c")],
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

fn assert_emitted_artifact_is_verifier_accepted(bytes: &[u8]) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
}

fn decode_single_i32_values(bytes: &[u8]) -> Vec<i32> {
    let registry = L2KernelRegistry::default_for_mvp0();
    let desc = decode_layout_payload_maybe_container(bytes).expect("decode LMP1 container");
    let data = decode_layout_to_array_data(&desc, &registry).expect("decode LMP1 rows");
    let array = Int32Array::from(data);
    (0..array.len()).map(|idx| array.value(idx)).collect()
}

fn decode_table_values(bytes: &[u8]) -> (Vec<i32>, Vec<i64>) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let table = decode_table_payload_maybe_container(bytes).expect("decode LMT1 container");
    let arrays = decode_table_to_array_data(&table, &registry).expect("decode LMT1 rows");
    let ids = Int32Array::from(arrays[0].clone());
    let scores = Int64Array::from(arrays[1].clone());
    (
        (0..ids.len()).map(|idx| ids.value(idx)).collect(),
        (0..scores.len()).map(|idx| scores.value(idx)).collect(),
    )
}

#[test]
fn accepted_single_column_handoff_is_verifier_routed_lmp1() {
    let vortex = vortex_file_bytes(buffer![7i32, -1, 42]);
    let accepted =
        emit_source_ingress_lmc1_from_vortex_buffer(&vortex).expect("accepted source handoff");

    assert!(!accepted.bytes.is_empty());
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(accepted.report.emission_kind, SourceEmissionKind::Lmp1);
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::CanonicalRaw
    );
    assert_eq!(
        accepted.report.lowering_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported
    );
    assert!(accepted.report.artifact_verification.required);
    assert!(accepted.report.artifact_verification.accepted);
    assert_eq!(
        accepted.report.artifact_verification.artifact_byte_len,
        Some(accepted.bytes.len())
    );
    assert!(accepted
        .report
        .artifact_verification
        .summary
        .contains("LMC1"));
    assert!(accepted
        .report
        .artifact_verification
        .summary
        .contains("LMP1 layout"));
}

#[test]
fn accepted_table_handoff_is_verifier_routed_lmt1() {
    let vortex = supported_table_bytes();
    let accepted =
        emit_source_ingress_lmc1_from_vortex_buffer(&vortex).expect("accepted source handoff");

    assert!(!accepted.bytes.is_empty());
    assert_emitted_artifact_is_verifier_accepted(&accepted.bytes);
    assert_eq!(accepted.report.status, SourceIngressStatus::Accepted);
    assert_eq!(accepted.report.emission_kind, SourceEmissionKind::Lmt1);
    assert_eq!(
        accepted.report.emission_disposition,
        SourceEmissionDisposition::CanonicalTable
    );
    assert_eq!(
        accepted.report.lowering_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported
    );
    assert!(accepted.report.artifact_verification.required);
    assert!(accepted.report.artifact_verification.accepted);
    assert_eq!(
        accepted.report.artifact_verification.artifact_byte_len,
        Some(accepted.bytes.len())
    );
    assert!(accepted
        .report
        .artifact_verification
        .summary
        .contains("LMC1"));
    assert!(accepted
        .report
        .artifact_verification
        .summary
        .contains("LMT1 table"));
}

#[test]
fn accepted_single_column_records_source_native_oracle_evidence() {
    let vortex = vortex_file_bytes(buffer![7i32, -1, 42]);
    let accepted =
        emit_source_ingress_lmc1_from_vortex_buffer(&vortex).expect("accepted source handoff");

    let oracle = accepted
        .report
        .oracle_evidence
        .as_ref()
        .expect("source oracle evidence");
    assert_eq!(oracle.strategy, SourceOracleStrategy::SourceNativeScan);
    assert!(oracle.accepted);
    assert_eq!(oracle.row_count_checked, Some(3));
    assert!(oracle.nulls_checked);
    assert!(oracle.source_native_scan_used);
    assert!(oracle
        .notes
        .iter()
        .any(|note| note.contains("metadata only")));
    assert_eq!(decode_single_i32_values(&accepted.bytes), vec![7, -1, 42]);
}

#[test]
fn accepted_table_records_source_native_oracle_evidence() {
    let vortex = supported_table_bytes();
    let accepted =
        emit_source_ingress_lmc1_from_vortex_buffer(&vortex).expect("accepted source handoff");

    let oracle = accepted
        .report
        .oracle_evidence
        .as_ref()
        .expect("source oracle evidence");
    assert_eq!(oracle.strategy, SourceOracleStrategy::SourceNativeScan);
    assert!(oracle.accepted);
    assert_eq!(oracle.row_count_checked, Some(3));
    assert!(oracle.nulls_checked);
    assert!(oracle.source_native_scan_used);
    assert!(oracle
        .notes
        .iter()
        .any(|note| note.contains("metadata only")));
    assert_eq!(
        decode_table_values(&accepted.bytes),
        (vec![1, 2, 3], vec![10, 20, 30])
    );
}

#[test]
fn unsupported_valid_utf8_fails_closed_without_artifact_or_checked_oracle() {
    let vortex = unsupported_utf8_bytes();
    let report = emit_source_ingress_lmc1_from_vortex_buffer(&vortex)
        .expect_err("unsupported source report");

    assert_eq!(report.status, SourceIngressStatus::Unsupported);
    assert!(report.facts.is_some());
    assert_eq!(
        report
            .facts
            .as_ref()
            .expect("unsupported facts")
            .root_schema
            .as_ref()
            .expect("root schema")
            .logical_kind,
        "utf8"
    );
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
    assert_eq!(
        report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(report.oracle_evidence.is_none());
}

#[test]
fn unsupported_valid_table_shape_fails_closed_with_diagnostics() {
    let vortex = unsupported_table_bytes();
    let report = emit_source_ingress_lmc1_from_vortex_buffer(&vortex)
        .expect_err("unsupported source report");

    assert_eq!(report.status, SourceIngressStatus::Unsupported);
    assert!(report.facts.is_some());
    assert_eq!(
        report
            .facts
            .as_ref()
            .expect("unsupported facts")
            .root_schema
            .as_ref()
            .expect("root schema")
            .field_names,
        vec!["id", "name"]
    );
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code
        == SourceDiagnosticCode::UnsupportedConversion
        && diagnostic.path == "$.payload"));
    assert!(report.oracle_evidence.is_none());
}

#[test]
fn malformed_source_fails_closed_without_facts_or_oracle() {
    let report = emit_source_ingress_lmc1_from_vortex_buffer(b"not a vortex file")
        .expect_err("malformed source report");

    assert_eq!(report.status, SourceIngressStatus::Rejected);
    assert!(report.facts.is_none());
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
    assert_eq!(
        report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(report.oracle_evidence.is_none());
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code
        == SourceDiagnosticCode::OpenFailed
        && diagnostic.path == "$"));
}
