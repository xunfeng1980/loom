//! Generate a LoomNative-routing fixture for the DuckDB extension E2E demo.
//!
//! Usage: cargo run -p loom-ffi --example make_fixture -- <host_path>
//!
//! Writes:
//!   <host_path>            — 100 bytes of host data
//!   <host_path>.loomsidecar — sidecar overlay (const-append i32 program +
//!                             a content-hash binding matching the host bytes)
//!
//! Then: loom_scan('<host_path>') routes LoomNative and decodes 10 rows x 1 col.

use loom_ir_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
};
use loom_ir_core::l2core_codec::{encode_l2core_program, l2core_program_hash};
use loom_ir_core::sidecar::{compute_chunk_hash, ChunkBinding, SidecarOverlay};

fn main() {
    let host_path = std::env::args()
        .nth(1)
        .expect("usage: make_fixture <host_path>");

    // 100 bytes of deterministic host data.
    let host: Vec<u8> = (0..100u32).map(|i| (i % 256) as u8).collect();

    // const-append program: for i in 0..10 { append(output, 42) }.
    let program = L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input".to_string(),
                offset: 0,
                length: host.len() as u64,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: 10,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(10),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(10)),
            body: vec![L2CoreStmt::AppendValue {
                builder: "output".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(42)),
            }],
        }],
    };

    let ir_bytes = encode_l2core_program(&program);
    let binding = ChunkBinding {
        granule_id: "output".to_string(),
        host_data_range: (0, host.len() as u64),
        content_hash: compute_chunk_hash(&host),
        ir_identity: l2core_program_hash(&program),
    };
    let overlay = SidecarOverlay {
        ir_bytes,
        bindings: vec![binding],
    };

    std::fs::write(&host_path, &host).expect("write host");
    std::fs::write(format!("{host_path}.loomsidecar"), overlay.encode()).expect("write sidecar");

    println!("wrote {host_path} ({} bytes) + {host_path}.loomsidecar", host.len());
}
