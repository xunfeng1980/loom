# Plan 49-01 Summary: Independent L2Core IR Codec

**Status:** Complete

## Delivered

`crates/loom-core/src/l2core_codec.rs` — a standalone binary wire format for `L2CoreProgram` with:
- `L2IR` magic bytes + `u16` version header
- Little-endian fixed-width encoding for integers, floats
- Length-prefixed (UInt16LE) strings, vectors, and maps
- `u8` enum discriminants for `Capability`, `ScalarValue`, `ScalarExpr`, `L2CoreStmt`
- Narrow `DataType` subset: Boolean, Int32, Int64, Float32, Float64, Utf8
- `ResourceBudget` and feature set encoding
- Round-trip stability: encode→decode→reencode byte-identical
- Zero dependency on container codecs (verified by source grep)

**Key file:** `crates/loom-core/src/l2core_codec.rs`
