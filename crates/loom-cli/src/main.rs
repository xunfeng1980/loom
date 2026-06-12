#![allow(deprecated)]
use std::env;
use std::fs;

use loom_ir_core::full_verifier::verify_l2_core;
use loom_ir_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, L2DataType,
    OutputBuilderCapability, ResourceBudget, ScalarExpr, ScalarValue,
};
use loom_ir_core::l2core_codec::{decode_l2core_program, encode_l2core_program, l2core_program_hash};
use loom_ir_core::sidecar::SidecarOverlay;
use loom_parquet_ingress::sidecar_parquet::{
    chunk_bindings_from_parquet, embed_sidecar_into_parquet_file,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("loom: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;

    match command.as_str() {
        "sidecar" => {
            let mode = args.next().ok_or_else(|| sidecar_usage())?;
            sidecar(&mode, args.collect())
        }
        "verify-l2core" => {
            let mode = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            verify_l2core_cmd(&mode)
        }
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        cmd => Err(format!("unknown command: {cmd}. Run `loom help`.")),
    }
}

fn usage() -> String {
    let mut s = String::new();
    s.push_str("loom — Loom sidecar CLI (Phase 101)\n\n");
    s.push_str("USAGE:\n");
    s.push_str("  loom sidecar embed <parquet_file> [ir_file]          Embed sidecar inline (dev only)\n");
    s.push_str("  loom sidecar embed-external <parquet_file> [ir_file] Write external .loomsidecar file\n");
    s.push_str("  loom verify-l2core <mode>                            Verify an L2Core IR program\n");
    s.push_str("  loom help                                            Print this message\n");
    s
}

fn sidecar_usage() -> String {
    "USAGE: loom sidecar <embed|embed-external> <parquet_file> [ir_file]".to_string()
}

// ── sidecar ───────────────────────────────────────────────────────────────

fn sidecar(mode: &str, args: Vec<String>) -> Result<(), String> {
    match mode {
        "embed" => sidecar_embed(args),
        "embed-external" => sidecar_embed_external(args),
        _ => Err(format!("unknown sidecar command: {mode}. Use `sidecar embed` or `sidecar embed-external`.")),
    }
}

#[allow(deprecated)]
fn sidecar_embed(mut args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err(sidecar_usage());
    }
    let parquet_path = args.remove(0);

    let program = match args.first() {
        Some(ref path) => {
            let bytes = fs::read(path)
                .map_err(|e| format!("failed to read IR file {path}: {e}"))?;
            decode_l2core_program(&bytes)
                .map_err(|e| format!("failed to decode L2Core program: {e}"))?
        }
        None => default_sidecar_program(),
    };

    let ir_bytes = encode_l2core_program(&program);

    // Generate real ChunkBindings from the Parquet file's column data.
    let bindings = chunk_bindings_from_parquet(std::path::Path::new(&parquet_path))
        .map_err(|e| format!("failed to compute chunk bindings: {e}"))?;

    let overlay = SidecarOverlay {
        ir_bytes,
        bindings,
    };

    embed_sidecar_into_parquet_file(std::path::Path::new(&parquet_path), &overlay)
        .map_err(|e| format!("failed to embed sidecar: {e}"))?;

    let hash = program.content_hash();
    eprintln!("WARNING: embed rewrites data pages via ArrowWriter (non-production).");
    eprintln!("  For production, use metadata-only embed or the external sidecar model.");
    println!("Embedded sidecar in {}", parquet_path);
    println!("Sidecar content hash: {}", hash);
    Ok(())
}

/// Write a sidecar overlay to a separate `.loomsidecar` file.
///
/// This is the production path: the original file is never touched.
/// The sidecar lives alongside the data file as `<path>.loomsidecar`.
fn sidecar_embed_external(mut args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err(sidecar_usage());
    }
    let parquet_path = args.remove(0);

    let program = match args.first() {
        Some(ref path) => {
            let bytes = fs::read(path)
                .map_err(|e| format!("failed to read IR file {path}: {e}"))?;
            decode_l2core_program(&bytes)
                .map_err(|e| format!("failed to decode L2Core program: {e}"))?
        }
        None => default_sidecar_program(),
    };

    let ir_bytes = encode_l2core_program(&program);
    let bindings = chunk_bindings_from_parquet(std::path::Path::new(&parquet_path))
        .map_err(|e| format!("failed to compute chunk bindings: {e}"))?;

    let overlay = SidecarOverlay { ir_bytes, bindings };
    let sidecar_bytes = overlay.encode();

    let sidecar_path = format!("{parquet_path}.loomsidecar");
    fs::write(&sidecar_path, &sidecar_bytes)
        .map_err(|e| format!("failed to write sidecar file {sidecar_path}: {e}"))?;

    let hash = program.content_hash();
    println!("Wrote external sidecar: {}", sidecar_path);
    println!("  original: {} (unchanged)", parquet_path);
    println!("Sidecar content hash: {}", hash);
    Ok(())
}

fn default_sidecar_program() -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec![],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input".to_string(),
                offset: 0,
                length: 1024,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: 1024,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(1024),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(1024)),
            body: vec![L2CoreStmt::AppendValue {
                builder: "output".to_string(),
                value: ScalarExpr::Var("i".to_string()),
            }],
        }],
    }
}

// ── verify-l2core ────────────────────────────────────────────────────────

fn verify_l2core_cmd(mode: &str) -> Result<(), String> {
    match mode {
        "--sample" => {
            let program = default_sidecar_program();
            let result = verify_l2_core(&program);
            if result.is_ok() {
                println!("Verification: passed");
            } else {
                println!("Verification: failed");
                for diag in result.diagnostics() {
                    println!("  [{:?}] {}: {}", diag.code, diag.path, diag.message);
                }
            }
            Ok(())
        }
        path => {
            let bytes =
                fs::read(path).map_err(|e| format!("failed to read IR file {path}: {e}"))?;
            let program = decode_l2core_program(&bytes)
                .map_err(|e| format!("failed to decode L2Core program: {e}"))?;
            let hash = l2core_program_hash(&program);
            println!("L2Core program hash: {}", hash);
            let result = verify_l2_core(&program);
            if result.is_ok() {
                println!("Verification: passed");
            } else {
                println!("Verification: failed");
                for diag in result.diagnostics() {
                    println!("  [{:?}] {}: {}", diag.code, diag.path, diag.message);
                }
            }
            Ok(())
        }
    }
}
