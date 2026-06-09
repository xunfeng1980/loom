use std::sync::LazyLock;

use arrow::array::{Int32Array, Int64Array};
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::decode_table_payload_maybe_container;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::table_codec::decode_table_to_array_data;
use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, reader_facts_from_vortex_buffer, VortexIngressStatus,
    VortexReaderEmissionKind, VortexReaderSupport,
};
use vortex_array::arrays::struct_::StructArrayExt;
use vortex_array::arrays::{PrimitiveArray, StructArray, VarBinArray};
use vortex_array::dtype::{DType, FieldNames, Nullability};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::validity::Validity;
use vortex_array::{IntoArray, VortexSessionExecute};
use vortex_buffer::buffer;
use vortex_buffer::ByteBufferMut;
use vortex_file::{OpenOptionsSessionExt, WriteOptionsSessionExt};
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

fn scan_supported_table_oracle(bytes: &[u8]) -> (Vec<i32>, Vec<i64>) {
    let session = session();
    let file = session
        .open_options()
        .open_buffer(vortex_buffer::ByteBuffer::copy_from(bytes))
        .expect("open Vortex table");
    let array = RUNTIME
        .block_on(async {
            let stream = file.scan()?.into_array_stream()?;
            stream.read_all().await
        })
        .expect("scan Vortex table");
    let mut ctx = file.session().create_execution_ctx();
    let struct_array = array
        .execute::<StructArray>(&mut ctx)
        .expect("struct array");
    let id = struct_array
        .unmasked_field_by_name("id")
        .expect("id field")
        .clone()
        .execute::<PrimitiveArray>(&mut ctx)
        .expect("id primitive")
        .as_slice::<i32>()
        .to_vec();
    let score = struct_array
        .unmasked_field_by_name("score")
        .expect("score field")
        .clone()
        .execute::<PrimitiveArray>(&mut ctx)
        .expect("score primitive")
        .as_slice::<i64>()
        .to_vec();
    (id, score)
}

#[test]
fn table_to_loom_supported_struct_emits_verified_lmt1() {
    let vortex = supported_table_bytes();
    let facts = reader_facts_from_vortex_buffer(&vortex).expect("reader facts");
    assert_eq!(facts.support, VortexReaderSupport::Accepted);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::Lmt1);
    assert_eq!(facts.root_dtype.kind, "struct");
    assert_eq!(facts.root_dtype.field_names, vec!["id", "score"]);

    let (id_oracle, score_oracle) = scan_supported_table_oracle(&vortex);
    let loom = emit_supported_lmc1_from_vortex_buffer(&vortex).expect("emit LMT1 container");

    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(&loom, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);

    let table = decode_table_payload_maybe_container(&loom).expect("decode table");
    assert_eq!(table.row_count, 3);
    assert_eq!(table.columns[0].name, "id");
    assert_eq!(table.columns[1].name, "score");

    let arrays = decode_table_to_array_data(&table, &registry).expect("decode table arrays");
    let ids = Int32Array::from(arrays[0].clone());
    let scores = Int64Array::from(arrays[1].clone());
    assert_eq!(
        (0..ids.len()).map(|idx| ids.value(idx)).collect::<Vec<_>>(),
        id_oracle
    );
    assert_eq!(
        (0..scores.len())
            .map(|idx| scores.value(idx))
            .collect::<Vec<_>>(),
        score_oracle
    );
}

#[test]
fn table_to_loom_unsupported_field_fails_closed() {
    let vortex = unsupported_table_bytes();
    let facts = reader_facts_from_vortex_buffer(&vortex).expect("reader facts");
    assert_eq!(facts.support, VortexReaderSupport::Unsupported);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::None);
    assert_eq!(facts.root_dtype.field_names, vec!["id", "name"]);

    let report = emit_supported_lmc1_from_vortex_buffer(&vortex).expect_err("unsupported table");
    assert_eq!(report.status, VortexIngressStatus::Unsupported);
    assert!(report.facts.is_some());
    assert!(!report.diagnostics.is_empty());
}
