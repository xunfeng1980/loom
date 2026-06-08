use std::sync::LazyLock;

use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, reader_facts_from_vortex_buffer,
    VortexEmissionDisposition, VortexIngressStatus, VortexLoweringDisposition,
    VortexReaderEmissionKind, VortexReaderSupport,
};
use vortex_array::arrays::primitive::PrimitiveArrayExt;
use vortex_array::arrays::PrimitiveArray;
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::IntoArray;
use vortex_array::VortexSessionExecute;
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

fn scan_primitive(bytes: &[u8]) -> PrimitiveArray {
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
}

fn scan_i32_options(bytes: &[u8]) -> Vec<Option<i32>> {
    let primitive = scan_primitive(bytes);
    let validity = PrimitiveArrayExt::validity(&primitive);
    primitive
        .as_slice::<i32>()
        .iter()
        .enumerate()
        .map(|(idx, value)| validity.is_valid(idx).expect("validity").then_some(*value))
        .collect()
}

fn scan_i64_options(bytes: &[u8]) -> Vec<Option<i64>> {
    let primitive = scan_primitive(bytes);
    let validity = PrimitiveArrayExt::validity(&primitive);
    primitive
        .as_slice::<i64>()
        .iter()
        .enumerate()
        .map(|(idx, value)| validity.is_valid(idx).expect("validity").then_some(*value))
        .collect()
}

fn scan_f32_options(bytes: &[u8]) -> Vec<Option<u32>> {
    let primitive = scan_primitive(bytes);
    let validity = PrimitiveArrayExt::validity(&primitive);
    primitive
        .as_slice::<f32>()
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            validity
                .is_valid(idx)
                .expect("validity")
                .then_some(value.to_bits())
        })
        .collect()
}

fn scan_f64_options(bytes: &[u8]) -> Vec<Option<u64>> {
    let primitive = scan_primitive(bytes);
    let validity = PrimitiveArrayExt::validity(&primitive);
    primitive
        .as_slice::<f64>()
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            validity
                .is_valid(idx)
                .expect("validity")
                .then_some(value.to_bits())
        })
        .collect()
}

fn assert_nullable_primitive_facts(bytes: &[u8]) {
    let facts = reader_facts_from_vortex_buffer(bytes).expect("reader facts");
    assert_eq!(facts.support, VortexReaderSupport::Unsupported);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::None);
    assert_eq!(facts.root_dtype.kind, "primitive");
    assert_eq!(facts.root_dtype.nullable, Some(true));
    assert_eq!(facts.coverage.dtype_kind, "primitive");
    assert_eq!(facts.coverage.nullable, Some(true));
    assert_eq!(
        facts.coverage.reader_support,
        VortexReaderSupport::Unsupported
    );
    assert_eq!(facts.coverage.emission_kind, VortexReaderEmissionKind::None);
    assert_eq!(
        facts.coverage.emission_disposition,
        VortexEmissionDisposition::None
    );
    assert_eq!(
        facts.coverage.lowering_disposition,
        VortexLoweringDisposition::FailClosedDeferred
    );

    let report = emit_supported_lmc1_from_vortex_buffer(bytes).expect_err("nullable unsupported");
    assert_eq!(report.status, VortexIngressStatus::Unsupported);
    assert!(report.facts.is_some());
}

#[test]
fn nullable_primitive_coverage_preserves_nulls_and_fails_closed() {
    let i32_vortex = vortex_file_bytes(PrimitiveArray::from_option_iter([
        Some(7i32),
        None,
        Some(-1),
        Some(42),
    ]));
    let i64_vortex = vortex_file_bytes(PrimitiveArray::from_option_iter([
        Some(7i64),
        None,
        Some(-1),
        Some(42),
    ]));
    let f32_vortex = vortex_file_bytes(PrimitiveArray::from_option_iter([
        Some(1.25f32),
        None,
        Some(-2.5),
        Some(3.75),
    ]));
    let f64_vortex = vortex_file_bytes(PrimitiveArray::from_option_iter([
        Some(1.25f64),
        None,
        Some(-2.5),
        Some(3.75),
    ]));

    assert_eq!(
        scan_i32_options(&i32_vortex),
        vec![Some(7), None, Some(-1), Some(42)]
    );
    assert_eq!(
        scan_i64_options(&i64_vortex),
        vec![Some(7), None, Some(-1), Some(42)]
    );
    assert_eq!(
        scan_f32_options(&f32_vortex),
        vec![
            Some(1.25f32.to_bits()),
            None,
            Some((-2.5f32).to_bits()),
            Some(3.75f32.to_bits())
        ]
    );
    assert_eq!(
        scan_f64_options(&f64_vortex),
        vec![
            Some(1.25f64.to_bits()),
            None,
            Some((-2.5f64).to_bits()),
            Some(3.75f64.to_bits())
        ]
    );

    for bytes in [&i32_vortex, &i64_vortex, &f32_vortex, &f64_vortex] {
        assert_nullable_primitive_facts(bytes);
    }
}
