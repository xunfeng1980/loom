use std::sync::LazyLock;

use loom_vortex_ingress::{
    inspect_vortex_buffer, reader_facts_from_vortex_buffer, VortexReaderDiagnosticCode,
    VortexReaderSupport,
};
use vortex_array::arrays::StructArray;
use vortex_array::dtype::FieldNames;
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

fn struct_vortex_bytes() -> Vec<u8> {
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
fn reader_recursive_facts_layout_tree_is_deterministic() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);
    let first = reader_facts_from_vortex_buffer(&bytes).expect("reader facts");
    let second = reader_facts_from_vortex_buffer(&bytes).expect("reader facts repeat");

    assert_eq!(first.layout_facts, second.layout_facts);
    assert!(!first.layout_facts.is_empty());
    assert!(first
        .layout_facts
        .iter()
        .any(|layout| layout.path.starts_with("$.children[")));

    for layout in &first.layout_facts {
        assert!(!layout.path.is_empty());
        assert!(!layout.encoding_id.is_empty());
        assert!(!layout.dtype_summary.is_empty());
        assert!(layout.row_count <= first.row_count);
    }
}

#[test]
fn reader_recursive_facts_dtype_records_struct_fields() {
    let bytes = struct_vortex_bytes();
    let facts = reader_facts_from_vortex_buffer(&bytes).expect("reader facts");

    assert_eq!(facts.support, VortexReaderSupport::Unsupported);
    assert_eq!(facts.root_dtype.kind, "struct");
    assert_eq!(facts.root_dtype.nullable, Some(false));
    assert_eq!(facts.root_dtype.field_count, Some(2));
    assert_eq!(facts.root_dtype.field_names, vec!["id", "score"]);
    assert!(facts
        .dtype_facts
        .iter()
        .any(|dtype| dtype.kind == "struct" && dtype.field_count == Some(2)));
}

#[test]
fn reader_recursive_facts_segments_match_legacy_facts() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);
    let legacy = inspect_vortex_buffer(&bytes)
        .facts
        .expect("legacy ingress facts");
    let facts = reader_facts_from_vortex_buffer(&bytes).expect("reader facts");

    assert_eq!(facts.segment_facts.len(), legacy.segment_count);
    for (segment, legacy_range) in facts.segment_facts.iter().zip(legacy.segment_ranges) {
        assert_eq!((segment.start, segment.end), legacy_range);
        assert_eq!(segment.length, segment.end.saturating_sub(segment.start));
        assert!(segment.ordered_after_previous);
        assert!(!segment.overlaps_previous);
    }
}

#[test]
fn reader_recursive_facts_split_discovery_is_recorded_or_diagnosed() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);
    let facts = reader_facts_from_vortex_buffer(&bytes).expect("reader facts");

    if facts.split_facts.is_empty() {
        assert!(facts
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == VortexReaderDiagnosticCode::SplitUnavailable }));
    } else {
        let total_rows = facts
            .split_facts
            .iter()
            .map(|split| {
                assert!(split.start_row <= split.end_row);
                assert_eq!(split.row_count, split.end_row - split.start_row);
                split.row_count
            })
            .sum::<u64>();
        assert_eq!(total_rows, facts.row_count);
    }
}
