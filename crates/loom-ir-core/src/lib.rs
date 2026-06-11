//! `loom-ir-core` — independent Loom decode IR.
//!
//! This crate owns the L2Core program model, binary codec, content-hash
//! identity, structural/full verifier, sidecar overlay contract, sidecar
//! routing decision, and host-neutral runtime ABI. It has **zero** Arrow
//! dependencies and **zero** container/packaging dependencies.

#![forbid(unsafe_code)]

pub mod error;
pub mod l2_core;
pub mod l2core_codec;
pub mod full_verifier;
