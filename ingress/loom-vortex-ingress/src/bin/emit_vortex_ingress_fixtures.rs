use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use loom_vortex_ingress::{emit_supported_lmc1_from_vortex_buffer, inspect_vortex_buffer};
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

fn main() -> Result<(), String> {
    let vortex_bytes = supported_vortex_bytes();
    let loom_bytes = emit_supported_lmc1_from_vortex_buffer(&vortex_bytes)
        .map_err(|report| format!("failed to emit supported fixture: {report:?}"))?;
    let report = inspect_vortex_buffer(&vortex_bytes);
    let facts = report.facts.as_ref().ok_or("fixture should have facts")?;

    let vortex_path = Path::new("fixtures/vortex/int32-flat.vortex");
    let loom_path = Path::new("fixtures/loom/int32-flat.loom");
    fs::create_dir_all(vortex_path.parent().ok_or("missing vortex parent")?)
        .map_err(|err| format!("failed to create vortex fixture dir: {err}"))?;
    fs::create_dir_all(loom_path.parent().ok_or("missing loom parent")?)
        .map_err(|err| format!("failed to create loom fixture dir: {err}"))?;
    fs::write(vortex_path, &vortex_bytes)
        .map_err(|err| format!("failed to write Vortex fixture: {err}"))?;
    fs::write(loom_path, &loom_bytes)
        .map_err(|err| format!("failed to write Loom fixture: {err}"))?;

    println!("Vortex ingress fixture emitted");
    println!("vortex: {}", vortex_path.display());
    println!("loom: {}", loom_path.display());
    println!("status: {}", report.status.as_str());
    println!("row_count: {}", facts.row_count);
    println!("dtype: {}", facts.dtype_summary);
    println!("layout: {}", facts.layout_summary);
    println!("segments: {}", facts.segment_count);
    println!("supported: {}", facts.supported_loom_payload);

    Ok(())
}

fn supported_vortex_bytes() -> Vec<u8> {
    let session = VortexSession::empty()
        .with::<MemorySession>()
        .with::<ArraySession>()
        .with::<LayoutSession>()
        .with::<ScalarFnSession>()
        .with::<RuntimeSession>()
        .with_handle(RUNTIME.handle());
    vortex_file::register_default_encodings(&session);

    let mut buf = ByteBufferMut::empty();
    let array = buffer![7i32, -1, 42, 99].into_array();
    RUNTIME
        .block_on(
            session
                .write_options()
                .write(&mut buf, array.to_array_stream()),
        )
        .expect("write deterministic Vortex fixture");
    buf.as_slice().to_vec()
}
