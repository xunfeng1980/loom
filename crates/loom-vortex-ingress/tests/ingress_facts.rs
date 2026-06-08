use std::fs;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use loom_vortex_ingress::{
    inspect_vortex_buffer, inspect_vortex_path, VortexIngressDiagnosticCode,
    VortexIngressSourceKind, VortexIngressStatus,
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

fn simple_i32_vortex_bytes() -> Vec<u8> {
    let session = session();
    let mut buf = ByteBufferMut::empty();
    let array = buffer![10i32, 20, 30].into_array();
    RUNTIME
        .block_on(
            session
                .write_options()
                .write(&mut buf, array.to_array_stream()),
        )
        .expect("write simple Vortex file");
    buf.as_slice().to_vec()
}

#[test]
fn ingress_facts_real_buffer_reports_owned_metadata() {
    let bytes = simple_i32_vortex_bytes();
    let report = inspect_vortex_buffer(&bytes);
    assert_eq!(report.status, VortexIngressStatus::Accepted);

    let facts = report.facts.expect("facts for valid Vortex file");
    assert_eq!(facts.source_kind, VortexIngressSourceKind::Buffer);
    assert_eq!(facts.row_count, 3);
    assert!(facts.dtype_summary.contains("I32"));
    assert!(!facts.layout_summary.is_empty());
    assert_eq!(facts.segment_count, facts.segment_ranges.len());
    assert_eq!(facts.segment_count, facts.alignment_summary.len());
    assert!(facts.supported_loom_payload);
}

#[test]
fn ingress_facts_real_path_reports_path_source_kind() {
    let bytes = simple_i32_vortex_bytes();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("loom-ingress-{nonce}.vortex"));
    fs::write(&path, bytes).expect("write temp Vortex file");

    let report = inspect_vortex_path(&path);
    let _ = fs::remove_file(&path);

    assert_eq!(report.status, VortexIngressStatus::Accepted);
    let facts = report.facts.expect("facts for path Vortex file");
    assert_eq!(facts.source_kind, VortexIngressSourceKind::Path);
    assert_eq!(facts.row_count, 3);
}

#[test]
fn ingress_facts_malformed_buffers_fail_closed() {
    let valid = simple_i32_vortex_bytes();
    let cases: Vec<&[u8]> = vec![
        &[],
        b"not a vortex file",
        &valid[..valid.len().saturating_sub(4)],
    ];

    for case in cases {
        let report = inspect_vortex_buffer(case);
        assert_eq!(report.status, VortexIngressStatus::Rejected);
        assert!(report.facts.is_none());
        assert_eq!(
            report.diagnostics[0].code,
            VortexIngressDiagnosticCode::OpenFailed
        );
    }
}
