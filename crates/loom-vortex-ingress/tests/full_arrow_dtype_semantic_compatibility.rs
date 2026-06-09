use std::sync::LazyLock;

use loom_core::arrow_semantic_codec::decode_arrow_semantic_payload;
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_source_ingress::{
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition,
};
use loom_vortex_ingress::{
    emit_source_ingress_lma1_from_vortex_buffer, vortex_arrow_oracle_batches_from_buffer,
};
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

fn assert_vortex_lma1_roundtrip(bytes: &[u8]) {
    let accepted = emit_source_ingress_lma1_from_vortex_buffer(bytes)
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
    let facts = verification.facts().expect("LMA1 verifier facts");
    assert_eq!(facts.artifact_kind, "LMA1");
    assert_eq!(
        facts.payload_kind.as_deref(),
        Some("Arrow semantic payload")
    );

    let source = vortex_arrow_oracle_batches_from_buffer(bytes).expect("Vortex Arrow oracle");
    let decoded = decode_arrow_semantic_payload(&accepted.bytes)
        .expect("decode LMA1")
        .to_record_batches()
        .expect("LMA1 batches");
    assert_eq!(decoded, source);
}

#[test]
fn vortex_root_primitive_utf8_and_struct_emit_lma1() {
    assert_vortex_lma1_roundtrip(&vortex_file_bytes(buffer![7i32, -1, 42]));
    assert_vortex_lma1_roundtrip(&vortex_file_bytes(VarBinArray::from_iter(
        [Some("a"), Some("b"), Some("c")],
        DType::Utf8(Nullability::Nullable),
    )));
    assert_vortex_lma1_roundtrip(&struct_vortex_bytes());
}
