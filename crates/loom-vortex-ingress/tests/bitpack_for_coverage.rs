use std::sync::LazyLock;

use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, reader_facts_from_vortex_buffer,
    VortexEmissionDisposition, VortexLoweringDisposition, VortexReaderEmissionKind,
    VortexReaderSupport,
};
use vortex_array::arrays::PrimitiveArray;
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
use vortex_buffer::ByteBufferMut;
use vortex_fastlanes::{BitPackedData, FoR};
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

fn bitpacked_i32_bytes() -> (Vec<u8>, Vec<i32>) {
    let session = session();
    let mut ctx = session.create_execution_ctx();
    let expected = (0i32..150).map(|idx| idx * 37 % 128).collect::<Vec<_>>();
    let primitive = PrimitiveArray::from_iter(expected.iter().copied());
    let packed =
        BitPackedData::encode(&primitive.into_array(), 7, &mut ctx).expect("BitPackedData");
    (vortex_file_bytes(packed), expected)
}

fn for_i32_bytes() -> (Vec<u8>, Vec<i32>) {
    let session = session();
    let mut ctx = session.create_execution_ctx();
    let reference = 1000i32;
    let deltas = (0i32..100).collect::<Vec<_>>();
    let primitive = PrimitiveArray::from_iter(deltas.iter().copied());
    let packed =
        BitPackedData::encode(&primitive.into_array(), 7, &mut ctx).expect("BitPackedData");
    let for_array = FoR::try_new(packed.into_array(), reference.into()).expect("FoR");
    (
        vortex_file_bytes(for_array),
        deltas
            .iter()
            .map(|delta| reference + *delta)
            .collect::<Vec<_>>(),
    )
}

fn assert_canonical_or_deferred_numeric(bytes: &[u8]) {
    let facts = reader_facts_from_vortex_buffer(bytes).expect("reader facts");
    assert_eq!(facts.support, VortexReaderSupport::Accepted);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(
        facts.coverage.emission_disposition,
        VortexEmissionDisposition::CanonicalRaw
    );

    match facts.coverage.array_encoding.as_str() {
        "bitpack" | "frame-of-reference" => assert_eq!(
            facts.coverage.lowering_disposition,
            VortexLoweringDisposition::InterpreterOnly
        ),
        "primitive" => assert_eq!(
            facts.coverage.lowering_disposition,
            VortexLoweringDisposition::ProductionLoweringSupported
        ),
        other => panic!("unexpected numeric coverage encoding: {other}"),
    }

    let emitted = emit_supported_lmc1_from_vortex_buffer(bytes).expect("canonical raw artifact");
    assert!(!emitted.is_empty());
}

#[test]
fn bitpack_coverage_records_oracle_and_native_delta_boundary() {
    let (bytes, expected) = bitpacked_i32_bytes();
    assert_eq!(scan_i32_rows(&bytes), expected);
    assert_canonical_or_deferred_numeric(&bytes);
}

#[test]
fn for_coverage_records_oracle_and_native_delta_boundary() {
    let (bytes, expected) = for_i32_bytes();
    assert_eq!(scan_i32_rows(&bytes), expected);
    assert_canonical_or_deferred_numeric(&bytes);
}
