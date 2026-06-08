use std::sync::LazyLock;

use arrow::array::Int32Array;
use loom_core::container_codec::{decode_layout_payload_maybe_container, is_container_payload};
use loom_core::l1_model::decode_layout_to_array_data;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::verifier::verify_container;
use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, scan_i32_values_from_vortex_buffer,
    VortexIngressDiagnosticCode, VortexIngressStatus,
};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
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

#[test]
fn real_file_to_loom_supported_i32_roundtrips_with_oracle_rows() {
    let vortex = vortex_file_bytes(buffer![7i32, -1, 42, 99]);
    let oracle_values = scan_i32_values_from_vortex_buffer(&vortex).expect("Vortex scan oracle");

    let loom = emit_supported_lmc1_from_vortex_buffer(&vortex).expect("emit LMC1");
    assert!(is_container_payload(&loom));

    let registry = L2KernelRegistry::default_for_mvp0();
    let verification = verify_container(&loom, &registry);
    assert!(
        verification.is_ok(),
        "emitted LMC1 must verify: {:?}",
        verification.diagnostics()
    );

    let desc = decode_layout_payload_maybe_container(&loom).expect("decode LMC1 layout payload");
    let data = decode_layout_to_array_data(&desc, &registry).expect("decode Loom layout");
    let array = Int32Array::from(data);
    let loom_values: Vec<i32> = (0..array.len()).map(|idx| array.value(idx)).collect();

    assert_eq!(loom_values, oracle_values);
}

#[test]
fn real_file_to_loom_unsupported_i64_fails_closed() {
    let vortex = vortex_file_bytes(buffer![7i64, -1, 42, 99]);
    let report = emit_supported_lmc1_from_vortex_buffer(&vortex).expect_err("unsupported i64");
    assert_eq!(report.status, VortexIngressStatus::Unsupported);
    assert_eq!(
        report.diagnostics[0].code,
        VortexIngressDiagnosticCode::UnsupportedConversion
    );
    assert!(
        !report
            .facts
            .expect("facts for opened unsupported file")
            .supported_loom_payload
    );
}
