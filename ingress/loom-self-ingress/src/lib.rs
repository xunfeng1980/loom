//! Single IO boundary for Loom native `.loom` file format.
//!
//! All format-specific logic is delegated to [`loom_container`]; this crate
//! provides the validated file-system boundary. Every function performs
//! container-level validation before returning bytes or writing to disk,
//! ensuring malformed or non-Loom files are rejected at the boundary.
//!
//! # Public API
//!
//! - [`read_loom_file`] — read and validate a `.loom` file, returning raw bytes
//! - [`write_loom_file`] — validate and atomically write bytes to a `.loom` file
//! - [`verify_loom_file`] — read, validate, and return the decoded
//!   [`ContainerDescription`] for inspection
//!
//! [`ContainerDescription`]: loom_container_legacy::container_codec::ContainerDescription

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use loom_container_legacy::container_codec;

/// Errors that can occur during Loom file ingress/egress operations.
#[derive(Debug)]
pub enum SelfIngressError {
    /// File-system I/O error (read or write).
    Io(io::Error),
    /// The file does not contain valid Loom container magic bytes.
    NotALoomFile,
    /// The byte content is structurally invalid (decode failed).
    InvalidContainer(String),
}

impl fmt::Display for SelfIngressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelfIngressError::Io(err) => write!(f, "IO error: {err}"),
            SelfIngressError::NotALoomFile => {
                write!(f, "not a Loom container file (missing LMC1/LMC2 magic)")
            }
            SelfIngressError::InvalidContainer(msg) => {
                write!(f, "invalid Loom container: {msg}")
            }
        }
    }
}

impl Error for SelfIngressError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SelfIngressError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for SelfIngressError {
    fn from(err: io::Error) -> Self {
        SelfIngressError::Io(err)
    }
}

/// Read a `.loom` file, verifying it is a valid Loom container.
///
/// Returns the raw file bytes on success. The caller may decode further
/// (e.g. via `container_codec::decode_container` or higher-level helpers)
/// but is guaranteed that the bytes pass the container magic check.
///
/// # Errors
///
/// - `SelfIngressError::Io` if the file cannot be read
/// - `SelfIngressError::NotALoomFile` if the magic bytes do not match
///   `LMC1` or `LMC2`
pub fn read_loom_file(path: &Path) -> Result<Vec<u8>, SelfIngressError> {
    let bytes = fs::read(path)?;
    if !container_codec::is_container_payload(&bytes) {
        return Err(SelfIngressError::NotALoomFile);
    }
    Ok(bytes)
}

/// Write bytes to a `.loom` file after validating they form a valid container.
///
/// Uses an atomic write pattern: bytes are first written to a temporary file
/// (`{path}.tmp.loom-ingress`), then renamed to the target path. This prevents
/// partial or corrupt writes from appearing at the final path.
///
/// # Errors
///
/// - `SelfIngressError::InvalidContainer` if the bytes do not decode as a
///   valid Loom container
/// - `SelfIngressError::Io` if the file cannot be written or renamed
pub fn write_loom_file(path: &Path, bytes: &[u8]) -> Result<(), SelfIngressError> {
    // Validate the bytes form a valid container before touching disk.
    container_codec::decode_container(bytes)
        .map_err(|err| SelfIngressError::InvalidContainer(err.to_string()))?;

    // Atomic write: temp file → rename.
    let tmp_path = path.with_extension("tmp.loom-ingress");
    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)?;

    Ok(())
}

/// Read and fully verify a `.loom` file, returning the decoded
/// [`ContainerDescription`].
///
/// This combines the magic check and full container decode into a single
/// call so callers can inspect version, features, sections, and trailer
/// without re-decoding.
///
/// [`ContainerDescription`]: container_codec::ContainerDescription
///
/// # Errors
///
/// - `SelfIngressError::Io` if the file cannot be read
/// - `SelfIngressError::NotALoomFile` if the magic bytes do not match
/// - `SelfIngressError::InvalidContainer` if the container structure is
///   malformed
pub fn verify_loom_file(path: &Path) -> Result<container_codec::ContainerDescription, SelfIngressError> {
    let bytes = fs::read(path)?;
    if !container_codec::is_container_payload(&bytes) {
        return Err(SelfIngressError::NotALoomFile);
    }
    container_codec::decode_container(&bytes)
        .map_err(|err| SelfIngressError::InvalidContainer(err.to_string()))
}
