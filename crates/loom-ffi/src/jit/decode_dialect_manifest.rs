//! Rust manifest for the compiled `loom.decode` ODS source surface.
//!
//! The manifest is intentionally available in default builds. It lets tests and
//! future backend gates compare the Phase 20 textual dialect names against the
//! Phase 23 ODS records without requiring `mlir-tblgen`.

use std::path::{Path, PathBuf};

pub const ODS_DIALECT_SOURCE: &str = "mlir/include/LoomDecode/LoomDecodeDialect.td";
pub const ODS_OPS_SOURCE: &str = "mlir/include/LoomDecode/LoomDecodeOps.td";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeDialectDisposition {
    Structural,
    NativeSupported,
    DeclaredGuarded,
    InterpreterOnly,
    Deferred,
}

impl DecodeDialectDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::NativeSupported => "native-supported",
            Self::DeclaredGuarded => "declared-guarded",
            Self::InterpreterOnly => "interpreter-only",
            Self::Deferred => "deferred",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeDialectOpManifest {
    pub textual_name: &'static str,
    pub ods_record: &'static str,
    pub attributes: &'static [&'static str],
    pub disposition: DecodeDialectDisposition,
    pub ods_source: &'static str,
}

pub const DECODE_DIALECT_MANIFEST: &[DecodeDialectOpManifest] = &[
    DecodeDialectOpManifest {
        textual_name: "loom.decode.module",
        ods_record: "LoomDecode_ModuleOp",
        attributes: &[
            "artifact_kind",
            "payload_kind",
            "rows",
            "constraint_status",
            "backend",
            "columns",
        ],
        disposition: DecodeDialectDisposition::Structural,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.kernel",
        ods_record: "LoomDecode_KernelOp",
        attributes: &["rows"],
        disposition: DecodeDialectDisposition::Structural,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.input_slice",
        ods_record: "LoomDecode_InputSliceOp",
        attributes: &["slice_id", "offset", "length"],
        disposition: DecodeDialectDisposition::Deferred,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.column",
        ods_record: "LoomDecode_ColumnOp",
        attributes: &["builder_id", "arrow_type", "nullable"],
        disposition: DecodeDialectDisposition::Structural,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.builder",
        ods_record: "LoomDecode_BuilderOp",
        attributes: &["builder_id", "arrow_type", "validity"],
        disposition: DecodeDialectDisposition::Structural,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.finish",
        ods_record: "LoomDecode_FinishOp",
        attributes: &["builder_id"],
        disposition: DecodeDialectDisposition::Structural,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.for_rows",
        ods_record: "LoomDecode_ForRowsOp",
        attributes: &["start", "end"],
        disposition: DecodeDialectDisposition::Structural,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.bit_unpack",
        ods_record: "LoomDecode_BitpackUnpackOp",
        attributes: &["builder_id", "row", "bit_width"],
        disposition: DecodeDialectDisposition::DeclaredGuarded,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.for_delta",
        ods_record: "LoomDecode_FrameOfReferenceDeltaOp",
        attributes: &["builder_id", "row", "reference"],
        disposition: DecodeDialectDisposition::DeclaredGuarded,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.validity_all_valid",
        ods_record: "LoomDecode_ValidityAllValidOp",
        attributes: &["builder_id", "row"],
        disposition: DecodeDialectDisposition::NativeSupported,
        ods_source: ODS_OPS_SOURCE,
    },
    DecodeDialectOpManifest {
        textual_name: "loom.decode.validity_copy",
        ods_record: "LoomDecode_ValidityCopyOp",
        attributes: &["builder_id", "row", "bitmap_offset"],
        disposition: DecodeDialectDisposition::InterpreterOnly,
        ods_source: ODS_OPS_SOURCE,
    },
];

pub fn decode_dialect_manifest() -> &'static [DecodeDialectOpManifest] {
    DECODE_DIALECT_MANIFEST
}

pub fn manifest_entry(textual_name: &str) -> Option<&'static DecodeDialectOpManifest> {
    DECODE_DIALECT_MANIFEST
        .iter()
        .find(|entry| entry.textual_name == textual_name)
}

pub fn ods_source_paths() -> [PathBuf; 2] {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    [
        manifest_dir.join(ODS_DIALECT_SOURCE),
        manifest_dir.join(ODS_OPS_SOURCE),
    ]
}
