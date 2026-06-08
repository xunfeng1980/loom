use std::sync::LazyLock;

use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, reader_facts_from_vortex_buffer,
    VortexEmissionDisposition, VortexLoweringDisposition, VortexReaderEmissionKind,
    VortexReaderSupport,
};
use vortex_array::arrays::{ChunkedArray, PrimitiveArray};
use vortex_array::dtype::{DType, Nullability, PType};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
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

fn chunked_i32_bytes() -> Vec<u8> {
    let chunk1 = buffer![1i32, 2, 3].into_array();
    let chunk2 = buffer![4i32, 5].into_array();
    let chunk3 = buffer![6i32, 7, 8, 9].into_array();
    let dtype = DType::Primitive(PType::I32, Nullability::NonNullable);
    let array = ChunkedArray::try_new(vec![chunk1, chunk2, chunk3], dtype).expect("chunked i32");
    vortex_file_bytes(array)
}

fn scan_i32_rows(bytes: &[u8]) -> Vec<i32> {
    let session = session();
    let file = session
        .open_options()
        .open_buffer(vortex_buffer::ByteBuffer::copy_from(bytes))
        .expect("open Vortex file");
    let array = RUNTIME
        .block_on(async {
            let stream = file.scan()?.into_array_stream()?;
            stream.read_all().await
        })
        .expect("scan Vortex file");
    let mut ctx = file.session().create_execution_ctx();
    array
        .execute::<PrimitiveArray>(&mut ctx)
        .expect("scan primitive")
        .as_slice::<i32>()
        .to_vec()
}

#[test]
fn chunked_primitive_coverage_records_row_order_and_lowering_boundary() {
    let vortex = chunked_i32_bytes();
    assert_eq!(scan_i32_rows(&vortex), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

    let facts = reader_facts_from_vortex_buffer(&vortex).expect("reader facts");
    assert_eq!(facts.row_count, 9);
    assert_eq!(facts.support, VortexReaderSupport::Accepted);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(facts.coverage.reader_support, VortexReaderSupport::Accepted);
    assert_eq!(facts.coverage.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(
        facts.coverage.emission_disposition,
        VortexEmissionDisposition::CanonicalRaw
    );
    assert_eq!(facts.coverage.dtype_kind, "primitive");
    assert_eq!(facts.coverage.nullable, Some(false));

    if facts.coverage.layout_class == "chunked" {
        assert_eq!(
            facts.coverage.lowering_disposition,
            VortexLoweringDisposition::InterpreterOnly
        );
    } else {
        assert_eq!(
            facts.coverage.lowering_disposition,
            VortexLoweringDisposition::ProductionLoweringSupported
        );
    }
    assert!(
        facts.coverage.has_splits || facts.coverage.layout_class != "chunked",
        "chunked logical layout should expose either split facts or a non-chunked canonicalized root"
    );

    let emitted = emit_supported_lmc1_from_vortex_buffer(&vortex).expect("canonical raw artifact");
    assert!(!emitted.is_empty());
}
