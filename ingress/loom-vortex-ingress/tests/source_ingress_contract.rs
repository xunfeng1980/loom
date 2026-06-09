use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnosticCode, SourceDiagnosticFamily,
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressStatus, SourceLoweringDisposition,
};
use loom_vortex_ingress::{
    emit_supported_lmc1_from_vortex_buffer, inspect_vortex_buffer, reader_facts_from_vortex_buffer,
    source_coverage_from_vortex_coverage, source_diagnostic_from_vortex_ingress_diagnostic,
    source_diagnostic_from_vortex_reader_diagnostic, source_facts_from_vortex_buffer,
    source_ingress_report_from_vortex_buffer, source_report_from_vortex_reader_facts,
    VortexIngressDiagnostic, VortexIngressDiagnosticCode, VortexIngressStatus,
    VortexReaderDiagnostic, VortexReaderDiagnosticCode, VortexReaderEmissionKind,
    VortexReaderSupport,
};
use vortex_array::arrays::{StructArray, VarBinArray};
use vortex_array::dtype::{DType, FieldNames, Nullability};
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

fn supported_table_bytes() -> Vec<u8> {
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
fn supported_non_null_primitive_maps_to_source_contract() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);
    let vortex = reader_facts_from_vortex_buffer(&bytes).expect("Vortex reader facts");
    let source = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let coverage = source.coverage.as_ref().expect("source coverage");

    assert_eq!(vortex.support, VortexReaderSupport::Accepted);
    assert_eq!(vortex.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert_eq!(source.row_count, 3);
    assert_eq!(source.identity.source_kind, "buffer");
    assert_eq!(source.identity.format, "external-source");
    assert_eq!(
        source
            .root_schema
            .as_ref()
            .expect("root schema")
            .logical_kind,
        "primitive"
    );
    assert_eq!(coverage.support, SourceIngressStatus::Accepted);
    assert_eq!(coverage.emission_kind, SourceEmissionKind::Lmp1);
    assert_eq!(
        coverage.emission_disposition,
        SourceEmissionDisposition::CanonicalRaw
    );
    assert_eq!(
        coverage.lowering_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported
    );
}

#[test]
fn supported_primitive_struct_maps_to_table_contract() {
    let bytes = supported_table_bytes();
    let source = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let coverage = source.coverage.as_ref().expect("source coverage");
    let root = source.root_schema.as_ref().expect("root schema");

    assert_eq!(source.row_count, 3);
    assert_eq!(root.logical_kind, "struct");
    assert_eq!(root.field_names, vec!["id", "score"]);
    assert_eq!(coverage.support, SourceIngressStatus::Accepted);
    assert_eq!(coverage.emission_kind, SourceEmissionKind::Lmt1);
    assert_eq!(
        coverage.emission_disposition,
        SourceEmissionDisposition::CanonicalTable
    );
    assert_eq!(
        coverage.lowering_disposition,
        SourceLoweringDisposition::ProductionLoweringSupported
    );
}

#[test]
fn unsupported_utf8_maps_to_fail_closed_source_contract() {
    let rows = [Some("a"), Some("b"), Some("c")];
    let bytes = vortex_file_bytes(VarBinArray::from_iter(
        rows,
        DType::Utf8(Nullability::Nullable),
    ));
    let source = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let coverage = source.coverage.as_ref().expect("source coverage");

    assert_eq!(source.row_count, 3);
    assert_eq!(
        source
            .root_schema
            .as_ref()
            .expect("root schema")
            .logical_kind,
        "utf8"
    );
    assert_eq!(coverage.support, SourceIngressStatus::Unsupported);
    assert_eq!(coverage.emission_kind, SourceEmissionKind::None);
    assert_eq!(
        coverage.emission_disposition,
        SourceEmissionDisposition::None
    );
    assert_eq!(
        coverage.lowering_disposition,
        SourceLoweringDisposition::FailClosedDeferred
    );
}

#[test]
fn diagnostic_mapping_preserves_neutral_family_and_source_detail() {
    let reader_cases = [
        (
            VortexReaderDiagnosticCode::OpenFailed,
            SourceDiagnosticCode::OpenFailed,
            SourceDiagnosticFamily::Open,
        ),
        (
            VortexReaderDiagnosticCode::SplitUnavailable,
            SourceDiagnosticCode::SplitUnavailable,
            SourceDiagnosticFamily::Layout,
        ),
        (
            VortexReaderDiagnosticCode::TraversalFailed,
            SourceDiagnosticCode::LayoutUnavailable,
            SourceDiagnosticFamily::Layout,
        ),
        (
            VortexReaderDiagnosticCode::UnsupportedLayout,
            SourceDiagnosticCode::UnsupportedLayout,
            SourceDiagnosticFamily::Layout,
        ),
        (
            VortexReaderDiagnosticCode::UnsupportedDType,
            SourceDiagnosticCode::UnsupportedSchema,
            SourceDiagnosticFamily::Schema,
        ),
        (
            VortexReaderDiagnosticCode::UnsupportedConversion,
            SourceDiagnosticCode::UnsupportedConversion,
            SourceDiagnosticFamily::Conversion,
        ),
        (
            VortexReaderDiagnosticCode::VerificationRequired,
            SourceDiagnosticCode::VerificationFailed,
            SourceDiagnosticFamily::Verification,
        ),
    ];

    for (vortex_code, source_code, family) in reader_cases {
        let source = source_diagnostic_from_vortex_reader_diagnostic(&VortexReaderDiagnostic::new(
            vortex_code,
            "$.reader",
            "reader diagnostic",
        ));
        assert_eq!(source.code, source_code);
        assert_eq!(source.family, family);
        assert_eq!(source.path, "$.reader");
        assert_eq!(source.message, "reader diagnostic");
        assert_eq!(source.source_detail.as_deref(), Some(vortex_code.as_str()));
    }

    let ingress = source_diagnostic_from_vortex_ingress_diagnostic(&VortexIngressDiagnostic::new(
        VortexIngressDiagnosticCode::UnsupportedConversion,
        "$.payload",
        "conversion diagnostic",
    ));
    assert_eq!(ingress.code, SourceDiagnosticCode::UnsupportedConversion);
    assert_eq!(ingress.family, SourceDiagnosticFamily::Conversion);
    assert_eq!(ingress.path, "$.payload");
    assert_eq!(ingress.message, "conversion diagnostic");
    assert_eq!(
        ingress.source_detail.as_deref(),
        Some(VortexIngressDiagnosticCode::UnsupportedConversion.as_str())
    );
}

#[test]
fn malformed_buffer_maps_to_rejected_source_report_without_facts() {
    let report = source_ingress_report_from_vortex_buffer(b"not a vortex file");

    assert_eq!(report.status, SourceIngressStatus::Rejected);
    assert!(report.facts.is_none());
    assert_eq!(report.emission_kind, SourceEmissionKind::None);
    assert_eq!(report.emission_disposition, SourceEmissionDisposition::None);
    assert_eq!(
        report.artifact_verification,
        SourceArtifactVerificationSummary::not_applicable()
    );
    assert!(report.oracle_evidence.is_none());
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, SourceDiagnosticCode::OpenFailed);
    assert_eq!(report.diagnostics[0].family, SourceDiagnosticFamily::Open);
}

#[test]
fn materializable_vortex_source_report_is_arrow_semantic_accepted() {
    let rows = [Some("a"), Some("b"), Some("c")];
    let bytes = vortex_file_bytes(VarBinArray::from_iter(
        rows,
        DType::Utf8(Nullability::Nullable),
    ));
    let report = source_ingress_report_from_vortex_buffer(&bytes);

    assert_eq!(report.status, SourceIngressStatus::Accepted);
    assert!(report.facts.is_some());
    assert_eq!(report.emission_kind, SourceEmissionKind::ArrowSemantic);
    assert_eq!(
        report.emission_disposition,
        SourceEmissionDisposition::SemanticArrow
    );
    assert_eq!(
        report.lowering_disposition,
        SourceLoweringDisposition::InterpreterOnly
    );
    assert!(report.artifact_verification.required);
    assert!(report.artifact_verification.accepted);
    assert!(report.oracle_evidence.is_some());
}

#[test]
fn old_vortex_api_and_new_source_helpers_compile_together() {
    let bytes = vortex_file_bytes(buffer![7i32, -1, 42]);

    let legacy_inspect = inspect_vortex_buffer(&bytes);
    let legacy_facts = reader_facts_from_vortex_buffer(&bytes).expect("legacy reader facts");
    let legacy_artifact =
        emit_supported_lmc1_from_vortex_buffer(&bytes).expect("legacy artifact emission");

    let source_facts = source_facts_from_vortex_buffer(&bytes).expect("source facts");
    let source_report = source_report_from_vortex_reader_facts(&legacy_facts);
    let source_coverage = source_coverage_from_vortex_coverage(&legacy_facts.coverage);
    let source_diagnostic =
        source_diagnostic_from_vortex_reader_diagnostic(&VortexReaderDiagnostic::new(
            VortexReaderDiagnosticCode::VerificationRequired,
            "$.verification",
            "verification required",
        ));
    let ingress_diagnostic =
        source_diagnostic_from_vortex_ingress_diagnostic(&VortexIngressDiagnostic::new(
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            "conversion unsupported",
        ));

    assert_eq!(legacy_inspect.status, VortexIngressStatus::Accepted);
    assert_eq!(legacy_facts.support, VortexReaderSupport::Accepted);
    assert_eq!(legacy_facts.emission_kind, VortexReaderEmissionKind::Lmp1);
    assert!(!legacy_artifact.is_empty());
    assert_eq!(source_facts.row_count, legacy_facts.row_count);
    assert_eq!(source_report.status, SourceIngressStatus::Accepted);
    assert_eq!(source_coverage.emission_kind, SourceEmissionKind::Lmp1);
    assert_eq!(
        source_diagnostic.code,
        SourceDiagnosticCode::VerificationFailed
    );
    assert_eq!(
        ingress_diagnostic.code,
        SourceDiagnosticCode::UnsupportedConversion
    );
}

#[test]
fn generic_contract_sources_remain_source_neutral() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let output = Command::new("rg")
        .args([
            "-n",
            "Vortex|vortex",
            "ingress/loom-source-ingress/src",
            "ingress/loom-source-ingress/tests",
        ])
        .current_dir(&workspace_root)
        .output()
        .expect("run rg source-neutral guard");

    assert_eq!(
        output.status.code(),
        Some(1),
        "generic source-ingress crate must not contain source-specific vocabulary:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
