use std::sync::LazyLock;

use loom_vortex_ingress::{
    reader_facts_from_vortex_buffer, VortexIngressStatus, VortexReaderDiagnosticCode,
    VortexReaderEmissionKind, VortexReaderSupport,
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
fn reader_facts_contract_stable_strings() {
    assert_eq!(VortexReaderSupport::Accepted.as_str(), "accepted");
    assert_eq!(VortexReaderSupport::Unsupported.as_str(), "unsupported");
    assert_eq!(VortexReaderSupport::Rejected.as_str(), "rejected");
    assert_eq!(VortexReaderEmissionKind::None.as_str(), "none");
    assert_eq!(VortexReaderEmissionKind::Lmp1.as_str(), "LMP1");
    assert_eq!(VortexReaderEmissionKind::Lmt1.as_str(), "LMT1");
    assert_eq!(
        VortexReaderDiagnosticCode::VerificationRequired.as_str(),
        "READER_VERIFICATION_REQUIRED"
    );
}

#[test]
fn reader_facts_contract_supported_i32_reports_complete_boundary_fields() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);
    let facts = reader_facts_from_vortex_buffer(&bytes).expect("reader facts");

    assert_eq!(facts.row_count, 3);
    assert_eq!(facts.support, VortexReaderSupport::Accepted);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(facts.root_dtype.kind, "primitive");
    assert_eq!(facts.root_dtype.nullable, Some(false));
    assert!(facts.root_dtype.summary.contains("I32"));
    assert!(!facts.root_layout_encoding.is_empty());
    assert!(!facts.layout_facts.is_empty());
    assert!(!facts.dtype_facts.is_empty());
    assert!(!facts.segment_facts.is_empty());

    let root = &facts.layout_facts[0];
    assert_eq!(root.path, "$");
    assert!(!root.encoding_id.is_empty());
    assert_eq!(root.row_count, 3);
    assert!(root.dtype_summary.contains("I32"));
    assert_eq!(root.child_count, 2);
    assert!(root.child_type.is_none());
    assert!(root.child_name.is_none());
    assert!(root.child_row_offset.is_none());
    assert!(facts.layout_facts.len() >= root.child_count + 1);
    assert!(facts
        .layout_facts
        .iter()
        .any(|layout| !layout.segment_ids.is_empty()));

    for segment in &facts.segment_facts {
        assert_eq!(segment.length, segment.end.saturating_sub(segment.start));
        assert!(segment.ordered_after_previous);
        assert!(!segment.overlaps_previous);
        assert_eq!(segment.id as usize, segment.index);
        assert!(!segment.alignment.is_empty());
    }
}

#[test]
fn reader_facts_contract_unsupported_valid_file_emits_no_artifact_kind() {
    let bytes = vortex_file_bytes(buffer![7i64, -1, 42]);
    let facts = reader_facts_from_vortex_buffer(&bytes).expect("reader facts");

    assert_eq!(facts.row_count, 3);
    assert_eq!(facts.support, VortexReaderSupport::Unsupported);
    assert_eq!(facts.emission_kind, VortexReaderEmissionKind::None);
    assert!(facts.root_dtype.summary.contains("I64"));
}

#[test]
fn reader_facts_contract_malformed_buffer_fails_closed() {
    let report = reader_facts_from_vortex_buffer(b"not a vortex file").expect_err("rejected");

    assert_eq!(report.status, VortexIngressStatus::Rejected);
    assert!(report.facts.is_none());
}
