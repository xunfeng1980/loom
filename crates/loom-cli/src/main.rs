use std::env;
use std::fs;
use std::path::Path;

use arrow::array::{
    Array, BooleanArray, Float32Array, Float64Array, Int32Array, Int64Array, StringArray,
};
use arrow_schema::DataType;
use loom_core::alp_params::AlpParams;
use loom_core::artifact_verifier::{
    verify_artifact, ArtifactVerificationReport, ArtifactVerificationStatus,
};
use loom_core::container_codec::{
    decode_container, decode_layout_payload_maybe_container, decode_table_payload_maybe_container,
    extract_wrapped_payload, feature_names, is_container_payload, unknown_feature_bits,
    ContainerDescription, SectionKind, WrappedPayload,
};
use loom_core::descriptor::{from_descriptor_text, payload_to_descriptor_text};
use loom_core::error::LoomDecodeError;
use loom_core::fsst_params::FsstParams;
use loom_core::full_verifier::verify_l2_core;
use loom_core::l1_model::{decode_layout_to_array_data, LayoutDescription, LayoutNode};
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr,
};
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::layout_codec::decode_layout_payload;
use loom_core::table_codec::{decode_table_payload, decode_table_to_array_data, is_table_payload};
use loom_core::verifier::{verify_container, verify_layout, verify_table, VerificationReport};
use loom_vortex_ingress::{
    inspect_vortex_path, reader_facts_from_vortex_path,
    source_facts_from_vortex_buffer, VortexIngressReport, VortexReaderEmissionKind,
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
        "inspect" => {
            let input = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            inspect(Path::new(&input))
        }
        "decode" => {
            let input = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            decode(Path::new(&input))
        }
        "verify-l2core" => {
            let mode = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            verify_l2core(&mode)
        }
        "verify-artifact" => verify_artifact_cli(args.collect()),
        "ingest-vortex" => {
            let mode = args.next().ok_or_else(usage)?;
            ingest_vortex(&mode, args.collect())
        }
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n{}", usage())),
    }
}

fn usage() -> String {
    "usage: loom <inspect|decode> <payload-or-descriptor>\n       loom verify-artifact <artifact.loom>\n       loom verify-l2core --sample\n       loom ingest-vortex --inspect <input.vortex>\n       loom ingest-vortex --emit-loom <input.vortex> <output.loom>"
        .to_string()
}

fn verify_artifact_cli(args: Vec<String>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }

    let mut input = None;
    for arg in args {
        match arg.as_str() {
            _ if input.is_none() => input = Some(arg),
            _ => return Err(usage()),
        }
    }

    let input = input.ok_or_else(usage)?;
    let path = Path::new(&input);
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(&bytes, &registry, &Default::default());

    println!("input: {}", path.display());
    println!("artifact_verification_mode: structural");
    print_artifact_verification_report(&report);
    if report.status() == ArtifactVerificationStatus::Rejected {
        return Err("artifact verification failed".to_string());
    }
    Ok(())
}

fn ingest_vortex(mode: &str, args: Vec<String>) -> Result<(), String> {
    match mode {
        "--inspect" => {
            if args.len() != 1 {
                return Err(usage());
            }
            let path = Path::new(&args[0]);
            let report = inspect_vortex_path(path);
            println!("input: {}", path.display());
            print_vortex_ingress_report(&report);
            if let Ok(reader_facts) = reader_facts_from_vortex_path(path) {
                println!("reader_support: {}", reader_facts.support.as_str());
                println!("emission_kind: {}", reader_facts.emission_kind.as_str());
                println!("reader_layout_facts: {}", reader_facts.layout_facts.len());
                println!("reader_dtype_facts: {}", reader_facts.dtype_facts.len());
                println!("reader_segment_facts: {}", reader_facts.segment_facts.len());
                println!("reader_split_facts: {}", reader_facts.split_facts.len());
                println!("reader_diagnostics: {}", reader_facts.diagnostics.len());
                print_reader_artifact_verification(path, reader_facts.emission_kind)?;
            }
            if report.status.as_str() == "rejected" {
                return Err("Vortex ingress rejected input".to_string());
            }
            Ok(())
        }
        "--emit-loom" => {
            if args.len() != 2 {
                return Err(usage());
            }
            let input = Path::new(&args[0]);
            let _output = Path::new(&args[1]);
            let bytes =
                fs::read(input).map_err(|err| format!("read {}: {err}", input.display()))?;
            match source_facts_from_vortex_buffer(&bytes) {
                Ok(facts) => {
                    println!("input: {}", input.display());
                    println!("status: ingress_ok");
                    println!("format: {}", facts.identity.format);
                    println!("source_kind: {}", facts.identity.source_kind);
                    println!("row_count: {}", facts.row_count);
                    Ok(())
                }
                Err(report) => {
                    println!("input: {}", input.display());
                    println!("status: {}", report.status.as_str());
                    Err("Vortex file is not in the supported Loom ingress slice".to_string())
                }
            }
        }
        "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn print_reader_artifact_verification(
    path: &Path,
    emission_kind: VortexReaderEmissionKind,
) -> Result<(), String> {
    if emission_kind == VortexReaderEmissionKind::None {
        println!("reader_artifact_verification: not_applicable");
        return Ok(());
    }

    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    match source_facts_from_vortex_buffer(&bytes) {
        Ok(_) => {
            println!("reader_artifact_verification: pass");
        }
        Err(_) => {
            println!("reader_artifact_verification: fail");
        }
    }
    Ok(())
}

fn verify_l2core(mode: &str) -> Result<(), String> {
    if mode != "--sample" {
        return Err(usage());
    }

    let program = sample_l2core_program();
    let report = verify_l2_core(&program);
    if report.is_ok() {
        println!("full_verification: pass");
    } else {
        println!("full_verification: fail");
        for diagnostic in report.diagnostics() {
            println!(
                "diagnostic: code={} path={} message={}",
                diagnostic.code, diagnostic.path, diagnostic.message
            );
        }
        return Err("L2Core verification failed".to_string());
    }

    println!("proof_obligations:");
    for obligation in report.proof_obligations() {
        println!(
            "  {} layer={} constraints={}",
            obligation.id,
            obligation.layer,
            obligation.constraint_ids.len()
        );
    }

    if let Some(facts) = report.facts() {
        println!("facts: present");
        println!("row_count_bound: {}", facts.row_count_bound.unwrap_or(0));
        println!("input_ranges: {}", facts.input_ranges.len());
        println!("output_schema: {}", facts.output_schema.len());
        println!("constraint_ids: {}", facts.constraint_ids.len());
    }
    print!("{}", report.constraint_comments());
    Ok(())
}

fn sample_l2core_program() -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input0".to_string(),
                offset: 0,
                length: 16,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "out0".to_string(),
                arrow_type: DataType::Int32,
                nullable: true,
                max_events: 4,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(4),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::u64(0),
            end: ScalarExpr::u64(4),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: "input0".to_string(),
                    offset: ScalarExpr::Add(
                        Box::new(ScalarExpr::var("i")),
                        Box::new(ScalarExpr::u64(0)),
                    ),
                    width: ScalarExpr::u64(4),
                    bind: "value".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: "out0".to_string(),
                    value: ScalarExpr::var("value"),
                },
            ],
        }],
    }
}

fn inspect(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    if is_container_payload(&bytes) {
        return inspect_container(path, &bytes);
    }

    let registry = L2KernelRegistry::default_for_mvp0();
    if is_table_payload(&bytes) {
        let table = decode_table_payload(&bytes).map_err(display_decode_error)?;
        println!("input: {}", path.display());
        let report = verify_table(&table, &registry);
        print_verification(&report);
        if !report.is_ok() {
            return Err("verification failed".to_string());
        }
        println!("table_row_count: {}", table.row_count);
        println!("columns:");
        for column in &table.columns {
            println!(
                "  {}: {} rows={}",
                column.name,
                data_type_name(&column.layout.data_type),
                column.layout.row_count
            );
            print_node(&column.layout.root, 2, &column.layout.data_type);
        }
        return Ok(());
    }
    let desc = load_layout(&bytes)?;
    println!("input: {}", path.display());
    let report = verify_layout(&desc, &registry);
    print_verification(&report);
    if !report.is_ok() {
        return Err("verification failed".to_string());
    }
    println!("data_type: {}", data_type_name(&desc.data_type));
    println!("row_count: {}", desc.row_count);
    println!("layout:");
    print_node(&desc.root, 1, &desc.data_type);
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
    if is_table_payload(&bytes) || container_wraps_table(&bytes)? {
        return decode_table(&bytes);
    }
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
        DataType::Float32 => {
            let array = Float32Array::from(data);
            for row in 0..array.len() {
                if array.is_null(row) {
                    println!("NULL");
                } else {
                    println!("{}", array.value(row));
                }
            }
        }
        DataType::Float64 => {
            let array = Float64Array::from(data);
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

fn decode_table(bytes: &[u8]) -> Result<(), String> {
    let table = decode_table_payload_maybe_container(bytes).map_err(display_decode_error)?;
    let registry = L2KernelRegistry::default_for_mvp0();
    let arrays = decode_table_to_array_data(&table, &registry).map_err(display_decode_error)?;

    for (i, column) in table.columns.iter().enumerate() {
        if i > 0 {
            print!("\t");
        }
        print!("{}", column.name);
    }
    println!();

    for row in 0..table.row_count {
        for (col_idx, column) in table.columns.iter().enumerate() {
            if col_idx > 0 {
                print!("\t");
            }
            print_cell(&arrays[col_idx], &column.layout.data_type, row)?;
        }
        println!();
    }
    Ok(())
}

fn print_cell(
    data: &arrow_data::ArrayData,
    data_type: &DataType,
    row: usize,
) -> Result<(), String> {
    match data_type {
        DataType::Boolean => {
            let array = BooleanArray::from(data.clone());
            if array.is_null(row) {
                print!("NULL");
            } else {
                print!("{}", array.value(row));
            }
        }
        DataType::Int32 => {
            let array = Int32Array::from(data.clone());
            if array.is_null(row) {
                print!("NULL");
            } else {
                print!("{}", array.value(row));
            }
        }
        DataType::Int64 => {
            let array = Int64Array::from(data.clone());
            if array.is_null(row) {
                print!("NULL");
            } else {
                print!("{}", array.value(row));
            }
        }
        DataType::Utf8 => {
            let array = StringArray::from(data.clone());
            if array.is_null(row) {
                print!("NULL");
            } else {
                print!("{}", array.value(row));
            }
        }
        DataType::Float32 => {
            let array = Float32Array::from(data.clone());
            if array.is_null(row) {
                print!("NULL");
            } else {
                print!("{}", array.value(row));
            }
        }
        DataType::Float64 => {
            let array = Float64Array::from(data.clone());
            if array.is_null(row) {
                print!("NULL");
            } else {
                print!("{}", array.value(row));
            }
        }
        other => return Err(format!("unsupported table output type {other:?}")),
    }
    Ok(())
}

fn load_layout(bytes: &[u8]) -> Result<LayoutDescription, String> {
    if is_binary_payload(bytes) {
        return decode_layout_payload_maybe_container(bytes).map_err(display_decode_error);
    }
    let input = std::str::from_utf8(bytes)
        .map_err(|err| format!("input is neither LMP1 payload nor UTF-8 descriptor: {err}"))?;
    from_descriptor_text(input).map_err(display_decode_error)
}

fn is_binary_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(b"LMP1") || is_container_payload(bytes)
}

fn container_wraps_table(bytes: &[u8]) -> Result<bool, String> {
    if !is_container_payload(bytes) {
        return Ok(false);
    }
    match extract_wrapped_payload(bytes).map_err(display_decode_error)? {
        WrappedPayload::Layout(_) => Ok(false),
        WrappedPayload::Table(_) => Ok(true),
    }
}

fn inspect_container(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let container = decode_container(bytes).map_err(display_decode_error)?;
    println!("input: {}", path.display());
    print_container_summary(&container);

    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_container(bytes, &registry);
    print_verification(&report);
    if !report.is_ok() {
        return Err("verification failed".to_string());
    }

    match extract_wrapped_payload(bytes).map_err(display_decode_error)? {
        WrappedPayload::Layout(payload) => {
            println!("payload_kind: LMP1 layout");
            let desc = decode_layout_payload(&payload).map_err(display_decode_error)?;
            println!("data_type: {}", data_type_name(&desc.data_type));
            println!("row_count: {}", desc.row_count);
            println!("layout:");
            print_node(&desc.root, 1, &desc.data_type);
            println!("descriptor:");
            let text =
                loom_core::descriptor::to_descriptor_text(&desc).map_err(display_decode_error)?;
            println!("{text}");
        }
        WrappedPayload::Table(payload) => {
            println!("payload_kind: LMT1 table");
            let table = decode_table_payload(&payload).map_err(display_decode_error)?;
            println!("table_row_count: {}", table.row_count);
            println!("columns:");
            for column in &table.columns {
                println!(
                    "  {}: {} rows={}",
                    column.name,
                    data_type_name(&column.layout.data_type),
                    column.layout.row_count
                );
                print_node(&column.layout.root, 2, &column.layout.data_type);
            }
        }
    }
    Ok(())
}

fn print_vortex_ingress_report(report: &VortexIngressReport) {
    println!("ingress: vortex");
    println!("status: {}", report.status.as_str());
    if let Some(facts) = &report.facts {
        println!("source: {}", facts.source_kind.as_str());
        println!("vortex_file_version: {}", facts.vortex_file_version);
        println!("row_count: {}", facts.row_count);
        println!("dtype: {}", facts.dtype_summary);
        println!("layout: {}", facts.layout_summary);
        println!("segments: {}", facts.segment_count);
        println!("segment_ranges:");
        for (start, end) in &facts.segment_ranges {
            println!("  {start}..{end}");
        }
        println!("statistics_present: {}", facts.statistics_present);
        println!(
            "footer_approx_byte_size: {}",
            facts
                .footer_approx_byte_size
                .map_or_else(|| "unknown".to_string(), |size| size.to_string())
        );
        println!("supported_loom_payload: {}", facts.supported_loom_payload);
    } else {
        println!("facts: none");
    }
    if report.diagnostics.is_empty() {
        println!("diagnostics: none");
    } else {
        println!("diagnostics:");
        for diagnostic in &report.diagnostics {
            println!(
                "  code={} path={} message={}",
                diagnostic.code, diagnostic.path, diagnostic.message
            );
        }
    }
}

fn print_container_summary(container: &ContainerDescription) {
    println!("container: LMC1");
    println!("container_version: {}", container.version);
    println!(
        "required_features: {}",
        feature_list(container.required_features)
    );
    println!(
        "optional_features: {}",
        feature_list(container.optional_features)
    );
    println!("section_count: {}", container.sections.len());
    println!("sections:");
    for (idx, section) in container.sections.iter().enumerate() {
        println!(
            "  [{idx}] kind={} tag={} required={} offset={} length={}",
            section_kind_name(section.kind),
            section.kind.tag(),
            section.is_required(),
            section.offset,
            section.bytes.len()
        );
    }
    println!(
        "trailer: {}",
        if container.has_trailer {
            "present"
        } else {
            "none"
        }
    );
}

fn feature_list(bits: u64) -> String {
    let mut names = feature_names(bits);
    let unknown = unknown_feature_bits(bits);
    if unknown != 0 {
        names.push("unknown");
    }
    if names.is_empty() {
        "none".to_string()
    } else if unknown == 0 {
        names.join(",")
    } else {
        format!("{} (unknown_bits=0x{unknown:016x})", names.join(","))
    }
}

fn section_kind_name(kind: SectionKind) -> &'static str {
    kind.as_str()
}

fn print_node(node: &LayoutNode, depth: usize, data_type: &DataType) {
    let indent = "  ".repeat(depth);
    match node {
        LayoutNode::Raw {
            elem_size,
            count,
            data,
        } => {
            println!(
                "{indent}Raw(elem_size={elem_size}, count={count}, bytes={})",
                data.len()
            );
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
            print_node(inner, depth + 1, data_type);
        }
        LayoutNode::Dictionary { codes, values } => {
            println!("{indent}Dictionary");
            println!("{indent}  codes:");
            print_node(codes, depth + 2, data_type);
            println!("{indent}  values:");
            print_node(values, depth + 2, data_type);
        }
        LayoutNode::RunEnd {
            run_ends,
            values,
            count,
        } => {
            println!("{indent}RunEnd(count={count})");
            println!("{indent}  run_ends:");
            print_node(run_ends, depth + 2, data_type);
            println!("{indent}  values:");
            print_node(values, depth + 2, data_type);
        }
        LayoutNode::KernelEscape {
            kernel_id,
            params,
            count,
        } => {
            println!(
                "{}",
                kernel_escape_summary(&indent, *kernel_id, params, *count, data_type)
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
        DataType::Float32 => "Float32",
        DataType::Float64 => "Float64",
        _ => "Unsupported",
    }
}

fn kernel_escape_summary(
    indent: &str,
    kernel_id: u32,
    params: &[u8],
    count: usize,
    data_type: &DataType,
) -> String {
    match kernel_id {
        0 => match FsstParams::decode(params, count) {
            Ok(decoded) => format!(
                "{indent}KernelEscape(kernel=fsst, kernel_id=0, output_type={}, count={count}, params=symbols={}, codes_bytes={}, validity={}, params_bytes={})",
                data_type_name(data_type),
                decoded.symbol_lengths.len(),
                decoded.codes_bytes.len(),
                decoded.validity.as_ref().map_or("none", |_| "present"),
                params.len()
            ),
            Err(err) => format!(
                "{indent}KernelEscape(kernel=fsst, kernel_id=0, output_type={}, count={count}, params=malformed:{err}, params_bytes={})",
                data_type_name(data_type),
                params.len()
            ),
        },
        1 => match AlpParams::decode(params, count) {
            Ok(decoded) => format!(
                "{indent}KernelEscape(kernel=alp, kernel_id=1, output_type={}, count={count}, params=output_type={}, row_count={}, exponent={}, value_count={}, validity={}, params_bytes={})",
                data_type_name(data_type),
                decoded.output_type.as_str(),
                decoded.mantissas.len(),
                decoded.decimal_exponent,
                decoded.mantissas.len(),
                decoded.validity.as_ref().map_or("none", |_| "present"),
                params.len()
            ),
            Err(err) => format!(
                "{indent}KernelEscape(kernel=alp, kernel_id=1, output_type={}, count={count}, params=malformed:{err}, params_bytes={})",
                data_type_name(data_type),
                params.len()
            ),
        },
        _ => format!(
            "{indent}KernelEscape(kernel=unknown, kernel_id={kernel_id}, output_type={}, count={count}, params_bytes={})",
            data_type_name(data_type),
            params.len()
        ),
    }
}

fn display_decode_error(err: LoomDecodeError) -> String {
    err.to_string()
}

fn print_artifact_verification_report(report: &ArtifactVerificationReport) {
    let status = match report.status() {
        ArtifactVerificationStatus::Accepted => "pass",
        ArtifactVerificationStatus::Rejected => "fail",
        ArtifactVerificationStatus::Unsupported => "unsupported",
    };
    println!("artifact_verification: {status}");

    if let Some(facts) = report.facts() {
        println!("facts: present");
        println!("artifact: {}", facts.artifact_kind);
        println!("artifact_kind: {}", facts.artifact_kind);
        println!(
            "container_version: {}",
            facts
                .container_version
                .map_or_else(|| "unknown".to_string(), |version| version.to_string())
        );
        println!(
            "required_features: {}",
            if facts.required_features.is_empty() {
                "none".to_string()
            } else {
                facts.required_features.join(",")
            }
        );
        println!(
            "optional_features: {}",
            if facts.optional_features.is_empty() {
                "none".to_string()
            } else {
                facts.optional_features.join(",")
            }
        );
        println!(
            "payload_kind: {}",
            facts.payload_kind.as_deref().unwrap_or("unknown")
        );
        println!(
            "payload: {}",
            facts.payload_kind.as_deref().unwrap_or("unknown")
        );
        println!(
            "row_count_bound: {}",
            facts
                .row_count_bound
                .map_or_else(|| "unknown".to_string(), |rows| rows.to_string())
        );
        println!(
            "constraint_status: {}",
            if facts.constraints_discharged {
                "discharged"
            } else {
                "collected"
            }
        );
        println!("lowering_ready: {}", facts.lowering_ready.ready);
        println!(
            "lowering_backend: {}",
            facts.lowering_ready.backend.as_deref().unwrap_or("none")
        );
        if facts.lowering_ready.diagnostics.is_empty() {
            println!("lowering_diagnostics: none");
        } else {
            println!("lowering_diagnostics:");
            for diagnostic in &facts.lowering_ready.diagnostics {
                println!(
                    "lowering_diagnostic: code={} path={} message={}",
                    diagnostic.code, diagnostic.path, diagnostic.message
                );
            }
        }
        println!(
            "production_discharge_ready: {}",
            facts.lowering_ready.ready
        );
    } else {
        println!("facts: none");
        println!("constraint_status: none");
        println!("production_discharge_ready: false");
        println!("lowering_ready: false");
        println!("lowering_backend: none");
        println!("lowering_diagnostics: none");
    }

    if report.diagnostics().is_empty() {
        println!("diagnostics: none");
    } else {
        println!("diagnostics:");
        for diagnostic in report.diagnostics() {
            println!(
                "diagnostic: stage={} code={} path={} message={}",
                diagnostic.stage.as_str(),
                diagnostic.code,
                diagnostic.path,
                diagnostic.message
            );
        }
    }
}

fn print_verification(report: &VerificationReport) {
    if report.is_ok() {
        println!("verification: pass");
        return;
    }

    println!("verification: fail");
    println!("diagnostics:");
    for diagnostic in report.diagnostics() {
        println!(
            "  - code={} path={} message={}",
            diagnostic.code, diagnostic.path, diagnostic.message
        );
    }
}
