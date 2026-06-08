use std::sync::LazyLock;

use arrow::array::{ArrayData, Float32Array, Float64Array, Int32Array, Int64Array};
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::{decode_layout_payload_maybe_container, is_container_payload};
use loom_core::l1_model::decode_layout_to_array_data;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, reader_facts_from_vortex_buffer,
    scan_f32_values_from_vortex_buffer, scan_f64_values_from_vortex_buffer,
    scan_i32_values_from_vortex_buffer, scan_i64_values_from_vortex_buffer, VortexIngressStatus,
    VortexReaderEmissionKind, VortexReaderSupport,
};
use vortex_array::arrays::VarBinArray;
use vortex_array::dtype::{DType, Nullability};
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

fn decode_lmc1(bytes: &[u8]) -> ArrayData {
    assert!(is_container_payload(bytes));
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);

    let desc = decode_layout_payload_maybe_container(bytes).expect("decode LMC1 layout");
    decode_layout_to_array_data(&desc, &registry).expect("decode Loom layout")
}

#[test]
fn single_column_to_loom_accepts_i32_i64_f32_f64() {
    let i32_vortex = vortex_file_bytes(buffer![7i32, -1, 42]);
    let i64_vortex = vortex_file_bytes(buffer![7i64, -1, 42]);
    let f32_vortex = vortex_file_bytes(buffer![1.25f32, -2.5, 3.75]);
    let f64_vortex = vortex_file_bytes(buffer![1.25f64, -2.5, 3.75]);

    let i32_oracle = scan_i32_values_from_vortex_buffer(&i32_vortex).expect("i32 oracle");
    let i64_oracle = scan_i64_values_from_vortex_buffer(&i64_vortex).expect("i64 oracle");
    let f32_oracle = scan_f32_values_from_vortex_buffer(&f32_vortex).expect("f32 oracle");
    let f64_oracle = scan_f64_values_from_vortex_buffer(&f64_vortex).expect("f64 oracle");

    let i32_data =
        decode_lmc1(&emit_supported_lmc1_from_vortex_buffer(&i32_vortex).expect("i32 LMC1"));
    let i64_data =
        decode_lmc1(&emit_supported_lmc1_from_vortex_buffer(&i64_vortex).expect("i64 LMC1"));
    let f32_data =
        decode_lmc1(&emit_supported_lmc1_from_vortex_buffer(&f32_vortex).expect("f32 LMC1"));
    let f64_data =
        decode_lmc1(&emit_supported_lmc1_from_vortex_buffer(&f64_vortex).expect("f64 LMC1"));

    let i32_array = Int32Array::from(i32_data);
    let i64_array = Int64Array::from(i64_data);
    let f32_array = Float32Array::from(f32_data);
    let f64_array = Float64Array::from(f64_data);

    assert_eq!(
        (0..i32_array.len())
            .map(|idx| i32_array.value(idx))
            .collect::<Vec<_>>(),
        i32_oracle
    );
    assert_eq!(
        (0..i64_array.len())
            .map(|idx| i64_array.value(idx))
            .collect::<Vec<_>>(),
        i64_oracle
    );
    assert_eq!(
        (0..f32_array.len())
            .map(|idx| f32_array.value(idx).to_bits())
            .collect::<Vec<_>>(),
        f32_oracle
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        (0..f64_array.len())
            .map(|idx| f64_array.value(idx).to_bits())
            .collect::<Vec<_>>(),
        f64_oracle
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
}

#[test]
fn single_column_to_loom_reader_facts_mark_supported_matrix() {
    for vortex in [
        vortex_file_bytes(buffer![7i32, -1, 42]),
        vortex_file_bytes(buffer![7i64, -1, 42]),
        vortex_file_bytes(buffer![1.25f32, -2.5, 3.75]),
        vortex_file_bytes(buffer![1.25f64, -2.5, 3.75]),
    ] {
        let facts = reader_facts_from_vortex_buffer(&vortex).expect("reader facts");
        assert_eq!(facts.support, VortexReaderSupport::Accepted);
        assert_eq!(facts.emission_kind, VortexReaderEmissionKind::Lmp1);
        assert_eq!(facts.root_dtype.kind, "primitive");
        assert_eq!(facts.root_dtype.nullable, Some(false));
    }
}

#[test]
fn single_column_to_loom_unsupported_utf8_emits_no_bytes() {
    let rows = [Some("a"), Some("b"), Some("c")];
    let vortex = vortex_file_bytes(VarBinArray::from_iter(
        rows,
        DType::Utf8(Nullability::Nullable),
    ));
    let facts = reader_facts_from_vortex_buffer(&vortex).expect("reader facts");
    assert_eq!(facts.support, VortexReaderSupport::Unsupported);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::None);
    assert_eq!(facts.root_dtype.kind, "utf8");

    let report = emit_supported_lmc1_from_vortex_buffer(&vortex).expect_err("unsupported utf8");
    assert_eq!(report.status, VortexIngressStatus::Unsupported);
    assert!(report.facts.is_some());
}
