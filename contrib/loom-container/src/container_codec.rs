//! Versioned Loom distribution container codec.
//!
//! `LMC1` is the first explicit distribution boundary for Loom payloads. Phase
//! 11 keeps the existing `LMP1` and `LMT1` codecs intact by wrapping those
//! bytes in checked container sections.

use loom_ir_core::error::LoomDecodeError;
use loom_common::l1_model::LayoutDescription;
use crate::layout_codec::decode_layout_payload;
use crate::table_codec::{decode_table_payload, TableDescription};

pub const MAGIC: &[u8; 4] = b"LMC1";
pub const VERSION: u16 = 1;

const RAW_LAYOUT_MAGIC: &[u8; 4] = b"LMP1";
const RAW_TABLE_MAGIC: &[u8; 4] = b"LMT1";

const HEADER_PREFIX_LEN: usize = 4 + 2 + 2 + 8 + 8 + 4;
const SECTION_ENTRY_LEN: usize = 2 + 2 + 8 + 8 + 4 + 4;

pub const SECTION_FLAG_REQUIRED: u16 = 1;

const SECTION_SCHEMA: u16 = 1;
const SECTION_LAYOUT_PAYLOAD: u16 = 2;
const SECTION_TABLE_PAYLOAD: u16 = 3;
const SECTION_KERNEL_MANIFEST: u16 = 4;
const SECTION_STATS: u16 = 5;
const SECTION_DEBUG_DESCRIPTOR: u16 = 255;

const FEATURE_SINGLE_COLUMN_LMP1: u8 = 0;
const FEATURE_TABLE_LMT1: u8 = 1;
const FEATURE_KERNEL_FSST: u8 = 2;
const FEATURE_KERNEL_ALP_FLOAT: u8 = 3;
const FEATURE_FLOAT32_FLOAT64: u8 = 4;
const FEATURE_DEBUG_SECTIONS: u8 = 5;
const FEATURE_STATS_SECTION: u8 = 6;

const KNOWN_REQUIRED_FEATURE_MASK: u64 = (1 << FEATURE_SINGLE_COLUMN_LMP1)
    | (1 << FEATURE_TABLE_LMT1)
    | (1 << FEATURE_KERNEL_FSST)
    | (1 << FEATURE_KERNEL_ALP_FLOAT)
    | (1 << FEATURE_FLOAT32_FLOAT64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDescription {
    pub version: u16,
    pub required_features: u64,
    pub optional_features: u64,
    pub sections: Vec<ContainerSection>,
    pub has_trailer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSection {
    pub kind: SectionKind,
    pub flags: u16,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

impl ContainerSection {
    pub fn required(kind: SectionKind, bytes: Vec<u8>) -> Self {
        Self {
            kind,
            flags: SECTION_FLAG_REQUIRED,
            offset: 0,
            bytes,
        }
    }

    pub fn optional(kind: SectionKind, bytes: Vec<u8>) -> Self {
        Self {
            kind,
            flags: 0,
            offset: 0,
            bytes,
        }
    }

    pub fn is_required(&self) -> bool {
        self.flags & SECTION_FLAG_REQUIRED != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Schema,
    LayoutPayload,
    TablePayload,
    KernelManifest,
    Stats,
    DebugDescriptor,
    Unknown(u16),
}

impl SectionKind {
    pub fn tag(self) -> u16 {
        match self {
            Self::Schema => SECTION_SCHEMA,
            Self::LayoutPayload => SECTION_LAYOUT_PAYLOAD,
            Self::TablePayload => SECTION_TABLE_PAYLOAD,
            Self::KernelManifest => SECTION_KERNEL_MANIFEST,
            Self::Stats => SECTION_STATS,
            Self::DebugDescriptor => SECTION_DEBUG_DESCRIPTOR,
            Self::Unknown(tag) => tag,
        }
    }

    pub fn from_tag(tag: u16) -> Self {
        match tag {
            SECTION_SCHEMA => Self::Schema,
            SECTION_LAYOUT_PAYLOAD => Self::LayoutPayload,
            SECTION_TABLE_PAYLOAD => Self::TablePayload,
            SECTION_KERNEL_MANIFEST => Self::KernelManifest,
            SECTION_STATS => Self::Stats,
            SECTION_DEBUG_DESCRIPTOR => Self::DebugDescriptor,
            other => Self::Unknown(other),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Schema => "schema",
            Self::LayoutPayload => "layout_payload",
            Self::TablePayload => "table_payload",
            Self::KernelManifest => "kernel_manifest",
            Self::Stats => "stats",
            Self::DebugDescriptor => "debug_descriptor",
            Self::Unknown(_) => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    SingleColumnLmp1,
    TableLmt1,
    KernelFsst,
    KernelAlpFloat,
    Float32Float64,
    DebugSections,
    StatsSection,
}

impl Feature {
    pub fn bit(self) -> u8 {
        match self {
            Self::SingleColumnLmp1 => FEATURE_SINGLE_COLUMN_LMP1,
            Self::TableLmt1 => FEATURE_TABLE_LMT1,
            Self::KernelFsst => FEATURE_KERNEL_FSST,
            Self::KernelAlpFloat => FEATURE_KERNEL_ALP_FLOAT,
            Self::Float32Float64 => FEATURE_FLOAT32_FLOAT64,
            Self::DebugSections => FEATURE_DEBUG_SECTIONS,
            Self::StatsSection => FEATURE_STATS_SECTION,
        }
    }

    pub fn mask(self) -> u64 {
        1u64 << self.bit()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleColumnLmp1 => "single_column_lmp1",
            Self::TableLmt1 => "table_lmt1",
            Self::KernelFsst => "kernel_fsst",
            Self::KernelAlpFloat => "kernel_alp_float",
            Self::Float32Float64 => "float32_float64",
            Self::DebugSections => "debug_sections",
            Self::StatsSection => "stats_section",
        }
    }

    pub fn from_bit(bit: u8) -> Option<Self> {
        match bit {
            FEATURE_SINGLE_COLUMN_LMP1 => Some(Self::SingleColumnLmp1),
            FEATURE_TABLE_LMT1 => Some(Self::TableLmt1),
            FEATURE_KERNEL_FSST => Some(Self::KernelFsst),
            FEATURE_KERNEL_ALP_FLOAT => Some(Self::KernelAlpFloat),
            FEATURE_FLOAT32_FLOAT64 => Some(Self::Float32Float64),
            FEATURE_DEBUG_SECTIONS => Some(Self::DebugSections),
            FEATURE_STATS_SECTION => Some(Self::StatsSection),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    RawLayout,
    RawTable,
    Container,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrappedPayload {
    Layout(Vec<u8>),
    Table(Vec<u8>),
}

pub fn payload_kind(bytes: &[u8]) -> PayloadKind {
    if bytes.starts_with(RAW_LAYOUT_MAGIC) {
        PayloadKind::RawLayout
    } else if bytes.starts_with(RAW_TABLE_MAGIC) {
        PayloadKind::RawTable
    } else if bytes.starts_with(MAGIC) {
        PayloadKind::Container
    } else {
        PayloadKind::Unknown
    }
}

pub fn is_container_payload(bytes: &[u8]) -> bool {
    matches!(payload_kind(bytes), PayloadKind::Container)
}

pub fn feature_bitset(features: &[Feature]) -> u64 {
    features
        .iter()
        .fold(0u64, |bits, feature| bits | feature.mask())
}

pub fn feature_names(bits: u64) -> Vec<&'static str> {
    (0..64)
        .filter_map(|bit| {
            if bits & (1u64 << bit) == 0 {
                return None;
            }
            Feature::from_bit(bit).map(Feature::as_str)
        })
        .collect()
}

pub fn unknown_feature_bits(bits: u64) -> u64 {
    bits & !known_feature_mask()
}

pub fn known_feature_mask() -> u64 {
    feature_bitset(&[
        Feature::SingleColumnLmp1,
        Feature::TableLmt1,
        Feature::KernelFsst,
        Feature::KernelAlpFloat,
        Feature::Float32Float64,
        Feature::DebugSections,
        Feature::StatsSection,
    ])
}

pub fn encode_container(desc: &ContainerDescription) -> Result<Vec<u8>, LoomDecodeError> {
    validate_feature_bits(desc.required_features)?;
    validate_sections(&desc.sections)?;

    let section_count = u32::try_from(desc.sections.len())
        .map_err(|_| LoomDecodeError::MalformedContainer("section count overflow"))?;
    let header_len = HEADER_PREFIX_LEN
        .checked_add(desc.sections.len().checked_mul(SECTION_ENTRY_LEN).ok_or(
            LoomDecodeError::MalformedContainer("header length overflow"),
        )?)
        .ok_or(LoomDecodeError::MalformedContainer(
            "header length overflow",
        ))?;
    let header_len_u16 = u16::try_from(header_len)
        .map_err(|_| LoomDecodeError::MalformedContainer("header length overflow"))?;

    let mut entries = Vec::with_capacity(desc.sections.len());
    let mut offset = u64::try_from(header_len)
        .map_err(|_| LoomDecodeError::MalformedContainer("header length overflow"))?;
    for section in &desc.sections {
        let length = u64::try_from(section.bytes.len())
            .map_err(|_| LoomDecodeError::MalformedContainer("section length overflow"))?;
        entries.push((section.kind, section.flags, offset, length));
        offset = offset
            .checked_add(length)
            .ok_or(LoomDecodeError::MalformedContainer(
                "section offset overflow",
            ))?;
    }

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&header_len_u16.to_le_bytes());
    out.extend_from_slice(&desc.required_features.to_le_bytes());
    out.extend_from_slice(&desc.optional_features.to_le_bytes());
    out.extend_from_slice(&section_count.to_le_bytes());
    for (kind, flags, entry_offset, length) in entries {
        out.extend_from_slice(&kind.tag().to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&entry_offset.to_le_bytes());
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
    }
    for section in &desc.sections {
        out.extend_from_slice(&section.bytes);
    }
    if desc.has_trailer {
        out.extend_from_slice(MAGIC);
    }
    Ok(out)
}

pub fn decode_container(bytes: &[u8]) -> Result<ContainerDescription, LoomDecodeError> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_array::<4>()?;
    if &magic != MAGIC {
        return Err(LoomDecodeError::MalformedContainer("wrong magic"));
    }

    let version = reader.read_u16()?;
    if version != VERSION {
        return Err(LoomDecodeError::MalformedContainer("unsupported version"));
    }

    let header_len = usize::from(reader.read_u16()?);
    if header_len < HEADER_PREFIX_LEN {
        return Err(LoomDecodeError::MalformedContainer("header too short"));
    }

    let required_features = reader.read_u64()?;
    validate_feature_bits(required_features)?;
    let optional_features = reader.read_u64()?;
    let section_count = usize::try_from(reader.read_u32()?)
        .map_err(|_| LoomDecodeError::MalformedContainer("section count overflow"))?;
    let expected_header_len = HEADER_PREFIX_LEN
        .checked_add(section_count.checked_mul(SECTION_ENTRY_LEN).ok_or(
            LoomDecodeError::MalformedContainer("header length overflow"),
        )?)
        .ok_or(LoomDecodeError::MalformedContainer(
            "header length overflow",
        ))?;
    if header_len != expected_header_len {
        return Err(LoomDecodeError::MalformedContainer(
            "header length mismatch",
        ));
    }
    if header_len > bytes.len() {
        return Err(LoomDecodeError::MalformedContainer("truncated header"));
    }

    let mut entries = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        let tag = reader.read_u16()?;
        let flags = reader.read_u16()?;
        if flags & !SECTION_FLAG_REQUIRED != 0 {
            return Err(LoomDecodeError::MalformedContainer("unknown section flags"));
        }
        let offset = reader.read_u64()?;
        let length = reader.read_u64()?;
        let checksum_or_reserved = reader.read_u32()?;
        if checksum_or_reserved != 0 {
            return Err(LoomDecodeError::MalformedContainer(
                "checksum field must be zero",
            ));
        }
        let reserved = reader.read_u32()?;
        if reserved != 0 {
            return Err(LoomDecodeError::MalformedContainer(
                "reserved field must be zero",
            ));
        }
        let kind = SectionKind::from_tag(tag);
        if matches!(kind, SectionKind::Unknown(_)) && flags & SECTION_FLAG_REQUIRED != 0 {
            return Err(LoomDecodeError::MalformedContainer(
                "unknown required section",
            ));
        }
        entries.push(SectionEntry {
            kind,
            flags,
            offset,
            length,
        });
    }
    if reader.pos != header_len {
        return Err(LoomDecodeError::MalformedContainer(
            "header length mismatch",
        ));
    }

    let has_trailer = bytes.len() >= MAGIC.len() && &bytes[bytes.len() - MAGIC.len()..] == MAGIC;
    let payload_end = if has_trailer {
        bytes.len() - MAGIC.len()
    } else {
        bytes.len()
    };
    let mut sorted = entries.clone();
    sorted.sort_by_key(|entry| entry.offset);
    let mut expected_offset = header_len;
    for entry in &sorted {
        let offset = usize::try_from(entry.offset)
            .map_err(|_| LoomDecodeError::MalformedContainer("section offset overflow"))?;
        let length = usize::try_from(entry.length)
            .map_err(|_| LoomDecodeError::MalformedContainer("section length overflow"))?;
        let end = offset
            .checked_add(length)
            .ok_or(LoomDecodeError::MalformedContainer(
                "section offset overflow",
            ))?;
        if offset != expected_offset {
            return Err(LoomDecodeError::MalformedContainer(
                "section gap or overlap",
            ));
        }
        if end > payload_end {
            return Err(LoomDecodeError::MalformedContainer(
                "section outside container",
            ));
        }
        expected_offset = end;
    }
    if expected_offset != payload_end {
        return Err(LoomDecodeError::MalformedContainer(
            "trailing section bytes",
        ));
    }

    let mut sections = Vec::with_capacity(entries.len());
    for entry in entries {
        let offset = usize::try_from(entry.offset)
            .map_err(|_| LoomDecodeError::MalformedContainer("section offset overflow"))?;
        let length = usize::try_from(entry.length)
            .map_err(|_| LoomDecodeError::MalformedContainer("section length overflow"))?;
        let end = offset
            .checked_add(length)
            .ok_or(LoomDecodeError::MalformedContainer(
                "section offset overflow",
            ))?;
        sections.push(ContainerSection {
            kind: entry.kind,
            flags: entry.flags,
            offset: entry.offset,
            bytes: bytes[offset..end].to_vec(),
        });
    }
    validate_sections(&sections)?;

    Ok(ContainerDescription {
        version,
        required_features,
        optional_features,
        sections,
        has_trailer,
    })
}

pub fn wrap_layout_payload(payload: &[u8]) -> Result<Vec<u8>, LoomDecodeError> {
    let desc = ContainerDescription {
        version: VERSION,
        required_features: feature_bitset(&[Feature::SingleColumnLmp1]),
        optional_features: 0,
        sections: vec![
            ContainerSection::required(SectionKind::Schema, b"single-column".to_vec()),
            ContainerSection::required(SectionKind::LayoutPayload, payload.to_vec()),
        ],
        has_trailer: true,
    };
    encode_container(&desc)
}

pub fn wrap_table_payload(payload: &[u8]) -> Result<Vec<u8>, LoomDecodeError> {
    let desc = ContainerDescription {
        version: VERSION,
        required_features: feature_bitset(&[Feature::TableLmt1]),
        optional_features: 0,
        sections: vec![
            ContainerSection::required(SectionKind::Schema, b"table".to_vec()),
            ContainerSection::required(SectionKind::TablePayload, payload.to_vec()),
        ],
        has_trailer: true,
    };
    encode_container(&desc)
}

pub fn layout_payload_section(desc: &ContainerDescription) -> Option<&[u8]> {
    desc.sections
        .iter()
        .find(|section| section.kind == SectionKind::LayoutPayload)
        .map(|section| section.bytes.as_slice())
}

pub fn table_payload_section(desc: &ContainerDescription) -> Option<&[u8]> {
    desc.sections
        .iter()
        .find(|section| section.kind == SectionKind::TablePayload)
        .map(|section| section.bytes.as_slice())
}

pub fn extract_wrapped_payload(bytes: &[u8]) -> Result<WrappedPayload, LoomDecodeError> {
    match payload_kind(bytes) {
        PayloadKind::RawLayout => Ok(WrappedPayload::Layout(bytes.to_vec())),
        PayloadKind::RawTable => Ok(WrappedPayload::Table(bytes.to_vec())),
        PayloadKind::Container => {
            let container = decode_container(bytes)?;
            if let Some(payload) = layout_payload_section(&container) {
                Ok(WrappedPayload::Layout(payload.to_vec()))
            } else if let Some(payload) = table_payload_section(&container) {
                Ok(WrappedPayload::Table(payload.to_vec()))
            } else {
                Err(LoomDecodeError::MalformedContainer(
                    "expected payload section",
                ))
            }
        }
        PayloadKind::Unknown => Err(LoomDecodeError::MalformedContainer("unknown payload kind")),
    }
}

pub fn decode_layout_payload_maybe_container(
    bytes: &[u8],
) -> Result<LayoutDescription, LoomDecodeError> {
    match extract_wrapped_payload(bytes)? {
        WrappedPayload::Layout(payload) => decode_layout_payload(&payload),
        WrappedPayload::Table(_) => Err(LoomDecodeError::MalformedContainer(
            "expected layout payload",
        )),
    }
}

pub fn decode_table_payload_maybe_container(
    bytes: &[u8],
) -> Result<TableDescription, LoomDecodeError> {
    match extract_wrapped_payload(bytes)? {
        WrappedPayload::Table(payload) => decode_table_payload(&payload),
        WrappedPayload::Layout(_) => Err(LoomDecodeError::MalformedContainer(
            "expected table payload",
        )),
    }
}

fn validate_feature_bits(required_features: u64) -> Result<(), LoomDecodeError> {
    if required_features & !KNOWN_REQUIRED_FEATURE_MASK != 0 {
        return Err(LoomDecodeError::MalformedContainer(
            "unknown required feature",
        ));
    }
    Ok(())
}

fn validate_sections(sections: &[ContainerSection]) -> Result<(), LoomDecodeError> {
    let mut schema_count = 0usize;
    let mut layout_count = 0usize;
    let mut table_count = 0usize;
    for section in sections {
        match section.kind {
            SectionKind::Schema => schema_count += 1,
            SectionKind::LayoutPayload => layout_count += 1,
            SectionKind::TablePayload => table_count += 1,
            SectionKind::Unknown(_) if section.is_required() => {
                return Err(LoomDecodeError::MalformedContainer(
                    "unknown required section",
                ));
            }
            _ => {}
        }
    }

    if schema_count != 1 {
        return Err(LoomDecodeError::MalformedContainer(
            "expected exactly one schema section",
        ));
    }
    if layout_count + table_count != 1 {
        return Err(LoomDecodeError::MalformedContainer(
            "expected exactly one payload section",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SectionEntry {
    kind: SectionKind,
    flags: u16,
    offset: u64,
    length: u64,
}

struct Reader<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], LoomDecodeError> {
        let bytes = self.read_bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn read_u16(&mut self) -> Result<u16, LoomDecodeError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, LoomDecodeError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, LoomDecodeError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], LoomDecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(LoomDecodeError::MalformedContainer("truncated container"))?;
        if end > self.input.len() {
            return Err(LoomDecodeError::MalformedContainer("truncated container"));
        }
        let bytes = &self.input[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_common::l1_model::LayoutNode;
    use crate::layout_codec::encode_layout_payload;
    use crate::table_codec::{encode_table_payload, TableColumn};
    use arrow_schema::DataType;

    fn layout_payload() -> Vec<u8> {
        let mut bytes = RAW_LAYOUT_MAGIC.to_vec();
        bytes.extend_from_slice(b"layout-bytes");
        bytes
    }

    fn table_payload() -> Vec<u8> {
        let mut bytes = RAW_TABLE_MAGIC.to_vec();
        bytes.extend_from_slice(b"table-bytes");
        bytes
    }

    fn raw_i32_desc() -> LayoutDescription {
        LayoutDescription {
            data_type: DataType::Int32,
            root: LayoutNode::Raw {
                data: [10i32, -20, 30]
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect(),
                elem_size: 4,
                count: 3,
            },
            row_count: 3,
        }
    }

    fn simple_table() -> TableDescription {
        TableDescription {
            row_count: 3,
            columns: vec![TableColumn {
                name: "value".to_string(),
                layout: raw_i32_desc(),
            }],
        }
    }

    fn minimal_layout_container() -> ContainerDescription {
        ContainerDescription {
            version: VERSION,
            required_features: feature_bitset(&[Feature::SingleColumnLmp1]),
            optional_features: feature_bitset(&[Feature::DebugSections]),
            sections: vec![
                ContainerSection::required(SectionKind::Schema, b"single-column".to_vec()),
                ContainerSection::required(SectionKind::LayoutPayload, layout_payload()),
                ContainerSection::optional(SectionKind::DebugDescriptor, b"debug".to_vec()),
            ],
            has_trailer: true,
        }
    }

    #[test]
    fn feature_bitsets_have_names_and_unknown_bits() {
        let bits = feature_bitset(&[
            Feature::SingleColumnLmp1,
            Feature::KernelFsst,
            Feature::Float32Float64,
        ]);

        assert_eq!(
            feature_names(bits),
            vec!["single_column_lmp1", "kernel_fsst", "float32_float64"]
        );
        assert_eq!(unknown_feature_bits(bits), 0);
        assert_ne!(unknown_feature_bits(1u64 << 63), 0);
    }

    #[test]
    fn section_kind_roundtrips_known_and_unknown_tags() {
        for kind in [
            SectionKind::Schema,
            SectionKind::LayoutPayload,
            SectionKind::TablePayload,
            SectionKind::KernelManifest,
            SectionKind::Stats,
            SectionKind::DebugDescriptor,
        ] {
            assert_eq!(SectionKind::from_tag(kind.tag()), kind);
        }
        assert_eq!(SectionKind::from_tag(999), SectionKind::Unknown(999));
    }

    #[test]
    fn roundtrip_layout_container_preserves_sections() {
        let encoded = encode_container(&minimal_layout_container()).expect("encode container");
        assert_eq!(payload_kind(&encoded), PayloadKind::Container);
        assert!(is_container_payload(&encoded));

        let decoded = decode_container(&encoded).expect("decode container");

        assert_eq!(decoded.version, VERSION);
        assert!(decoded.has_trailer);
        assert_eq!(decoded.sections.len(), 3);
        assert_eq!(
            layout_payload_section(&decoded).expect("layout section"),
            layout_payload()
        );
        assert!(table_payload_section(&decoded).is_none());
    }

    #[test]
    fn roundtrip_table_container_preserves_payload() {
        let encoded = wrap_table_payload(&table_payload()).expect("wrap table");
        let decoded = decode_container(&encoded).expect("decode table container");

        assert_eq!(
            table_payload_section(&decoded).expect("table section"),
            table_payload()
        );
        assert_eq!(layout_payload_section(&decoded), None);
    }

    #[test]
    fn wrappers_preserve_raw_payload_bytes() {
        let layout = layout_payload();
        let wrapped_layout = wrap_layout_payload(&layout).expect("wrap layout");
        let decoded_layout = decode_container(&wrapped_layout).expect("decode wrapped layout");
        assert_eq!(
            layout_payload_section(&decoded_layout),
            Some(layout.as_slice())
        );

        let table = table_payload();
        let wrapped_table = wrap_table_payload(&table).expect("wrap table");
        let decoded_table = decode_container(&wrapped_table).expect("decode wrapped table");
        assert_eq!(
            table_payload_section(&decoded_table),
            Some(table.as_slice())
        );
    }

    #[test]
    fn layout_decode_helper_accepts_raw_and_wrapped_payloads() {
        let raw = encode_layout_payload(&raw_i32_desc());
        let wrapped = wrap_layout_payload(&raw).expect("wrap layout");

        let raw_decoded = decode_layout_payload_maybe_container(&raw).expect("decode raw");
        let wrapped_decoded =
            decode_layout_payload_maybe_container(&wrapped).expect("decode wrapped");

        assert_eq!(raw_decoded.data_type, wrapped_decoded.data_type);
        assert_eq!(raw_decoded.row_count, wrapped_decoded.row_count);
    }

    #[test]
    fn table_decode_helper_accepts_raw_and_wrapped_payloads() {
        let raw = encode_table_payload(&simple_table()).expect("encode table");
        let wrapped = wrap_table_payload(&raw).expect("wrap table");

        let raw_decoded = decode_table_payload_maybe_container(&raw).expect("decode raw table");
        let wrapped_decoded =
            decode_table_payload_maybe_container(&wrapped).expect("decode wrapped table");

        assert_eq!(raw_decoded.row_count, wrapped_decoded.row_count);
        assert_eq!(raw_decoded.columns[0].name, wrapped_decoded.columns[0].name);
    }

    #[test]
    fn layout_decode_helper_rejects_table_container() {
        let raw = encode_table_payload(&simple_table()).expect("encode table");
        let wrapped = wrap_table_payload(&raw).expect("wrap table");

        let err = decode_layout_payload_maybe_container(&wrapped).expect_err("reject table");
        assert_eq!(
            err,
            LoomDecodeError::MalformedContainer("expected layout payload")
        );
    }

    #[test]
    fn payload_kind_classifies_known_magic_values() {
        assert_eq!(payload_kind(&layout_payload()), PayloadKind::RawLayout);
        assert_eq!(payload_kind(&table_payload()), PayloadKind::RawTable);
        assert_eq!(
            payload_kind(&wrap_layout_payload(&layout_payload()).expect("wrap layout")),
            PayloadKind::Container
        );
        assert_eq!(payload_kind(b"NOPE"), PayloadKind::Unknown);
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut encoded = encode_container(&minimal_layout_container()).expect("encode");
        encoded[0] = b'X';

        assert_eq!(
            decode_container(&encoded),
            Err(LoomDecodeError::MalformedContainer("wrong magic"))
        );
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut encoded = encode_container(&minimal_layout_container()).expect("encode");
        encoded[4..6].copy_from_slice(&2u16.to_le_bytes());

        assert_eq!(
            decode_container(&encoded),
            Err(LoomDecodeError::MalformedContainer("unsupported version"))
        );
    }

    #[test]
    fn rejects_unknown_required_feature() {
        let mut desc = minimal_layout_container();
        desc.required_features |= 1u64 << 63;

        assert_eq!(
            encode_container(&desc),
            Err(LoomDecodeError::MalformedContainer(
                "unknown required feature"
            ))
        );
    }

    #[test]
    fn rejects_truncated_header() {
        let encoded = encode_container(&minimal_layout_container()).expect("encode");

        assert_eq!(
            decode_container(&encoded[..10]),
            Err(LoomDecodeError::MalformedContainer("truncated container"))
        );
    }

    #[test]
    fn rejects_truncated_directory() {
        let mut encoded = encode_container(&minimal_layout_container()).expect("encode");
        encoded[6..8].copy_from_slice(&(HEADER_PREFIX_LEN as u16).to_le_bytes());

        assert_eq!(
            decode_container(&encoded),
            Err(LoomDecodeError::MalformedContainer(
                "header length mismatch"
            ))
        );
    }

    #[test]
    fn rejects_section_offset_overflow() {
        let mut encoded = encode_container(&minimal_layout_container()).expect("encode");
        let first_entry_len = HEADER_PREFIX_LEN + 12;
        encoded[first_entry_len..first_entry_len + 8].copy_from_slice(&u64::MAX.to_le_bytes());

        assert_eq!(
            decode_container(&encoded),
            Err(LoomDecodeError::MalformedContainer(
                "section offset overflow"
            ))
        );
    }

    #[test]
    fn rejects_section_outside_payload() {
        let mut encoded = encode_container(&minimal_layout_container()).expect("encode");
        let first_entry_len = HEADER_PREFIX_LEN + 12;
        encoded[first_entry_len..first_entry_len + 8].copy_from_slice(&9999u64.to_le_bytes());

        assert_eq!(
            decode_container(&encoded),
            Err(LoomDecodeError::MalformedContainer(
                "section outside container"
            ))
        );
    }

    #[test]
    fn rejects_duplicate_required_payload_section() {
        let mut desc = minimal_layout_container();
        desc.sections.push(ContainerSection::required(
            SectionKind::LayoutPayload,
            layout_payload(),
        ));

        assert_eq!(
            encode_container(&desc),
            Err(LoomDecodeError::MalformedContainer(
                "expected exactly one payload section"
            ))
        );
    }

    #[test]
    fn rejects_trailing_corruption_when_trailer_is_changed() {
        let mut encoded = encode_container(&minimal_layout_container()).expect("encode");
        let last = encoded.len() - 1;
        encoded[last] = b'X';

        assert_eq!(
            decode_container(&encoded),
            Err(LoomDecodeError::MalformedContainer(
                "trailing section bytes"
            ))
        );
    }
}
