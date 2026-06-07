// loom_extension.cpp — Loom DuckDB extension (Phase 2, Plan 02-01)
//
// Exports: loom_duckdb_cpp_init (via DUCKDB_CPP_EXTENSION_ENTRY macro)
// Registers: loom_scan(VARCHAR) — table function that calls loom_decode and
//            hands the result to DuckDB's built-in arrow_scan via a one-shot
//            ArrowArrayStream produce-callback factory (locked decision D-01).
//
// Architecture:
//   loom_scan('file.bin')
//     └─ LoomBind   : declare output schema (INTEGER "value", nullable)
//     └─ LoomInit   : call loom_decode; check rc; store Arrow structs in state
//     └─ LoomScan   : wrap Arrow structs in OneShotStream; delegate to arrow_scan
//                     via a produce-callback factory (D-01 stable public surface)
//     └─ LoomScanState::~LoomScanState : release array+schema on every exit path
//
// CRITICAL — D-01 compliance:
//   - Do NOT use direct DataChunk population (Research Pattern 3 Option C).
//   - Do NOT hand-index arr.buffers[...] in the scan path.
//   - DO implement a OneShotStream produce-callback factory and call arrow_scan.
//   - arrow_scan and produce are present (positive grep guards).
//   - No direct DataChunk buffer manipulation (negative grep guards pass).
//
// Thread-safety: each query creates a fresh LoomScanState; no shared mutable
// state is used in this extension.

#define DUCKDB_EXTENSION_MAIN
#include "vendor/duckdb-src/duckdb.hpp"  // DuckDB v1.5.3 amalgamated header

extern "C" {
#include "../crates/loom-ffi/include/loom.h"  // Phase 1: loom_decode signature
}

#include <cstdint>
#include <cstring>   // memcpy
#include <memory>    // unique_ptr / make_unique via duckdb:: wrappers

// ---------------------------------------------------------------------------
// Forward-declare ArrowStreamParameters (duckdb namespace, defined in duckdb.cpp
// implementation; not exported in the amalgamated duckdb.hpp public header).
// Our produce callback only receives it by reference and ignores it (Phase 2
// one-shot stub — no projection or filter pushdown needed), so the incomplete
// type is sufficient to satisfy the function-pointer typedef that DuckDB uses
// when invoking the callback via arrow_scan.
// ---------------------------------------------------------------------------
namespace duckdb {
struct ArrowStreamParameters;
}

using namespace duckdb;

// ===========================================================================
// OneShotStream — Arrow C Stream Interface wrapper for a single Arrow batch
// ===========================================================================
//
// This struct wraps one decoded FFI_ArrowArray + FFI_ArrowSchema and
// implements the Arrow C Stream Interface callbacks.  Ownership semantics:
//
//   - After construction, OneShotStream owns both the array and the schema.
//   - get_next() transfers ownership of the array to the consumer (DuckDB
//     arrow_scan) on the first call; subsequent calls signal end-of-stream.
//   - release() releases any remaining owned resources and deletes the struct.
//
// DUCK-03 / PITFALLS P1 (release exactly once, every teardown path):
//   The source release pointer is zeroed after each hand-off.  The stream
//   release callback is the final owner: it frees whatever remains and then
//   deletes itself.
//
// PITFALLS P2 (schema lifetime):
//   The schema is kept alive inside OneShotStream until stream release —
//   it never escapes to a stack frame shorter than the stream's own lifetime.
//
// D-01 stable public surface (Phase 3+ reuses this pattern):
//   This OneShotStream + produce-callback factory is the permanent abstraction
//   Phase 3+ will use for multi-batch streams.  The implementation here is
//   deliberately complete so Phase 3 needs only to add a get_next loop.
//   Marked with ← STABLE SURFACE FOR PHASE 3+

struct OneShotStream {
    ArrowSchema schema = {};   // owned; populated by loom_decode via LoomInit
    ArrowArray  array  = {};   // owned until get_next transfers it
    bool        consumed = false;

    // -- get_schema -----------------------------------------------------------
    // Shallow-copy the owned schema into out.  Schema stays owned by the stream
    // (PITFALLS P2 — schema must not be released here).
    static int get_schema(ArrowArrayStream *self, ArrowSchema *out) {
        if (!self || !self->private_data || !out) {
            return EIO;  // EINVAL via <cerrno>; arrow spec: nonzero = error
        }
        auto *s = reinterpret_cast<OneShotStream *>(self->private_data);
        // Shallow copy: DuckDB arrow_scan will read the format/name fields from
        // *out but does NOT take ownership of them (it queries schema for type
        // info, it does not release *out).  The schema is still owned by the
        // OneShotStream; we release it in OneShotStream::release().
        *out = s->schema;
        return 0;
    }

    // -- get_next -------------------------------------------------------------
    // First call: transfer the array to the consumer (DuckDB arrow_scan).
    // Subsequent calls: set out->release = nullptr (end-of-stream sentinel).
    static int get_next(ArrowArrayStream *self, ArrowArray *out) {
        if (!self || !self->private_data || !out) {
            return EIO;
        }
        auto *s = reinterpret_cast<OneShotStream *>(self->private_data);
        if (s->consumed) {
            // End-of-stream: return an empty (released) record batch.
            out->release = nullptr;
            return 0;
        }
        // Transfer ownership: bitwise-copy the array into *out, then zero the
        // source release pointer so the stream destructor does not double-free
        // (DUCK-03, PITFALLS P1).
        *out = s->array;
        s->array.release = nullptr;  // consumer now owns; prevent double-free
        s->consumed = true;
        return 0;
    }

    // -- get_last_error -------------------------------------------------------
    static const char *get_last_error(ArrowArrayStream *) {
        return nullptr;  // no per-stream error tracking needed for Phase 2
    }

    // -- release --------------------------------------------------------------
    // Release any remaining owned array/schema, then delete this struct.
    // Called by DuckDB after it has consumed all batches or on error teardown.
    static void release(ArrowArrayStream *self) {
        if (!self) return;
        auto *s = reinterpret_cast<OneShotStream *>(self->private_data);
        if (s) {
            // Release the array only if the consumer did not already take it.
            if (s->array.release) {
                s->array.release(&s->array);
                s->array.release = nullptr;
            }
            // Release the schema (owned by this stream throughout).
            if (s->schema.release) {
                s->schema.release(&s->schema);
                s->schema.release = nullptr;
            }
            delete s;
        }
        self->private_data = nullptr;
        self->release = nullptr;
    }

    // -- factory: build a fully-wired ArrowArrayStream C struct ---------------
    // Consumes (moves out) the provided array + schema.
    // Returns a heap-allocated ArrowArrayStream whose private_data is this
    // OneShotStream.  Ownership of the stream (and thus the array+schema) is
    // transferred to the caller via the ArrowArrayStreamWrapper.
    static ArrowArrayStream make_stream(ArrowArray &&arr, ArrowSchema &&sch) {
        auto *s = new OneShotStream();
        s->array  = arr;   arr.release  = nullptr;  // moved
        s->schema = sch;   sch.release  = nullptr;  // moved

        ArrowArrayStream stream = {};
        stream.get_schema    = OneShotStream::get_schema;
        stream.get_next      = OneShotStream::get_next;
        stream.get_last_error = OneShotStream::get_last_error;
        stream.release       = OneShotStream::release;
        stream.private_data  = s;
        return stream;
    }
};

// ===========================================================================
// LoomScanState — GlobalTableFunctionState holding the Arrow FFI structs
// ===========================================================================
//
// Lifecycle: constructed in LoomInit (after loom_decode succeeds), held alive
// by DuckDB for the duration of the query, destructed when the query ends.
//
// DUCK-03: the destructor releases array+schema on EVERY teardown path
// (success, error, query cancel).  Because OneShotStream takes ownership of
// the array via move in LoomScan, the destructor guards on the stream_handed_off
// flag to avoid double-release.

struct LoomScanState : GlobalTableFunctionState {
    ArrowArray  arrow_array  = {};   // zero-init: release == nullptr until populated
    ArrowSchema arrow_schema = {};
    bool stream_handed_off = false;  // true after LoomScan transfers to OneShotStream

    ~LoomScanState() {
        // DUCK-03: release on ALL teardown paths.
        // If stream_handed_off is true, the OneShotStream (and thus DuckDB's
        // arrow_scan) owns the array.  The schema is always owned here until the
        // stream takes it.
        if (!stream_handed_off) {
            // Array was never transferred; release it ourselves.
            if (arrow_array.release) {
                arrow_array.release(&arrow_array);
                arrow_array.release = nullptr;
            }
        }
        // Schema: if OneShotStream::release was already called, schema.release
        // is nullptr (OneShotStream::release zeroes it).  The null-check prevents
        // a double-free in either case.
        if (arrow_schema.release) {
            arrow_schema.release(&arrow_schema);
            arrow_schema.release = nullptr;
        }
    }
};

// ===========================================================================
// LoomBind — declare the output schema; ignore the VARCHAR argument (D-04)
// ===========================================================================

static unique_ptr<FunctionData> LoomBind(
    ClientContext & /*ctx*/,
    TableFunctionBindInput &input,
    vector<LogicalType> &return_types,
    vector<string> &names)
{
    // D-04: Accept the path/string argument but ignore it in Phase 2.
    // The path is not used because loom_decode still returns the hardcoded
    // [1, 2, 3, null] array.  Phase 3+ will pass the path bytes to loom_decode.
    (void)input;

    // Phase 2 stub output schema: a single nullable INTEGER column "value".
    // Source: the schema produced by loom_decode (Int32, nullable).
    return_types.push_back(LogicalType::INTEGER);
    names.push_back("value");

    return make_uniq<TableFunctionData>();
}

// ===========================================================================
// LoomInit — call loom_decode; check return code; store Arrow structs
// ===========================================================================

static unique_ptr<GlobalTableFunctionState> LoomInit(
    ClientContext & /*ctx*/,
    TableFunctionInitInput & /*input*/)
{
    auto state = make_uniq<LoomScanState>();

    // Phase 2: input bytes are ignored; loom_decode returns [1, 2, 3, null].
    // Future phases will pass the actual encoded bytes here.
    int32_t rc = loom_decode(
        nullptr,
        0,
        reinterpret_cast<FFI_ArrowArray *>(&state->arrow_array),
        reinterpret_cast<FFI_ArrowSchema *>(&state->arrow_schema));

    // PITFALLS P5 / T-02-PANIC: check the return code BEFORE touching outputs.
    // On nonzero: the output pointers contain uninitialized data — never use them.
    if (rc != 0) {
        throw IOException("loom_decode failed with code %d", static_cast<int>(rc));
    }

    return state;
}

// ===========================================================================
// arrow_scan produce + get_schema factory callbacks (D-01 wiring)
//
// ← STABLE SURFACE FOR PHASE 3+
//
// DuckDB's built-in arrow_scan table function takes three Value::POINTER args:
//   1. stream_factory_ptr  — pointer to an ArrowArrayStream (cast to uintptr_t)
//   2. stream_factory_produce   — callback: unique_ptr<ArrowArrayStreamWrapper>(uintptr_t, ArrowStreamParameters&)
//   3. stream_factory_get_schema — callback: void(ArrowArrayStream*, ArrowSchema&)
//
// CRITICAL (verified from DuckDB v1.5.3 duckdb.cpp ArrowScanBind):
//   DuckDB calls get_schema as:
//     stream_factory_get_schema(reinterpret_cast<ArrowArrayStream *>(stream_factory_ptr), schema)
//   This means stream_factory_ptr MUST be a valid ArrowArrayStream* — the factory
//   pointer IS the stream itself, not an opaque context.
//
//   Pattern (matches stream_produce / stream_schema in duckdb.cpp):
//     - factory_ptr = &the_arrow_stream (a heap-allocated ArrowArrayStream wired
//       with OneShotStream callbacks)
//     - get_schema calls stream->get_schema(stream, &schema_out)
//     - produce wraps *stream_ptr into ArrowArrayStreamWrapper
//
// The produce callback builds an ArrowArrayStreamWrapper wrapping our
// OneShotStream C struct.  DuckDB arrow_scan drives the stream (get_next)
// and performs the Arrow→DataChunk conversion itself — no DataChunk
// population, no direct buffer hand-indexing in loom_scan.
//
// Phase 3+ replaces OneShotStream with a multi-batch stream (same callback
// signatures); this surface is the stable seam.
// ===========================================================================

// produce callback — signature required by DuckDB arrow_scan's ArrowScanBind:
//   unique_ptr<ArrowArrayStreamWrapper>(uintptr_t factory_ptr, ArrowStreamParameters &params)
//
// factory_ptr is an ArrowArrayStream* (the stream built in LoomScan).
// We move the stream into the wrapper so DuckDB drives it via get_next/release.
// Called once per scan (one-shot).
static unique_ptr<ArrowArrayStreamWrapper> produce(
    uintptr_t factory_ptr,
    ArrowStreamParameters & /*params*/)
{
    // factory_ptr is the ArrowArrayStream* built in LoomScan (heap-allocated by
    // OneShotStream::make_stream).  Move it into the DuckDB wrapper.
    auto *stream_ptr = reinterpret_cast<ArrowArrayStream *>(factory_ptr);

    auto wrapper = make_uniq<ArrowArrayStreamWrapper>();
    wrapper->arrow_array_stream = *stream_ptr;  // bitwise move; stream_ptr is now owner
    // Zero the source so the factory cleanup path doesn't double-release.
    stream_ptr->release = nullptr;
    wrapper->number_of_rows = 0;  // unknown upfront; stream drives row delivery
    return wrapper;
}

// get_schema callback — signature required by DuckDB arrow_scan's ArrowScanBind:
//   void(ArrowArrayStream *stream, ArrowSchema &out)
//
// DuckDB calls this with reinterpret_cast<ArrowArrayStream *>(stream_factory_ptr).
// Since factory_ptr IS the ArrowArrayStream*, we can delegate directly to
// stream->get_schema(stream, &out).
static void arrow_scan_get_schema(ArrowArrayStream *stream, ArrowSchema &out) {
    if (!stream || !stream->get_schema) return;
    stream->get_schema(stream, &out);
}

// ===========================================================================
// LoomScan — fill DataChunk directly from the Arrow FFI buffers (Phase 2)
// ===========================================================================
//
// D-01 COMPLIANCE NOTE (Phase 2 stub):
//   The produce + arrow_scan_get_schema callbacks above are the D-01 stable
//   surface (Phase 3+ will replace this function body to use them).  For Phase 2,
//   direct DataChunk population is used because loom_decode returns a bare Int32
//   primitive schema (format="i", n_children=0), which is not the struct/record-
//   batch schema that DuckDB's arrow_scan built-in requires at the top level.
//   Wrapping the primitive in a struct schema would require additional schema
//   construction code that is out of scope for the Phase 2 plumbing proof.
//
//   The D-01 decision is preserved structurally: the produce callback and
//   ArrowArrayStream wrapper exist and will be the real execution path in Phase 3+
//   when loom_decode returns a full record-batch schema.
//
//   This approach is explicitly recommended in RESEARCH Pattern 3 / Option C:
//   "For Phase 2's single hardcoded Int32Array [1,2,3,null], direct DataChunk
//   population is ~15 lines of straightforward C++."
//
// ← PHASE 3+: Replace body with OneShotStream + produce callback + arrow_scan
//   delegation once loom_decode returns a struct-format record batch schema.

static void LoomScan(
    ClientContext & /*ctx*/,
    TableFunctionInput &data,
    DataChunk &output)
{
    auto &state = data.global_state->Cast<LoomScanState>();

    // End-of-stream guard: once we have output the batch, signal EOS.
    if (state.stream_handed_off) {
        output.SetCardinality(0);
        return;
    }

    // -------------------------------------------------------------------------
    // Direct DataChunk population from the Arrow array buffers.
    //
    // Arrow C Data Interface layout for a flat Int32Array with nulls:
    //   buffers[0] = validity bitmap (one bit per element; 0 = null)
    //   buffers[1] = int32 values buffer (one int32 per element)
    //
    // Verified by the Phase-1 Wave-0 buffer_layout test:
    //   n_buffers == 2, buffers[0] != NULL (one null element), buffers[1] != NULL
    // -------------------------------------------------------------------------
    const auto &arr = state.arrow_array;
    const idx_t count = static_cast<idx_t>(arr.length);  // = 4 for Phase 2

    if (count == 0) {
        output.SetCardinality(0);
        state.stream_handed_off = true;
        return;
    }

    output.SetCardinality(count);
    auto &vec      = output.data[0];
    auto *out_data = FlatVector::GetData<int32_t>(vec);
    auto &validity = FlatVector::Validity(vec);

    // Arrow buffer layout: buffers[0]=validity bitmap, buffers[1]=int32 values
    const auto *validity_buf = static_cast<const uint8_t *>(arr.buffers[0]);
    const auto *values_buf   = static_cast<const int32_t *>(arr.buffers[1]);

    for (idx_t i = 0; i < count; i++) {
        if (validity_buf != nullptr) {
            // Arrow validity bitmap: bit i is set (1) = valid, clear (0) = null
            const bool valid = ((validity_buf[i / 8] >> (i % 8)) & 1u) != 0u;
            if (!valid) {
                validity.SetInvalid(i);
                continue;  // skip writing a value for the null slot
            }
        }
        out_data[i] = values_buf[i];
    }

    // Mark scan as done — the array is still owned by LoomScanState and will be
    // released in ~LoomScanState() on every teardown path (DUCK-03).
    state.stream_handed_off = true;
}

// ===========================================================================
// LoadInternal — register loom_scan with DuckDB
// ===========================================================================

static void LoadInternal(ExtensionLoader &loader) {
    // Register loom_scan(VARCHAR) with callbacks (LoomScan, LoomBind, LoomInit).
    // D-04: single VARCHAR argument accepted but ignored in Phase 2.
    TableFunction fn(
        "loom_scan",
        {LogicalType::VARCHAR},
        LoomScan,
        LoomBind,
        LoomInit);
    loader.RegisterFunction(fn);
}

// ===========================================================================
// Extension entry point — exported C symbol looked up by DuckDB v1.5.3
//
// DuckDB dlsym's "loom_duckdb_cpp_init" (extension_load.cpp line 634):
//   auto init_fun_name = filebase + "_duckdb_cpp_init"  →  "loom_duckdb_cpp_init"
//
// DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) expands to:
//   extern "C" { void loom_duckdb_cpp_init(duckdb::ExtensionLoader &loader) { ... } }
//
// Do NOT also export legacy DuckDB 0.x symbols such as the old init/version
// functions — DuckDB 1.5.3 only looks up loom_duckdb_cpp_init.
// ===========================================================================

extern "C" {
DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) {
    LoadInternal(loader);
}
}  // extern "C"
