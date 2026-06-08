use std::env;
use std::fs;
use std::path::Path;

use arrow::array::{Array, BooleanArray, Int32Array, Int64Array, StringArray};
use arrow_schema::DataType;
use loom_core::descriptor::{from_descriptor_text, payload_to_descriptor_text};
use loom_core::error::LoomDecodeError;
use loom_core::l1_model::{decode_layout_to_array_data, LayoutDescription, LayoutNode};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::decode_layout_payload;

fn main() {
    if let Err(err) = run() {
        eprintln!("loom: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    let input = args.next().ok_or_else(usage)?;
    if args.next().is_some() {
        return Err(usage());
    }

    match command.as_str() {
        "inspect" => inspect(Path::new(&input)),
        "decode" => decode(Path::new(&input)),
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n{}", usage())),
    }
}

fn usage() -> String {
    "usage: loom <inspect|decode> <payload-or-descriptor>".to_string()
}

fn inspect(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let desc = load_layout(&bytes)?;
    println!("input: {}", path.display());
    println!("data_type: {}", data_type_name(&desc.data_type));
    println!("row_count: {}", desc.row_count);
    println!("layout:");
    print_node(&desc.root, 1);
    println!("descriptor:");
    let text = if is_binary_payload(&bytes) {
        payload_to_descriptor_text(&bytes).map_err(display_decode_error)?
    } else {
        let input = std::str::from_utf8(&bytes)
            .map_err(|err| format!("descriptor is not valid UTF-8: {err}"))?;
        let desc = from_descriptor_text(input).map_err(display_decode_error)?;
        loom_core::descriptor::to_descriptor_text(&desc).map_err(display_decode_error)?
    };
    println!("{text}");
    Ok(())
}

fn decode(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let desc = load_layout(&bytes)?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let data = decode_layout_to_array_data(&desc, &registry).map_err(display_decode_error)?;
    match desc.data_type {
        DataType::Boolean => {
            let array = BooleanArray::from(data);
            for row in 0..array.len() {
                if array.is_null(row) {
                    println!("NULL");
                } else {
                    println!("{}", array.value(row));
                }
            }
        }
        DataType::Int32 => {
            let array = Int32Array::from(data);
            for row in 0..array.len() {
                if array.is_null(row) {
                    println!("NULL");
                } else {
                    println!("{}", array.value(row));
                }
            }
        }
        DataType::Int64 => {
            let array = Int64Array::from(data);
            for row in 0..array.len() {
                if array.is_null(row) {
                    println!("NULL");
                } else {
                    println!("{}", array.value(row));
                }
            }
        }
        DataType::Utf8 => {
            let array = StringArray::from(data);
            for row in 0..array.len() {
                if array.is_null(row) {
                    println!("NULL");
                } else {
                    println!("{}", array.value(row));
                }
            }
        }
        other => return Err(format!("unsupported output type {other:?}")),
    }
    Ok(())
}

fn load_layout(bytes: &[u8]) -> Result<LayoutDescription, String> {
    if is_binary_payload(bytes) {
        return decode_layout_payload(bytes).map_err(display_decode_error);
    }
    let input = std::str::from_utf8(bytes)
        .map_err(|err| format!("input is neither LMP1 payload nor UTF-8 descriptor: {err}"))?;
    from_descriptor_text(input).map_err(display_decode_error)
}

fn is_binary_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(b"LMP1")
}

fn print_node(node: &LayoutNode, depth: usize) {
    let indent = "  ".repeat(depth);
    match node {
        LayoutNode::Raw {
            elem_size,
            count,
            data,
        } => {
            println!("{indent}Raw(elem_size={elem_size}, count={count}, bytes={})", data.len());
        }
        LayoutNode::BitPack {
            bit_width,
            offset,
            count,
            values_buf,
            validity,
            all_null,
        } => {
            println!(
                "{indent}BitPack(bit_width={bit_width}, offset={offset}, count={count}, bytes={}, validity={}, all_null={all_null})",
                values_buf.len(),
                validity.as_ref().map_or("none", |_| "per-row")
            );
        }
        LayoutNode::FrameOfReference { reference, inner } => {
            println!("{indent}FrameOfReference(reference={reference})");
            print_node(inner, depth + 1);
        }
        LayoutNode::Dictionary { codes, values } => {
            println!("{indent}Dictionary");
            println!("{indent}  codes:");
            print_node(codes, depth + 2);
            println!("{indent}  values:");
            print_node(values, depth + 2);
        }
        LayoutNode::RunEnd {
            run_ends,
            values,
            count,
        } => {
            println!("{indent}RunEnd(count={count})");
            println!("{indent}  run_ends:");
            print_node(run_ends, depth + 2);
            println!("{indent}  values:");
            print_node(values, depth + 2);
        }
        LayoutNode::KernelEscape {
            kernel_id,
            params,
            count,
        } => {
            println!(
                "{indent}KernelEscape(kernel_id={kernel_id}, count={count}, params_bytes={})",
                params.len()
            );
        }
    }
}

fn data_type_name(data_type: &DataType) -> &'static str {
    match data_type {
        DataType::Boolean => "Boolean",
        DataType::Int32 => "Int32",
        DataType::Int64 => "Int64",
        DataType::Utf8 => "Utf8",
        _ => "Unsupported",
    }
}

fn display_decode_error(err: LoomDecodeError) -> String {
    err.to_string()
}
