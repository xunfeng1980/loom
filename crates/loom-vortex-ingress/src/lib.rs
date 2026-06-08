//! Isolated real Vortex file ingress boundary.
//!
//! This crate is the only workspace crate that may depend on `vortex-file`.
//! It translates real Vortex file/container metadata into Loom-owned facts and
//! diagnostics so `loom-core` and `loom-ffi` remain Vortex-free.

pub mod source_contract;
pub use source_contract::{
    source_facts_from_vortex_buffer, source_facts_from_vortex_path,
    source_facts_from_vortex_reader_facts, source_ingress_report_from_vortex_buffer,
    source_ingress_report_from_vortex_path, source_report_from_vortex_ingress_report,
    source_report_from_vortex_reader_facts,
};

use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use arrow_schema::DataType;
use loom_core::container_codec::{wrap_layout_payload, wrap_table_payload};
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use loom_core::table_codec::{encode_table_payload, TableColumn, TableDescription};
use vortex_array::arrays::primitive::PrimitiveArrayExt;
use vortex_array::arrays::struct_::StructArrayExt;
use vortex_array::arrays::PrimitiveArray;
use vortex_array::arrays::StructArray;
use vortex_array::dtype::{DType, Nullability, PType};
use vortex_array::memory::MemorySession;
use vortex_array::scalar_fn::session::ScalarFnSession;
use vortex_array::session::ArraySession;
use vortex_array::stream::ArrayStreamExt;
use vortex_array::validity::Validity;
use vortex_array::VortexSessionExecute;
use vortex_buffer::ByteBuffer;
use vortex_file::{OpenOptionsSessionExt, VortexFile};
use vortex_io::runtime::current::CurrentThreadRuntime;
use vortex_io::runtime::BlockingRuntime;
use vortex_io::session::RuntimeSession;
use vortex_io::session::RuntimeSessionExt;
use vortex_layout::session::LayoutSession;
use vortex_session::VortexSession;

static RUNTIME: LazyLock<CurrentThreadRuntime> = LazyLock::new(CurrentThreadRuntime::new);

#[derive(Clone, Debug, Eq, PartialEq)]
struct SingleColumnSupport {
    ptype: PType,
    data_type: DataType,
    elem_size: u8,
}

/// High-level classification of an ingress attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexIngressStatus {
    /// The file was opened and the shape is supported by the current Loom
    /// ingress slice.
    Accepted,
    /// The file was opened, but the current Loom ingress slice cannot convert
    /// it to a Loom payload.
    Unsupported,
    /// The file or buffer could not be interpreted as a valid Vortex file.
    Rejected,
}

impl VortexIngressStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}

/// Stable diagnostic code for real Vortex ingress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexIngressDiagnosticCode {
    NotYetInspected,
    OpenFailed,
    UnsupportedLayout,
    UnsupportedDType,
    UnsupportedConversion,
}

impl VortexIngressDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotYetInspected => "INGRESS_NOT_YET_INSPECTED",
            Self::OpenFailed => "INGRESS_OPEN_FAILED",
            Self::UnsupportedLayout => "INGRESS_UNSUPPORTED_LAYOUT",
            Self::UnsupportedDType => "INGRESS_UNSUPPORTED_DTYPE",
            Self::UnsupportedConversion => "INGRESS_UNSUPPORTED_CONVERSION",
        }
    }
}

impl fmt::Display for VortexIngressDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Reviewer-visible diagnostic for ingress failures or unsupported shapes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexIngressDiagnostic {
    pub code: VortexIngressDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl VortexIngressDiagnostic {
    pub fn new(
        code: VortexIngressDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Source form inspected by the ingress bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexIngressSourceKind {
    Buffer,
    Path,
}

impl VortexIngressSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buffer => "buffer",
            Self::Path => "path",
        }
    }
}

/// Loom-owned facts extracted from a real Vortex file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexFileFacts {
    pub source_kind: VortexIngressSourceKind,
    pub vortex_file_version: u16,
    pub row_count: u64,
    pub dtype_summary: String,
    pub layout_summary: String,
    pub segment_count: usize,
    pub segment_ranges: Vec<(u64, u64)>,
    pub alignment_summary: Vec<String>,
    pub statistics_present: bool,
    pub footer_approx_byte_size: Option<usize>,
    pub supported_loom_payload: bool,
}

/// Complete-reader support classification for Phase 18 facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexReaderSupport {
    /// The reader facts and current Loom emission slice are accepted.
    Accepted,
    /// The file is valid Vortex, but the current Loom reader cannot emit it.
    Unsupported,
    /// The input cannot be opened as a valid Vortex file.
    Rejected,
}

impl VortexReaderSupport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Unsupported => "unsupported",
            Self::Rejected => "rejected",
        }
    }
}

/// Loom artifact kind the reader may emit after verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexReaderEmissionKind {
    /// No `.loom` bytes may be emitted for this input.
    None,
    /// A single-column layout payload wrapped in `LMC1`.
    Lmp1,
    /// A table payload wrapped in `LMC1`.
    Lmt1,
}

impl VortexReaderEmissionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lmp1 => "LMP1",
            Self::Lmt1 => "LMT1",
        }
    }
}

/// Semantic shape of emitted Loom artifacts for coverage reporting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexEmissionDisposition {
    None,
    CanonicalRaw,
    CanonicalTable,
    StructuredLayout,
}

impl VortexEmissionDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CanonicalRaw => "canonical-raw",
            Self::CanonicalTable => "canonical-table",
            Self::StructuredLayout => "structured-layout",
        }
    }
}

/// Native-lowering support classification for a reader-covered Vortex shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexLoweringDisposition {
    InterpreterOnly,
    ProductionLoweringSupported,
    FailClosedDeferred,
}

impl VortexLoweringDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InterpreterOnly => "interpreter-only",
            Self::ProductionLoweringSupported => "production-lowering-supported",
            Self::FailClosedDeferred => "fail-closed/deferred",
        }
    }
}

/// Phase 21 coverage facts. This separates reader support, artifact emission,
/// and native-lowering disposition for every inspected Vortex shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexEncodingCoverage {
    pub dtype_kind: String,
    pub nullable: Option<bool>,
    pub root_layout_encoding: String,
    pub layout_class: String,
    pub array_encoding: String,
    pub has_splits: bool,
    pub has_statistics: bool,
    pub reader_support: VortexReaderSupport,
    pub emission_kind: VortexReaderEmissionKind,
    pub emission_disposition: VortexEmissionDisposition,
    pub lowering_disposition: VortexLoweringDisposition,
    pub notes: Vec<String>,
}

/// Stable diagnostic code vocabulary for the complete-reader boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexReaderDiagnosticCode {
    OpenFailed,
    SplitUnavailable,
    TraversalFailed,
    UnsupportedLayout,
    UnsupportedDType,
    UnsupportedConversion,
    VerificationRequired,
}

impl VortexReaderDiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenFailed => "READER_OPEN_FAILED",
            Self::SplitUnavailable => "READER_SPLIT_UNAVAILABLE",
            Self::TraversalFailed => "READER_TRAVERSAL_FAILED",
            Self::UnsupportedLayout => "READER_UNSUPPORTED_LAYOUT",
            Self::UnsupportedDType => "READER_UNSUPPORTED_DTYPE",
            Self::UnsupportedConversion => "READER_UNSUPPORTED_CONVERSION",
            Self::VerificationRequired => "READER_VERIFICATION_REQUIRED",
        }
    }
}

/// Reviewer-visible diagnostic for non-fatal reader fact extraction gaps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexReaderDiagnostic {
    pub code: VortexReaderDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl VortexReaderDiagnostic {
    pub fn new(
        code: VortexReaderDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Loom-owned dtype fact. The strings are intentionally reviewer-facing
/// summaries; no public fact exposes a Vortex type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexReaderDTypeFact {
    pub path: String,
    pub summary: String,
    pub kind: String,
    pub nullable: Option<bool>,
    pub field_count: Option<usize>,
    pub field_names: Vec<String>,
}

/// Loom-owned layout fact for a node in the Vortex layout tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexReaderLayoutFact {
    pub path: String,
    pub encoding_id: String,
    pub dtype_summary: String,
    pub row_count: u64,
    pub child_count: usize,
    pub child_type: Option<String>,
    pub child_name: Option<String>,
    pub child_row_offset: Option<u64>,
    pub segment_ids: Vec<u32>,
    pub metadata_byte_len: usize,
}

/// Loom-owned segment byte-range fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexReaderSegmentFact {
    pub id: u32,
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub length: u64,
    pub alignment: String,
    pub ordered_after_previous: bool,
    pub overlaps_previous: bool,
}

/// Loom-owned split range fact produced from Vortex layout split discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexReaderSplitFact {
    pub index: usize,
    pub start_row: u64,
    pub end_row: u64,
    pub row_count: u64,
}

/// Rich Phase 18 facts extracted from a real Vortex file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexReaderFacts {
    pub source_kind: VortexIngressSourceKind,
    pub vortex_file_version: u16,
    pub row_count: u64,
    pub root_dtype: VortexReaderDTypeFact,
    pub root_layout_encoding: String,
    pub layout_facts: Vec<VortexReaderLayoutFact>,
    pub dtype_facts: Vec<VortexReaderDTypeFact>,
    pub segment_facts: Vec<VortexReaderSegmentFact>,
    pub split_facts: Vec<VortexReaderSplitFact>,
    pub statistics_present: bool,
    pub footer_approx_byte_size: Option<usize>,
    pub support: VortexReaderSupport,
    pub emission_kind: VortexReaderEmissionKind,
    pub coverage: VortexEncodingCoverage,
    pub diagnostics: Vec<VortexReaderDiagnostic>,
}

/// Result of inspecting or converting a real Vortex file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VortexIngressReport {
    pub status: VortexIngressStatus,
    pub facts: Option<VortexFileFacts>,
    pub diagnostics: Vec<VortexIngressDiagnostic>,
}

impl VortexIngressReport {
    pub fn accepted(facts: VortexFileFacts) -> Self {
        Self {
            status: VortexIngressStatus::Accepted,
            facts: Some(facts),
            diagnostics: Vec::new(),
        }
    }

    pub fn unsupported(
        facts: Option<VortexFileFacts>,
        code: VortexIngressDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: VortexIngressStatus::Unsupported,
            facts,
            diagnostics: vec![VortexIngressDiagnostic::new(code, path, message)],
        }
    }

    pub fn rejected(
        code: VortexIngressDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: VortexIngressStatus::Rejected,
            facts: None,
            diagnostics: vec![VortexIngressDiagnostic::new(code, path, message)],
        }
    }
}

fn default_session() -> VortexSession {
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

fn report_for_opened_file(
    file: &VortexFile,
    source_kind: VortexIngressSourceKind,
) -> VortexIngressReport {
    let mut facts = facts_from_file(file, source_kind);
    if reader_emission_kind(file) != VortexReaderEmissionKind::None {
        facts.supported_loom_payload = true;
        return VortexIngressReport::accepted(facts);
    }

    VortexIngressReport::unsupported(
        Some(facts),
        VortexIngressDiagnosticCode::UnsupportedConversion,
        "$.payload",
        "real Vortex file opened successfully, but Phase 15 conversion is not enabled for this layout",
    )
}

fn facts_from_file(file: &VortexFile, source_kind: VortexIngressSourceKind) -> VortexFileFacts {
    let footer = file.footer();
    let layout = footer.layout();
    let segment_map = footer.segment_map();
    let segment_ranges = segment_map
        .iter()
        .map(|segment| {
            let range = segment.byte_range();
            (range.start, range.end)
        })
        .collect();
    let alignment_summary = segment_map
        .iter()
        .map(|segment| format!("{:?}", segment.alignment))
        .collect();

    VortexFileFacts {
        source_kind,
        vortex_file_version: vortex_file::VERSION,
        row_count: file.row_count(),
        dtype_summary: format!("{:?}", file.dtype()),
        layout_summary: layout.encoding_id().as_ref().to_string(),
        segment_count: segment_map.len(),
        segment_ranges,
        alignment_summary,
        statistics_present: file.file_stats().is_some(),
        footer_approx_byte_size: footer.approx_byte_size(),
        supported_loom_payload: false,
    }
}

fn reader_facts_from_file(
    file: &VortexFile,
    source_kind: VortexIngressSourceKind,
) -> VortexReaderFacts {
    let footer = file.footer();
    let layout = footer.layout();
    let emission_kind = reader_emission_kind(file);
    let support = if emission_kind == VortexReaderEmissionKind::None {
        VortexReaderSupport::Unsupported
    } else {
        VortexReaderSupport::Accepted
    };

    let mut diagnostics = Vec::new();
    let mut layout_facts = Vec::new();
    collect_layout_facts(
        layout.clone(),
        "$".to_string(),
        None,
        &mut layout_facts,
        &mut diagnostics,
    );

    let dtype_facts = layout_facts
        .iter()
        .map(|layout| dtype_fact_from_summary(layout.path.clone(), layout.dtype_summary.clone()))
        .collect::<Vec<_>>();
    let root_dtype = dtype_fact_from_dtype("$", file.dtype());
    let split_facts = split_facts_from_file(file, &mut diagnostics);

    let coverage = coverage_from_reader_shape(
        &root_dtype,
        layout.encoding_id().as_ref(),
        split_facts.as_slice(),
        file.file_stats().is_some(),
        support,
        emission_kind,
    );

    VortexReaderFacts {
        source_kind,
        vortex_file_version: vortex_file::VERSION,
        row_count: file.row_count(),
        root_dtype,
        root_layout_encoding: layout.encoding_id().as_ref().to_string(),
        layout_facts,
        dtype_facts,
        segment_facts: segment_facts_from_file(file),
        split_facts,
        statistics_present: file.file_stats().is_some(),
        footer_approx_byte_size: footer.approx_byte_size(),
        support,
        emission_kind,
        coverage,
        diagnostics,
    }
}

fn coverage_from_reader_shape(
    root_dtype: &VortexReaderDTypeFact,
    root_layout_encoding: &str,
    split_facts: &[VortexReaderSplitFact],
    has_statistics: bool,
    reader_support: VortexReaderSupport,
    emission_kind: VortexReaderEmissionKind,
) -> VortexEncodingCoverage {
    let emission_disposition = match emission_kind {
        VortexReaderEmissionKind::None => VortexEmissionDisposition::None,
        VortexReaderEmissionKind::Lmp1 => VortexEmissionDisposition::CanonicalRaw,
        VortexReaderEmissionKind::Lmt1 => VortexEmissionDisposition::CanonicalTable,
    };
    let layout_class = layout_class(root_layout_encoding);
    let array_encoding = array_encoding(root_dtype, root_layout_encoding);
    let can_use_current_production_lowering = reader_support == VortexReaderSupport::Accepted
        && root_dtype.nullable == Some(false)
        && layout_class != "chunked"
        && matches!(array_encoding.as_str(), "primitive" | "struct")
        && matches!(
            emission_kind,
            VortexReaderEmissionKind::Lmp1 | VortexReaderEmissionKind::Lmt1
        );
    let lowering_disposition = match (reader_support, emission_kind) {
        (VortexReaderSupport::Accepted, VortexReaderEmissionKind::Lmp1)
        | (VortexReaderSupport::Accepted, VortexReaderEmissionKind::Lmt1)
            if can_use_current_production_lowering =>
        {
            VortexLoweringDisposition::ProductionLoweringSupported
        }
        (VortexReaderSupport::Accepted, VortexReaderEmissionKind::Lmp1)
        | (VortexReaderSupport::Accepted, VortexReaderEmissionKind::Lmt1) => {
            VortexLoweringDisposition::InterpreterOnly
        }
        (VortexReaderSupport::Accepted, VortexReaderEmissionKind::None)
        | (VortexReaderSupport::Unsupported, _)
        | (VortexReaderSupport::Rejected, _) => VortexLoweringDisposition::FailClosedDeferred,
    };
    let mut notes = Vec::new();
    if emission_disposition == VortexEmissionDisposition::CanonicalRaw
        || emission_disposition == VortexEmissionDisposition::CanonicalTable
    {
        notes.push(
            "canonicalized Vortex scan rows feed the current Loom raw/table artifact shape"
                .to_string(),
        );
    }
    if lowering_disposition == VortexLoweringDisposition::FailClosedDeferred {
        notes.push(
            "valid input remains fact-bearing but has no Phase 21 artifact emission".to_string(),
        );
    }
    if lowering_disposition == VortexLoweringDisposition::InterpreterOnly {
        notes.push("reader can emit a verified canonical artifact, but original Vortex shape is outside the production native-lowering slice".to_string());
    }

    VortexEncodingCoverage {
        dtype_kind: root_dtype.kind.clone(),
        nullable: root_dtype.nullable,
        root_layout_encoding: root_layout_encoding.to_string(),
        layout_class: layout_class.to_string(),
        array_encoding,
        has_splits: !split_facts.is_empty(),
        has_statistics,
        reader_support,
        emission_kind,
        emission_disposition,
        lowering_disposition,
        notes,
    }
}

fn layout_class(root_layout_encoding: &str) -> &'static str {
    if root_layout_encoding.contains("stats") {
        "statistics-wrapper"
    } else if root_layout_encoding.contains("struct") {
        "struct"
    } else if root_layout_encoding.contains("chunk") {
        "chunked"
    } else {
        "primitive-or-leaf"
    }
}

fn array_encoding(root_dtype: &VortexReaderDTypeFact, root_layout_encoding: &str) -> String {
    let root_layout_encoding = root_layout_encoding.to_ascii_lowercase();
    if root_layout_encoding.contains("dict") {
        return "dictionary".to_string();
    }
    if root_layout_encoding.contains("runend") || root_layout_encoding.contains("rle") {
        return "run-end".to_string();
    }
    if root_layout_encoding.contains("sequence") {
        return "sequence".to_string();
    }
    if root_layout_encoding.contains("bitpack") || root_layout_encoding.contains("bit-packed") {
        return "bitpack".to_string();
    }
    if root_layout_encoding.contains("for") || root_layout_encoding.contains("frame") {
        return "frame-of-reference".to_string();
    }

    match root_dtype.kind.as_str() {
        "primitive" => "primitive".to_string(),
        "struct" => "struct".to_string(),
        "utf8" | "binary" => "varbin".to_string(),
        "bool" => "boolean".to_string(),
        other => {
            if other.is_empty() {
                "unknown".to_string()
            } else {
                other.to_string()
            }
        }
    }
}

fn collect_layout_facts(
    layout: vortex_layout::LayoutRef,
    path: String,
    child_context: Option<(String, String, Option<u64>)>,
    facts: &mut Vec<VortexReaderLayoutFact>,
    diagnostics: &mut Vec<VortexReaderDiagnostic>,
) {
    let segment_ids = layout.segment_ids().into_iter().map(|id| *id).collect();
    let (child_type, child_name, child_row_offset) = child_context
        .map(|(child_type, child_name, child_row_offset)| {
            (Some(child_type), Some(child_name), child_row_offset)
        })
        .unwrap_or((None, None, None));

    facts.push(VortexReaderLayoutFact {
        path: path.clone(),
        encoding_id: layout.encoding_id().as_ref().to_string(),
        dtype_summary: format!("{:?}", layout.dtype()),
        row_count: layout.row_count(),
        child_count: layout.nchildren(),
        child_type,
        child_name,
        child_row_offset,
        segment_ids,
        metadata_byte_len: layout.metadata().len(),
    });

    let child_types = layout.child_types().collect::<Vec<_>>();
    let child_names = child_types
        .iter()
        .map(|child| child.name().to_string())
        .collect::<Vec<_>>();
    let child_offsets = child_types
        .iter()
        .map(|child| child.row_offset())
        .collect::<Vec<_>>();

    let Ok(children) = layout.children() else {
        diagnostics.push(VortexReaderDiagnostic::new(
            VortexReaderDiagnosticCode::TraversalFailed,
            path,
            "failed to traverse Vortex layout children",
        ));
        return;
    };

    for (idx, child) in children.into_iter().enumerate() {
        let child_type = child_types
            .get(idx)
            .map(|child| format!("{:?}", child))
            .unwrap_or_else(|| "unknown".to_string());
        let child_name = child_names
            .get(idx)
            .cloned()
            .unwrap_or_else(|| format!("child_{idx}"));
        let child_row_offset = child_offsets.get(idx).copied().flatten();
        collect_layout_facts(
            child,
            format!("{path}.children[{idx}]"),
            Some((child_type, child_name, child_row_offset)),
            facts,
            diagnostics,
        );
    }
}

fn dtype_fact_from_dtype(path: impl Into<String>, dtype: &DType) -> VortexReaderDTypeFact {
    let (kind, nullable, field_count, field_names) = match dtype {
        DType::Null => ("null", None, None, Vec::new()),
        DType::Bool(nullability) => ("bool", Some(is_nullable(*nullability)), None, Vec::new()),
        DType::Primitive(_, nullability) => (
            "primitive",
            Some(is_nullable(*nullability)),
            None,
            Vec::new(),
        ),
        DType::Decimal(_, nullability) => {
            ("decimal", Some(is_nullable(*nullability)), None, Vec::new())
        }
        DType::Utf8(nullability) => ("utf8", Some(is_nullable(*nullability)), None, Vec::new()),
        DType::Binary(nullability) => ("binary", Some(is_nullable(*nullability)), None, Vec::new()),
        DType::List(_, nullability) => ("list", Some(is_nullable(*nullability)), None, Vec::new()),
        DType::FixedSizeList(_, _, nullability) => (
            "fixed-size-list",
            Some(is_nullable(*nullability)),
            None,
            Vec::new(),
        ),
        DType::Struct(fields, nullability) => (
            "struct",
            Some(is_nullable(*nullability)),
            Some(fields.nfields()),
            fields
                .names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
        ),
        DType::Union(nullability) => ("union", Some(is_nullable(*nullability)), None, Vec::new()),
        DType::Variant(nullability) => {
            ("variant", Some(is_nullable(*nullability)), None, Vec::new())
        }
        DType::Extension(_) => ("extension", None, None, Vec::new()),
    };

    VortexReaderDTypeFact {
        path: path.into(),
        summary: format!("{dtype:?}"),
        kind: kind.to_string(),
        nullable,
        field_count,
        field_names,
    }
}

fn is_nullable(nullability: Nullability) -> bool {
    matches!(nullability, Nullability::Nullable)
}

fn dtype_fact_from_summary(
    path: impl Into<String>,
    summary: impl Into<String>,
) -> VortexReaderDTypeFact {
    let summary = summary.into();
    VortexReaderDTypeFact {
        path: path.into(),
        kind: dtype_kind_from_summary(&summary).to_string(),
        nullable: dtype_nullable_from_summary(&summary),
        field_count: dtype_field_count_from_summary(&summary),
        field_names: dtype_field_names_from_summary(&summary),
        summary,
    }
}

fn dtype_kind_from_summary(summary: &str) -> &'static str {
    if summary.contains("FixedSizeList") {
        "fixed-size-list"
    } else if summary.contains("Struct") {
        "struct"
    } else if summary.contains("List") {
        "list"
    } else if summary.contains("Decimal") {
        "decimal"
    } else if summary.contains("Utf8") {
        "utf8"
    } else if summary.contains("Binary") {
        "binary"
    } else if summary.contains("Bool") {
        "bool"
    } else if summary.contains("Null") {
        "null"
    } else if summary.contains("Extension") {
        "extension"
    } else if summary.contains("Primitive") || summary.contains("I32") || summary.contains("I64") {
        "primitive"
    } else {
        "unknown"
    }
}

fn dtype_nullable_from_summary(summary: &str) -> Option<bool> {
    if summary.contains("NonNullable") {
        Some(false)
    } else if summary.contains("Nullable") {
        Some(true)
    } else {
        None
    }
}

fn dtype_field_count_from_summary(summary: &str) -> Option<usize> {
    if !summary.contains("Struct") {
        return None;
    }

    Some(summary.matches("FieldDType").count())
}

fn dtype_field_names_from_summary(summary: &str) -> Vec<String> {
    if !summary.contains("Struct") {
        return Vec::new();
    }

    summary
        .split("FieldName(\"")
        .skip(1)
        .filter_map(|part| part.split_once("\")").map(|(name, _)| name.to_string()))
        .collect()
}

fn segment_facts_from_file(file: &VortexFile) -> Vec<VortexReaderSegmentFact> {
    let mut previous_end = None;
    file.footer()
        .segment_map()
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let range = segment.byte_range();
            let ordered_after_previous = previous_end.map(|end| range.start >= end).unwrap_or(true);
            let overlaps_previous = previous_end.map(|end| range.start < end).unwrap_or(false);
            previous_end = Some(range.end);
            VortexReaderSegmentFact {
                id: index as u32,
                index,
                start: range.start,
                end: range.end,
                length: range.end.saturating_sub(range.start),
                alignment: format!("{:?}", segment.alignment),
                ordered_after_previous,
                overlaps_previous,
            }
        })
        .collect()
}

fn split_facts_from_file(
    file: &VortexFile,
    diagnostics: &mut Vec<VortexReaderDiagnostic>,
) -> Vec<VortexReaderSplitFact> {
    match file.splits() {
        Ok(splits) => splits
            .into_iter()
            .enumerate()
            .map(|(index, range)| VortexReaderSplitFact {
                index,
                start_row: range.start,
                end_row: range.end,
                row_count: range.end.saturating_sub(range.start),
            })
            .collect(),
        Err(err) => {
            diagnostics.push(VortexReaderDiagnostic::new(
                VortexReaderDiagnosticCode::SplitUnavailable,
                "$.splits",
                format!("failed to collect Vortex split facts: {err}"),
            ));
            Vec::new()
        }
    }
}

fn opened_buffer_or_report(bytes: &[u8]) -> Result<VortexFile, VortexIngressReport> {
    let session = default_session();
    session
        .open_options()
        .open_buffer(ByteBuffer::copy_from(bytes))
        .map_err(|err| {
            VortexIngressReport::rejected(
                VortexIngressDiagnosticCode::OpenFailed,
                "$",
                format!("failed to open Vortex buffer: {err}"),
            )
        })
}

/// Inspect an in-memory Vortex file buffer.
pub fn inspect_vortex_buffer(bytes: &[u8]) -> VortexIngressReport {
    match opened_buffer_or_report(bytes) {
        Ok(file) => report_for_opened_file(&file, VortexIngressSourceKind::Buffer),
        Err(report) => report,
    }
}

/// Extract Phase 18 complete-reader facts from an in-memory Vortex file buffer.
///
/// Valid but unsupported files still return facts with `support = Unsupported`
/// and `emission_kind = None`. Invalid input returns a rejected ingress report
/// and no partial Loom artifact may be emitted.
pub fn reader_facts_from_vortex_buffer(
    bytes: &[u8],
) -> Result<VortexReaderFacts, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    Ok(reader_facts_from_file(
        &file,
        VortexIngressSourceKind::Buffer,
    ))
}

/// Inspect a local Vortex file path.
pub fn inspect_vortex_path(path: &Path) -> VortexIngressReport {
    let session = default_session();
    match RUNTIME.block_on(session.open_options().open_path(path)) {
        Ok(file) => report_for_opened_file(&file, VortexIngressSourceKind::Path),
        Err(err) => VortexIngressReport::rejected(
            VortexIngressDiagnosticCode::OpenFailed,
            "$.path",
            format!("failed to open Vortex path: {err}"),
        ),
    }
}

/// Extract Phase 18 complete-reader facts from a local Vortex file path.
pub fn reader_facts_from_vortex_path(
    path: &Path,
) -> Result<VortexReaderFacts, VortexIngressReport> {
    let session = default_session();
    RUNTIME
        .block_on(session.open_options().open_path(path))
        .map(|file| reader_facts_from_file(&file, VortexIngressSourceKind::Path))
        .map_err(|err| {
            VortexIngressReport::rejected(
                VortexIngressDiagnosticCode::OpenFailed,
                "$.path",
                format!("failed to open Vortex path: {err}"),
            )
        })
}

/// Emit a verifier-compatible Loom `LMC1` container for the supported reader slice.
///
/// Phase 18 supports an explicit fail-closed matrix: non-null primitive
/// single-column files emit wrapped `LMP1`, and non-null struct files whose
/// fields all match that matrix emit wrapped `LMT1`.
pub fn emit_supported_lmc1_from_vortex_buffer(
    bytes: &[u8],
) -> Result<Vec<u8>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let mut facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);

    if let Ok(table) = scan_supported_table(&file) {
        facts.supported_loom_payload = true;
        let payload = encode_table_payload(&table).map_err(|err| {
            VortexIngressReport::unsupported(
                Some(facts.clone()),
                VortexIngressDiagnosticCode::UnsupportedConversion,
                "$.payload",
                format!("failed to encode supported Loom table payload: {err}"),
            )
        })?;
        return wrap_table_payload(&payload).map_err(|err| {
            VortexIngressReport::unsupported(
                Some(facts),
                VortexIngressDiagnosticCode::UnsupportedConversion,
                "$.payload",
                format!("failed to wrap supported Loom table payload: {err}"),
            )
        });
    }

    let desc = scan_supported_single_column_layout(&file).map_err(|message| {
        facts.supported_loom_payload = false;
        VortexIngressReport::unsupported(
            Some(facts.clone()),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            message,
        )
    })?;

    facts.supported_loom_payload = true;
    let payload = encode_layout_payload(&desc);
    wrap_layout_payload(&payload).map_err(|err| {
        VortexIngressReport::unsupported(
            Some(facts),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            format!("failed to wrap supported Loom payload: {err}"),
        )
    })
}

/// Scan the supported real Vortex slice through Vortex and return Loom-owned rows.
///
/// This is oracle evidence for tests and diagnostics; it does not expose Vortex
/// types or bypass the emitted `LMC1` verifier/decode path.
pub fn scan_i32_values_from_vortex_buffer(bytes: &[u8]) -> Result<Vec<i32>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);
    scan_supported_i32_values(&file).map_err(|message| {
        VortexIngressReport::unsupported(
            Some(facts),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            message,
        )
    })
}

/// Scan the supported real Vortex Int64 slice through Vortex for oracle evidence.
pub fn scan_i64_values_from_vortex_buffer(bytes: &[u8]) -> Result<Vec<i64>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);
    scan_supported_i64_values(&file).map_err(|message| {
        VortexIngressReport::unsupported(
            Some(facts),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            message,
        )
    })
}

/// Scan the supported real Vortex Float32 slice through Vortex for oracle evidence.
pub fn scan_f32_values_from_vortex_buffer(bytes: &[u8]) -> Result<Vec<f32>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);
    scan_supported_f32_values(&file).map_err(|message| {
        VortexIngressReport::unsupported(
            Some(facts),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            message,
        )
    })
}

/// Scan the supported real Vortex Float64 slice through Vortex for oracle evidence.
pub fn scan_f64_values_from_vortex_buffer(bytes: &[u8]) -> Result<Vec<f64>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);
    scan_supported_f64_values(&file).map_err(|message| {
        VortexIngressReport::unsupported(
            Some(facts),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            message,
        )
    })
}

fn single_column_support(file: &VortexFile) -> Result<SingleColumnSupport, String> {
    single_column_support_for_dtype(file.dtype())
}

fn single_column_support_for_dtype(dtype: &DType) -> Result<SingleColumnSupport, String> {
    match dtype {
        DType::Primitive(PType::I32, Nullability::NonNullable) => Ok(SingleColumnSupport {
            ptype: PType::I32,
            data_type: DataType::Int32,
            elem_size: 4,
        }),
        DType::Primitive(PType::I64, Nullability::NonNullable) => Ok(SingleColumnSupport {
            ptype: PType::I64,
            data_type: DataType::Int64,
            elem_size: 8,
        }),
        DType::Primitive(PType::F32, Nullability::NonNullable) => Ok(SingleColumnSupport {
            ptype: PType::F32,
            data_type: DataType::Float32,
            elem_size: 4,
        }),
        DType::Primitive(PType::F64, Nullability::NonNullable) => Ok(SingleColumnSupport {
            ptype: PType::F64,
            data_type: DataType::Float64,
            elem_size: 8,
        }),
        dtype => Err(format!(
            "supported single-column matrix requires non-null Int32/Int64/Float32/Float64 rows, found {dtype:?}"
        )),
    }
}

fn reader_emission_kind(file: &VortexFile) -> VortexReaderEmissionKind {
    if table_support(file).is_ok() {
        VortexReaderEmissionKind::Lmt1
    } else if single_column_support(file).is_ok() {
        VortexReaderEmissionKind::Lmp1
    } else {
        VortexReaderEmissionKind::None
    }
}

fn table_support(file: &VortexFile) -> Result<Vec<(String, SingleColumnSupport)>, String> {
    let DType::Struct(fields, Nullability::NonNullable) = file.dtype() else {
        return Err(format!(
            "supported table matrix requires non-null struct rows, found {:?}",
            file.dtype()
        ));
    };

    let mut supported = Vec::with_capacity(fields.nfields());
    for idx in 0..fields.nfields() {
        let name = fields
            .field_name(idx)
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("field_{idx}"));
        let dtype = fields
            .field_by_index(idx)
            .ok_or_else(|| format!("failed to inspect struct field {name}"))?;
        supported.push((name, single_column_support_for_dtype(&dtype)?));
    }

    Ok(supported)
}

fn scan_supported_table(file: &VortexFile) -> Result<TableDescription, String> {
    let support = table_support(file)?;
    let array = RUNTIME.block_on(async {
        let stream = file
            .scan()
            .map_err(|err| format!("failed to create Vortex table scan: {err}"))?
            .into_array_stream()
            .map_err(|err| format!("failed to create Vortex table array stream: {err}"))?;
        stream
            .read_all()
            .await
            .map_err(|err| format!("failed to scan Vortex table rows: {err}"))
    })?;

    let mut ctx = file.session().create_execution_ctx();
    let struct_array = array
        .execute::<StructArray>(&mut ctx)
        .map_err(|err| format!("supported table matrix requires struct rows: {err}"))?;

    if has_nulls(&struct_array.struct_validity(), struct_array.len()) {
        return Err("supported table matrix requires non-null struct rows".to_string());
    }

    let fields = struct_array.unmasked_fields();
    if fields.len() != support.len() {
        return Err(format!(
            "supported table matrix field count mismatch: {} != {}",
            fields.len(),
            support.len()
        ));
    }

    let mut columns = Vec::with_capacity(support.len());
    for ((name, support), field) in support.into_iter().zip(fields.iter()) {
        let canonical = field
            .clone()
            .execute::<PrimitiveArray>(&mut ctx)
            .map_err(|err| {
                format!("supported table field {name} requires primitive rows: {err}")
            })?;
        let layout = primitive_array_to_layout(canonical, &support)?;
        columns.push(TableColumn { name, layout });
    }

    Ok(TableDescription {
        row_count: struct_array.len(),
        columns,
    })
}

fn scan_supported_single_column_layout(file: &VortexFile) -> Result<LayoutDescription, String> {
    let support = single_column_support(file)?;
    let canonical = scan_supported_primitive_array(file, &support)?;
    primitive_array_to_layout(canonical, &support)
}

fn primitive_array_to_layout(
    canonical: PrimitiveArray,
    support: &SingleColumnSupport,
) -> Result<LayoutDescription, String> {
    let len = canonical.as_ref().len();
    if canonical.ptype() != support.ptype {
        return Err(format!(
            "supported primitive matrix expected {:?} rows, found {:?}",
            support.ptype,
            canonical.ptype(),
        ));
    }
    let validity = PrimitiveArrayExt::validity(&canonical);
    if has_nulls(&validity, len) {
        return Err("supported primitive matrix requires non-null rows".to_string());
    }

    let data = match support.ptype {
        PType::I32 => canonical
            .as_slice::<i32>()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
        PType::I64 => canonical
            .as_slice::<i64>()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
        PType::F32 => canonical
            .as_slice::<f32>()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
        PType::F64 => canonical
            .as_slice::<f64>()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
        other => {
            return Err(format!(
                "supported single-column matrix cannot encode {other:?} rows"
            ))
        }
    };

    Ok(LayoutDescription {
        data_type: support.data_type.clone(),
        row_count: len,
        root: LayoutNode::Raw {
            data,
            elem_size: support.elem_size,
            count: len,
        },
    })
}

fn scan_supported_primitive_array(
    file: &VortexFile,
    support: &SingleColumnSupport,
) -> Result<PrimitiveArray, String> {
    if !matches!(file.dtype(), DType::Primitive(_, Nullability::NonNullable)) {
        return Err(format!(
            "supported single-column matrix requires non-null rows, found {:?}",
            file.dtype()
        ));
    }

    let array = RUNTIME.block_on(async {
        let stream = file
            .scan()
            .map_err(|err| format!("failed to create Vortex scan: {err}"))?
            .into_array_stream()
            .map_err(|err| format!("failed to create Vortex array stream: {err}"))?;
        stream
            .read_all()
            .await
            .map_err(|err| format!("failed to scan Vortex rows: {err}"))
    })?;

    let mut ctx = file.session().create_execution_ctx();
    let canonical = array
        .execute::<PrimitiveArray>(&mut ctx)
        .map_err(|err| format!("supported slice requires primitive rows: {err}"))?;

    if canonical.ptype() != support.ptype {
        return Err(format!(
            "supported single-column matrix expected {:?} rows, found {:?}",
            support.ptype,
            canonical.ptype(),
        ));
    }

    let validity = PrimitiveArrayExt::validity(&canonical);
    if has_nulls(&validity, canonical.as_ref().len()) {
        return Err("supported single-column matrix requires non-null rows".to_string());
    }

    Ok(canonical)
}

fn scan_supported_i32_values(file: &VortexFile) -> Result<Vec<i32>, String> {
    let support = single_column_support(file)?;
    if support.ptype != PType::I32 {
        return Err(format!(
            "supported Int32 oracle requires Int32 rows, found {:?}",
            file.dtype()
        ));
    }
    Ok(scan_supported_primitive_array(file, &support)?
        .as_slice::<i32>()
        .to_vec())
}

fn scan_supported_i64_values(file: &VortexFile) -> Result<Vec<i64>, String> {
    let support = single_column_support(file)?;
    if support.ptype != PType::I64 {
        return Err(format!(
            "supported Int64 oracle requires Int64 rows, found {:?}",
            file.dtype()
        ));
    }
    Ok(scan_supported_primitive_array(file, &support)?
        .as_slice::<i64>()
        .to_vec())
}

fn scan_supported_f32_values(file: &VortexFile) -> Result<Vec<f32>, String> {
    let support = single_column_support(file)?;
    if support.ptype != PType::F32 {
        return Err(format!(
            "supported Float32 oracle requires Float32 rows, found {:?}",
            file.dtype()
        ));
    }
    Ok(scan_supported_primitive_array(file, &support)?
        .as_slice::<f32>()
        .to_vec())
}

fn scan_supported_f64_values(file: &VortexFile) -> Result<Vec<f64>, String> {
    let support = single_column_support(file)?;
    if support.ptype != PType::F64 {
        return Err(format!(
            "supported Float64 oracle requires Float64 rows, found {:?}",
            file.dtype()
        ));
    }
    Ok(scan_supported_primitive_array(file, &support)?
        .as_slice::<f64>()
        .to_vec())
}

fn has_nulls(validity: &Validity, len: usize) -> bool {
    match validity {
        Validity::NonNullable | Validity::AllValid => false,
        Validity::AllInvalid => len > 0,
        Validity::Array(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_array::IntoArray;
    use vortex_buffer::buffer;
    use vortex_buffer::ByteBufferMut;
    use vortex_file::WriteOptionsSessionExt;

    fn simple_vortex_file_bytes() -> Vec<u8> {
        let session = default_session();
        let mut buf = ByteBufferMut::empty();
        let array = buffer![10i32, 20, 30].into_array();
        RUNTIME
            .block_on(
                session
                    .write_options()
                    .write(&mut buf, array.to_array_stream()),
            )
            .expect("write simple Vortex file");
        buf.as_slice().to_vec()
    }

    #[test]
    fn malformed_buffer_is_rejected_with_stable_code() {
        let report = inspect_vortex_buffer(&[]);
        assert_eq!(report.status, VortexIngressStatus::Rejected);
        assert!(report.facts.is_none());
        assert_eq!(report.diagnostics[0].code.as_str(), "INGRESS_OPEN_FAILED");
    }

    #[test]
    fn real_buffer_reports_vortex_facts_and_supported_slice() {
        let bytes = simple_vortex_file_bytes();
        let report = inspect_vortex_buffer(&bytes);
        assert_eq!(report.status, VortexIngressStatus::Accepted);
        assert!(report.diagnostics.is_empty());

        let facts = report.facts.expect("facts for opened Vortex file");
        assert_eq!(facts.source_kind, VortexIngressSourceKind::Buffer);
        assert_eq!(facts.vortex_file_version, vortex_file::VERSION);
        assert_eq!(facts.row_count, 3);
        assert!(facts.dtype_summary.contains("I32"));
        assert!(!facts.layout_summary.is_empty());
        assert_eq!(facts.segment_count, facts.segment_ranges.len());
        assert!(facts.supported_loom_payload);
    }
}
