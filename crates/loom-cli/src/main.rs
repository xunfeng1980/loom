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
        "gen-ir" => {
            let path = args.next().ok_or_else(|| "USAGE: loom gen-ir <output.l2ir|output.ron>".to_string())?;
            gen_ir(&path)
        }
        "convert" => {
            let input = args.next().ok_or_else(|| convert_usage())?;
            let output = args.next().ok_or_else(|| convert_usage())?;
            convert(&input, &output)
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
    s.push_str("  loom gen-ir <output.l2ir|output.ron>                 Generate L2Core IR file (binary or RON)\n");
    s.push_str("  loom convert <input> <output>                         Convert between .l2ir and .ron\n");
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

/// Generate a default L2Core IR program and write it to a file.
/// Auto-detects format by extension: .l2ir = binary, .ron = RON text.
fn gen_ir(path: &str) -> Result<(), String> {
    let program = default_sidecar_program();
    if path.ends_with(".ron") {
        let text = ron::ser::to_string_pretty(&program, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("RON serialization failed: {e}"))?;
        fs::write(path, &text)
            .map_err(|e| format!("failed to write: {e}"))?;
        println!("Wrote L2Core IR (RON): {path}");
    } else {
        let bytes = encode_l2core_program(&program);
        fs::write(path, &bytes)
            .map_err(|e| format!("failed to write: {e}"))?;
        println!("Wrote L2Core IR (binary): {path}");
    }
    let hash = program.content_hash();
    println!("Content hash: {hash}");
    Ok(())
}

fn convert_usage() -> String {
    "USAGE: loom convert <input> <output>\n  input/output extensions: .l2ir (binary) or .ron (text)".to_string()
}

/// Convert between .l2ir and .ron formats.
fn convert(input: &str, output: &str) -> Result<(), String> {
    let program = if input.ends_with(".ron") {
        let text = fs::read_to_string(input)
            .map_err(|e| format!("read {input}: {e}"))?;
        ron::from_str(&text)
            .map_err(|e| format!("RON parse error in {input}: {e}"))?
    } else {
        let bytes = fs::read(input)
            .map_err(|e| format!("read {input}: {e}"))?;
        decode_l2core_program(&bytes)
            .map_err(|e| format!("L2Core decode error in {input}: {e}"))?
    };

    if output.ends_with(".ron") {
        let text = ron::ser::to_string_pretty(&program, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("RON serialization: {e}"))?;
        fs::write(output, &text)
            .map_err(|e| format!("write {output}: {e}"))?;
        println!("Converted: {input} → {output} (RON)");
    } else {
        let bytes = encode_l2core_program(&program);
        fs::write(output, &bytes)
            .map_err(|e| format!("write {output}: {e}"))?;
        println!("Converted: {input} → {output} (binary .l2ir)");
    }
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
                length: 20,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "output".to_string(),
                arrow_type: L2DataType::Int32,
                nullable: false,
                max_events: 5,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(5),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::Const(ScalarValue::UInt64(0)),
            end: ScalarExpr::Const(ScalarValue::UInt64(5)),
            body: vec![L2CoreStmt::AppendValue {
                builder: "output".to_string(),
                value: ScalarExpr::Const(ScalarValue::Int32(42)),
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
