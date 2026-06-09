use std::sync::LazyLock;

use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, reader_facts_from_vortex_buffer,
    VortexEmissionDisposition, VortexLoweringDisposition, VortexReaderEmissionKind,
    VortexReaderSupport,
};
use vortex_array::arrays::{DictArray, PrimitiveArray};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
use vortex_buffer::ByteBufferMut;
use vortex_fastlanes::RLEData;
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
    vortex_fastlanes::initialize(&session);
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

fn dictionary_i32_bytes() -> Vec<u8> {
    let values = PrimitiveArray::from_iter([10i32, 20, 30]);
    let codes = PrimitiveArray::from_iter([2i32, 0, 1, 2]);
    let dict = DictArray::try_new(codes.into_array(), values.into_array()).expect("dict");
    vortex_file_bytes(dict)
}

fn runend_i32_bytes() -> Vec<u8> {
    let session = session();
    let mut ctx = session.create_execution_ctx();
    let input = PrimitiveArray::from_iter([1i32, 1, 2, 2, 2, 3]);
    let rle = RLEData::encode(input.as_view(), &mut ctx).expect("RLEData::encode");
    vortex_file_bytes(rle)
}

fn assert_accepted_canonical_raw(bytes: &[u8]) {
    let facts = reader_facts_from_vortex_buffer(bytes).expect("reader facts");
    assert_eq!(facts.support, VortexReaderSupport::Accepted);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(
        facts.coverage.emission_disposition,
        VortexEmissionDisposition::CanonicalRaw
    );

    if matches!(
        facts.coverage.array_encoding.as_str(),
        "dictionary" | "run-end" | "sequence"
    ) {
        assert_eq!(
            facts.coverage.lowering_disposition,
            VortexLoweringDisposition::InterpreterOnly
        );
    } else {
        assert_eq!(facts.coverage.array_encoding, "primitive");
        assert_eq!(
            facts.coverage.lowering_disposition,
            VortexLoweringDisposition::ProductionLoweringSupported
        );
    }

    let emitted = emit_supported_lmc1_from_vortex_buffer(bytes).expect("canonical raw artifact");
    assert!(!emitted.is_empty());
}

#[test]
fn dictionary_coverage_records_oracle_and_safe_disposition() {
    let bytes = dictionary_i32_bytes();
    assert_eq!(scan_i32_rows(&bytes), vec![30, 10, 20, 30]);
    assert_accepted_canonical_raw(&bytes);
}

#[test]
fn runend_coverage_records_oracle_and_safe_disposition() {
    let bytes = runend_i32_bytes();
    assert_eq!(scan_i32_rows(&bytes), vec![1, 1, 2, 2, 2, 3]);
    assert_accepted_canonical_raw(&bytes);
}
