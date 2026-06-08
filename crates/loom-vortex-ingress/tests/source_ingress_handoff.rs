use std::sync::LazyLock;

use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition,
};
use loom_vortex_ingress::emit_source_ingress_lmc1_from_vortex_buffer;
use vortex_array::arrays::StructArray;
use vortex_array::dtype::FieldNames;
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

fn assert_emitted_artifact_is_verifier_accepted(bytes: &[u8]) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
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
