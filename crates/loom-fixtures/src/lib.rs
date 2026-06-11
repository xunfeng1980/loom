//! `loom-fixtures` — reference fixture builders and oracle decoder.
//!
//! This is the **only** crate in the Loom workspace permitted to depend on
//! `vortex-*` crates (D-02). Keeping Vortex isolated here preserves the
//! independence of `loom-core`'s decode proof: the thing being verified
//! (`loom-core`) must not share code paths with the oracle that verifies it.
//!
//! # Modules
//!
//! - [`vortex_reader`] — inspects in-memory Vortex `BitPackedArray`/`FoRArray`
//!   and emits a `loom_ffi::l1_model::LayoutNode` + raw packed bytes. The sole
//!   gateway between the Vortex ecosystem and `loom-core` (D-02 isolation).
//! - [`oracle`] — decodes the same array via Vortex's own `execute` path,
//!   returning plain Rust values for row-for-row comparison against `loom-core`.

pub mod corpus;
pub mod oracle;
pub mod vortex_reader;
