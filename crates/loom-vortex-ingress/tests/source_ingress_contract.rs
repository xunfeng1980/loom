use std::sync::LazyLock;

use loom_source_ingress::{
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition,
};
use loom_vortex_ingress::{
    reader_facts_from_vortex_buffer, source_facts_from_vortex_buffer, VortexReaderEmissionKind,
    VortexReaderSupport,
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

#[test]
fn supported_non_null_primitive_maps_to_source_contract() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);
    let vortex = reader_facts_from_vortex_buffer(&bytes).expect("Vortex reader facts");
    let source = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let coverage = source.coverage.as_ref().expect("source coverage");

    assert_eq!(vortex.support, VortexReaderSupport::Accepted);
    assert_eq!(vortex.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(source.row_count, 3);
    assert_eq!(source.identity.source_kind, "buffer");
    assert_eq!(source.identity.format, "external-source");
    assert_eq!(
        source
            .root_schema
            .as_ref()
            .expect("root schema")
            .logical_kind,
        "primitive"
    );
    assert_eq!(coverage.support, SourceIngressStatus::Accepted);
    assert_eq!(coverage.emission_kind, SourceEmissionKind::Lmp1);
    assert_eq!(
        coverage.emission_disposition,
        SourceEmissionDisposition::CanonicalRaw
    );
    assert_eq!(
        coverage.lowering_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported
    );
}

#[test]
fn supported_primitive_struct_maps_to_table_contract() {
    let bytes = supported_table_bytes();
    let source = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let coverage = source.coverage.as_ref().expect("source coverage");
    let root = source.root_schema.as_ref().expect("root schema");

    assert_eq!(source.row_count, 3);
    assert_eq!(root.logical_kind, "struct");
    assert_eq!(root.field_names, vec!["id", "score"]);
    assert_eq!(coverage.support, SourceIngressStatus::Accepted);
    assert_eq!(coverage.emission_kind, SourceEmissionKind::Lmt1);
    assert_eq!(
        coverage.emission_disposition,
        SourceEmissionDisposition::CanonicalTable
    );
    assert_eq!(
        coverage.lowering_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported
    );
}

#[test]
fn unsupported_utf8_maps_to_fail_closed_source_contract() {
    let rows = [Some("a"), Some("b"), Some("c")];
    let bytes = vortex_file_bytes(VarBinArray::from_iter(
        rows,
        DType::Utf8(Nullability::Nullable),
    ));
    let source = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let coverage = source.coverage.as_ref().expect("source coverage");

    assert_eq!(source.row_count, 3);
    assert_eq!(
        source
            .root_schema
            .as_ref()
            .expect("root schema")
            .logical_kind,
        "utf8"
    );
    assert_eq!(coverage.support, SourceIngressStatus::Unsupported);
    assert_eq!(coverage.emission_kind, SourceEmissionKind::None);
    assert_eq!(
        coverage.emission_disposition,
        SourceEmissionDisposition::None
    );
    assert_eq!(
        coverage.lowering_disposition,
        SourceLoweringDisposition::FailClosedDeferred
    );
}
