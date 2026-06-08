//! Isolated real Vortex file ingress boundary.
//!
//! This crate is the only workspace crate that may depend on `vortex-file`.
//! It translates real Vortex file/container metadata into Loom-owned facts and
//! diagnostics so `loom-core` and `loom-ffi` remain Vortex-free.

use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use arrow_schema::DataType;
use loom_core::container_codec::wrap_layout_payload;
use loom_core::l1_model::{LayoutDescription, LayoutNode};
use loom_core::layout_codec::encode_layout_payload;
use vortex_array::arrays::primitive::PrimitiveArrayExt;
use vortex_array::arrays::PrimitiveArray;
use vortex_array::dtype::PType;
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

/// Stable diagnostic code vocabulary for the complete-reader boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VortexReaderDiagnosticCode {
    OpenFailed,
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
            Self::TraversalFailed => "READER_TRAVERSAL_FAILED",
            Self::UnsupportedLayout => "READER_UNSUPPORTED_LAYOUT",
            Self::UnsupportedDType => "READER_UNSUPPORTED_DTYPE",
            Self::UnsupportedConversion => "READER_UNSUPPORTED_CONVERSION",
            Self::VerificationRequired => "READER_VERIFICATION_REQUIRED",
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
    pub statistics_present: bool,
    pub footer_approx_byte_size: Option<usize>,
    pub support: VortexReaderSupport,
    pub emission_kind: VortexReaderEmissionKind,
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
    if supported_int32_non_nullable(file).is_ok() {
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
    let support = if supported_int32_non_nullable(file).is_ok() {
        VortexReaderSupport::Accepted
    } else {
        VortexReaderSupport::Unsupported
    };
    let emission_kind = match support {
        VortexReaderSupport::Accepted => VortexReaderEmissionKind::Lmp1,
        VortexReaderSupport::Unsupported | VortexReaderSupport::Rejected => {
            VortexReaderEmissionKind::None
        }
    };

    let mut layout_facts = Vec::new();
    collect_layout_facts(layout.clone(), "$".to_string(), None, &mut layout_facts);

    let dtype_facts = layout_facts
        .iter()
        .map(|layout| dtype_fact_from_summary(layout.path.clone(), layout.dtype_summary.clone()))
        .collect::<Vec<_>>();
    let root_dtype = dtype_fact_from_summary("$", format!("{:?}", file.dtype()));

    VortexReaderFacts {
        source_kind,
        vortex_file_version: vortex_file::VERSION,
        row_count: file.row_count(),
        root_dtype,
        root_layout_encoding: layout.encoding_id().as_ref().to_string(),
        layout_facts,
        dtype_facts,
        segment_facts: segment_facts_from_file(file),
        statistics_present: file.file_stats().is_some(),
        footer_approx_byte_size: footer.approx_byte_size(),
        support,
        emission_kind,
    }
}

fn collect_layout_facts(
    layout: vortex_layout::LayoutRef,
    path: String,
    child_context: Option<(String, String, Option<u64>)>,
    facts: &mut Vec<VortexReaderLayoutFact>,
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
            format!("{path}/children/{idx}"),
            Some((child_type, child_name, child_row_offset)),
            facts,
        );
    }
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
        summary,
    }
}

fn dtype_kind_from_summary(summary: &str) -> &'static str {
    if summary.contains("Struct") {
        "struct"
    } else if summary.contains("List") {
        "list"
    } else if summary.contains("Utf8") {
        "utf8"
    } else if summary.contains("Bool") {
        "bool"
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

/// Emit a verifier-compatible Loom `LMC1` container for the supported ingress slice.
///
/// Phase 15 intentionally supports only one tiny slice: real Vortex files whose
/// scanned payload canonicalizes to non-null `Int32` rows. All other valid
/// Vortex files return an unsupported report and no bytes.
pub fn emit_supported_lmc1_from_vortex_buffer(
    bytes: &[u8],
) -> Result<Vec<u8>, VortexIngressReport> {
    let file = opened_buffer_or_report(bytes)?;
    let mut facts = facts_from_file(&file, VortexIngressSourceKind::Buffer);

    let values = scan_supported_i32_values(&file).map_err(|message| {
        facts.supported_loom_payload = false;
        VortexIngressReport::unsupported(
            Some(facts.clone()),
            VortexIngressDiagnosticCode::UnsupportedConversion,
            "$.payload",
            message,
        )
    })?;

    facts.supported_loom_payload = true;
    let desc = raw_i32_layout(values);
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

fn supported_int32_non_nullable(file: &VortexFile) -> Result<(), String> {
    scan_supported_i32_values(file).map(|_| ())
}

fn scan_supported_i32_values(file: &VortexFile) -> Result<Vec<i32>, String> {
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

    if canonical.ptype() != PType::I32 {
        return Err(format!(
            "supported slice requires Int32 rows, found {:?}",
            canonical.ptype()
        ));
    }

    let validity = PrimitiveArrayExt::validity(&canonical);
    if has_nulls(&validity, canonical.as_ref().len()) {
        return Err("supported slice requires non-null Int32 rows".to_string());
    }

    Ok(canonical.as_slice::<i32>().to_vec())
}

fn raw_i32_layout(values: Vec<i32>) -> LayoutDescription {
    LayoutDescription {
        data_type: DataType::Int32,
        row_count: values.len(),
        root: LayoutNode::Raw {
            data: values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
            elem_size: 4,
            count: values.len(),
        },
    }
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
