#ifndef LOOM_DECODE_OPS_TD
#define LOOM_DECODE_OPS_TD

include "mlir/IR/OpBase.td"
include "LoomDecode/LoomDecodeDialect.td"

class LoomDecode_Op<string mnemonic, list<Trait> traits = []> :
    Op<LoomDecode_Dialect, mnemonic, traits>;

def LoomDecode_ModuleOp : LoomDecode_Op<"module", [IsolatedFromAbove]> {
  let summary = "verified Loom artifact decode module";
  let description = [{
    `loom.decode.module` records the verified artifact kind, payload kind, finite
    row count, constraint status, backend name, and output column count.

    Verifier note: operation verification checks metadata shape only. It does not
    replace Loom artifact verification, solver discharge, runtime planning, or
    cache-key validation.
  }];
  let arguments = (ins
    StrAttr:$artifact_kind,
    StrAttr:$payload_kind,
    I64Attr:$rows,
    StrAttr:$constraint_status,
    StrAttr:$backend,
    I64Attr:$columns
  );
  let regions = (region SizedRegion<1>:$body);
  let hasVerifier = 1;
}

def LoomDecode_KernelOp : LoomDecode_Op<"kernel"> {
  let summary = "finite decode kernel region";
  let description = [{
    `loom.decode.kernel` groups the finite decode loop and builder operations
    for one accepted runtime plan.

    Verifier note: `rows` must match the enclosing module metadata and the
    verified production lowering facts.
  }];
  let arguments = (ins I64Attr:$rows);
  let regions = (region SizedRegion<1>:$body);
  let hasVerifier = 1;
}

def LoomDecode_InputSliceOp : LoomDecode_Op<"input_slice"> {
  let summary = "verified input buffer slice";
  let description = [{
    `loom.decode.input_slice` names a verifier-owned physical input slice. It is
    declared for the compiled surface even though Phase 20 textual emission may
    omit physical slices until stable per-column source ranges are available.
  }];
  let arguments = (ins
    StrAttr:$slice_id,
    I64Attr:$offset,
    I64Attr:$length
  );
  let hasVerifier = 1;
}

def LoomDecode_ColumnOp : LoomDecode_Op<"column"> {
  let summary = "logical output column declaration";
  let description = [{
    `loom.decode.column` declares a non-null primitive output column in the
    current supported slice.
  }];
  let arguments = (ins
    StrAttr:$builder_id,
    StrAttr:$arrow_type,
    BoolAttr:$nullable
  );
  let hasVerifier = 1;
}

def LoomDecode_BuilderOp : LoomDecode_Op<"builder"> {
  let summary = "Arrow/raw-buffer builder declaration";
  let description = [{
    `loom.decode.builder` declares one output builder. Phase 23 supported
    builder types are Int32, Int64, Float32, and Float64 with all-valid validity.
  }];
  let arguments = (ins
    StrAttr:$builder_id,
    StrAttr:$arrow_type,
    StrAttr:$validity
  );
  let hasVerifier = 1;
}

def LoomDecode_FinishOp : LoomDecode_Op<"finish", [Terminator]> {
  let summary = "finish one decode builder or kernel region";
  let description = [{
    `loom.decode.finish` closes a builder or terminates a kernel region.
  }];
  let arguments = (ins OptionalAttr<StrAttr>:$builder_id);
}

def LoomDecode_ForRowsOp : LoomDecode_Op<"for_rows"> {
  let summary = "finite row loop";
  let description = [{
    `loom.decode.for_rows` models a finite row loop derived from the verified row
    bound. SIMD width and target vectorization are later lowering choices.
  }];
  let arguments = (ins I64Attr:$start, I64Attr:$end);
  let regions = (region SizedRegion<1>:$body);
  let hasVerifier = 1;
}

def LoomDecode_RawCopyOp : LoomDecode_Op<"raw_copy"> {
  let summary = "Raw primitive copy kernel";
  let description = [{
    `loom.decode.raw_copy` is the initial production-supported primitive kernel.
  }];
  let arguments = (ins StrAttr:$builder_id, I64Attr:$row);
  let hasVerifier = 1;
}

def LoomDecode_BitpackUnpackOp : LoomDecode_Op<"bit_unpack"> {
  let summary = "Bitpack primitive unpack kernel";
  let description = [{
    `loom.decode.bit_unpack` is declared for the compiled surface and remains
    guarded by production lowering facts before native backend execution.
  }];
  let arguments = (ins
    StrAttr:$builder_id,
    I64Attr:$row,
    I64Attr:$bit_width
  );
  let hasVerifier = 1;
}

def LoomDecode_FrameOfReferenceDeltaOp : LoomDecode_Op<"for_delta"> {
  let summary = "FrameOfReference delta decode kernel";
  let description = [{
    `loom.decode.for_delta` is declared for FrameOfReference primitive decode and
    remains guarded by overflow/range facts before native backend execution.
  }];
  let arguments = (ins
    StrAttr:$builder_id,
    I64Attr:$row,
    I64Attr:$reference
  );
  let hasVerifier = 1;
}

def LoomDecode_ValidityAllValidOp : LoomDecode_Op<"validity_all_valid"> {
  let summary = "all-valid validity marker";
  let description = [{
    `loom.decode.validity_all_valid` marks the current non-null primitive path.
  }];
  let arguments = (ins StrAttr:$builder_id, I64Attr:$row);
  let hasVerifier = 1;
}

def LoomDecode_ValidityCopyOp : LoomDecode_Op<"validity_copy"> {
  let summary = "copied validity bitmap marker";
  let description = [{
    `loom.decode.validity_copy` is declared for future nullable primitive support
    but is not part of the current native execution slice.
  }];
  let arguments = (ins
    StrAttr:$builder_id,
    I64Attr:$row,
    I64Attr:$bitmap_offset
  );
  let hasVerifier = 1;
}

#endif // LOOM_DECODE_OPS_TD
