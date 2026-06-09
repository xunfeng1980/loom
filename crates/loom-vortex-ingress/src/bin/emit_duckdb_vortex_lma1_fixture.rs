use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use loom_vortex_ingress::{
    emit_source_ingress_lma1_from_vortex_buffer, emit_source_ingress_lmc2_from_vortex_buffer,
};
use vortex_array::arrays::PrimitiveArray;
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::IntoArray;
use vortex_buffer::ByteBufferMut;
use vortex_file::WriteOptionsSessionExt;
use vortex_io::runtime::current::CurrentThreadRuntime;
use vortex_io::runtime::BlockingRuntime;
use vortex_io::session::RuntimeSession;
use vortex_io::session::RuntimeSessionExt;
use vortex_layout::session::LayoutSession;
use vortex_session::VortexSession;

static RUNTIME: LazyLock<CurrentThreadRuntime> = LazyLock::new(CurrentThreadRuntime::new);

fn main() {
    if let Err(err) = run() {
        eprintln!("emit_duckdb_vortex_lma1_fixture: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let out_dir = env::args()
        .nth(1)
        .ok_or_else(|| "usage: emit_duckdb_vortex_lma1_fixture <output-dir>".to_string())?;
    let out_dir = PathBuf::from(out_dir);
    fs::create_dir_all(&out_dir).map_err(|err| format!("create {}: {err}", out_dir.display()))?;

    let source_path = out_dir.join("source.vortex");
    let loom_path = out_dir.join("vortex.loom");
    let duckdb_bridge_path = out_dir.join("vortex-duckdb-bridge-lma1.loom");
    let vortex_bytes = vortex_file_bytes(PrimitiveArray::from_iter([7i32, -1, 42]));
    fs::write(&source_path, &vortex_bytes)
        .map_err(|err| format!("write {}: {err}", source_path.display()))?;

    let accepted = emit_source_ingress_lmc2_from_vortex_buffer(&vortex_bytes)
        .map_err(|report| format!("emit LMC2 from Vortex failed: {:?}", report.diagnostics))?;
    let duckdb_bridge =
        emit_source_ingress_lma1_from_vortex_buffer(&vortex_bytes).map_err(|report| {
            format!(
                "emit direct LMA1 from Vortex failed: {:?}",
                report.diagnostics
            )
        })?;
    fs::write(&loom_path, &accepted.bytes)
        .map_err(|err| format!("write {}: {err}", loom_path.display()))?;
    fs::write(&duckdb_bridge_path, duckdb_bridge.bytes)
        .map_err(|err| format!("write {}: {err}", duckdb_bridge_path.display()))?;

    println!("source: {}", source_path.display());
    println!("loom: {}", loom_path.display());
    println!("duckdb_bridge_lma1: {}", duckdb_bridge_path.display());
    println!("status: {}", accepted.report.status.as_str());
    println!("emission_kind: {}", accepted.report.emission_kind.as_str());
    Ok(())
}

fn vortex_file_bytes<T: IntoArray>(array: T) -> Vec<u8> {
    let session = VortexSession::empty()
        .with::<MemorySession>()
        .with::<ArraySession>()
        .with::<LayoutSession>()
        .with::<ScalarFnSession>()
        .with::<RuntimeSession>()
        .with_handle(RUNTIME.handle());
    vortex_file::register_default_encodings(&session);
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
