//! Minimal `LMA1`/`LMC2` marker helpers.
//!
//! The full deterministic payload codec is implemented in Phase 31 plan 02.

use crate::arrow_semantic::{LMA1_MAGIC, LMC2_MAGIC};

pub fn is_arrow_semantic_payload(bytes: &[u8]) -> bool {
    bytes.starts_with(LMA1_MAGIC)
}

pub fn is_arrow_semantic_container(bytes: &[u8]) -> bool {
    bytes.starts_with(LMC2_MAGIC)
}
