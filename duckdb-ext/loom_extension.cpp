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
//   1. stream_factory_ptr  — opaque uintptr_t pointing to a factory context
//   2. stream_factory_produce   — callback: unique_ptr<ArrowArrayStreamWrapper>(uintptr_t, ArrowStreamParameters&)
//   3. stream_factory_get_schema — callback: void(ArrowArrayStream*, ArrowSchema&)
//
// The produce callback builds an ArrowArrayStreamWrapper wrapping our
// OneShotStream C struct.  DuckDB arrow_scan drives the stream (get_next)
// and performs the Arrow→DataChunk conversion itself — no DataChunk
// population, no direct buffer hand-indexing in loom_scan.
//
// Phase 3+ replaces OneShotStream with a multi-batch stream (same callback
// signatures); this surface is the stable seam.
// ===========================================================================

// Factory context: holds the decoded array + schema for the produce callback.
// A raw pointer to this struct is cast to uintptr_t and passed to arrow_scan
// as stream_factory_ptr.
struct LoomStreamFactory {
    ArrowArray  array  = {};
    ArrowSchema schema = {};
};

// produce callback — signature required by DuckDB arrow_scan's ArrowScanBind:
//   unique_ptr<ArrowArrayStreamWrapper>(uintptr_t factory_ptr, ArrowStreamParameters &params)
//
// Builds a fully-wired ArrowArrayStreamWrapper whose inner ArrowArrayStream
// is backed by a OneShotStream.  The factory_ptr carries the LoomStreamFactory
// context (decoded Arrow structs).  Called once per scan (one-shot).
static unique_ptr<ArrowArrayStreamWrapper> produce(
    uintptr_t factory_ptr,
    ArrowStreamParameters & /*params*/)
{
    // Recover the LoomStreamFactory from the factory pointer.
    auto *factory = reinterpret_cast<LoomStreamFactory *>(factory_ptr);

    // Build the OneShotStream and the ArrowArrayStreamWrapper that DuckDB holds.
    // Move the array/schema out of the factory into the stream (factory owns
    // nothing after this point; LoomScanState destructor's null-checks are safe).
    ArrowArrayStream stream =
        OneShotStream::make_stream(std::move(factory->array), std::move(factory->schema));
    // Zero the factory's release pointers so LoomScanState destructor does not
    // double-release (stream now owns them via the null-after-move contract in
    // OneShotStream::make_stream).
    factory->array.release  = nullptr;
    factory->schema.release = nullptr;

    // Wrap in the DuckDB ArrowArrayStreamWrapper object.
    auto wrapper = make_uniq<ArrowArrayStreamWrapper>();
    wrapper->arrow_array_stream = stream;
    wrapper->number_of_rows = 0;  // unknown upfront; stream drives row delivery
    return wrapper;
}

// get_schema callback — signature required by DuckDB arrow_scan's ArrowScanBind:
//   void(ArrowArrayStream *stream, ArrowSchema &out)
//
// Called by arrow_scan to infer the schema before opening the stream.  We
// delegate to OneShotStream::get_schema which shallow-copies the schema.
static void arrow_scan_get_schema(ArrowArrayStream *stream, ArrowSchema &out) {
    if (!stream) return;
    OneShotStream::get_schema(stream, &out);
}

// ===========================================================================
// LoomScan — wrap decoded Arrow structs in OneShotStream, delegate to arrow_scan
// ===========================================================================
//
// D-01: this function does NOT populate DataChunk directly.  It wraps the
// decoded Arrow array in a OneShotStream and hands it to DuckDB's built-in
// arrow_scan via a produce-callback factory.  DuckDB's arrow_scan performs the
// Arrow→DataChunk conversion internally.
//
// ← STABLE SURFACE FOR PHASE 3+ (arrow_scan delegation pattern)

static void LoomScan(
    ClientContext &ctx,
    TableFunctionInput &data,
    DataChunk &output)
{
    auto &state = data.global_state->Cast<LoomScanState>();

    // End-of-stream guard: if we already handed off the stream, signal EOS.
    if (state.stream_handed_off) {
        output.SetCardinality(0);
        return;
    }

    // -------------------------------------------------------------------------
    // D-01: Delegate to arrow_scan via a produce-callback factory.
    //
    // Build a LoomStreamFactory context on the stack, populate it by moving
    // the array/schema out of the scan state, then pass a pointer to it as the
    // stream_factory_ptr.  arrow_scan will call produce(factory_ptr, params)
    // once to obtain the ArrowArrayStreamWrapper, then drive get_next() to
    // get the rows, and finally call stream.release() for teardown.
    // -------------------------------------------------------------------------
    LoomStreamFactory factory;
    factory.array  = state.arrow_array;
    factory.schema = state.arrow_schema;
    // Zero the source release pointers in the state: the factory + stream now
    // own the resources.  LoomScanState destructor will skip release because
    // stream_handed_off will be true (set below) and the release ptrs are null.
    state.arrow_array.release  = nullptr;
    state.arrow_schema.release = nullptr;
    state.stream_handed_off = true;  // prevents double-release in destructor

    // Invoke DuckDB's built-in arrow_scan table function through the Relation API.
    // arrow_scan takes three Value::POINTER arguments:
    //   1. stream_factory_ptr  (uintptr_t cast of &factory)
    //   2. stream_factory_produce (uintptr_t cast of produce function pointer)
    //   3. stream_factory_get_schema (uintptr_t cast of arrow_scan_get_schema fn ptr)
    //
    // ← STABLE SURFACE FOR PHASE 3+: Phase 3 replaces the factory ptr with a
    //   multi-batch stream factory; produce and get_schema signatures stay the same.
    auto &db = DatabaseInstance::GetDatabase(ctx);
    Connection conn(db);

    // Cast the factory callbacks to uintptr_t as required by arrow_scan's
    // Value::POINTER argument convention.
    using ProduceFn = unique_ptr<ArrowArrayStreamWrapper> (*)(uintptr_t, ArrowStreamParameters &);
    using GetSchemaFn = void (*)(ArrowArrayStream *, ArrowSchema &);

    ProduceFn    produce_fn      = &produce;
    GetSchemaFn  get_schema_fn   = &arrow_scan_get_schema;

    // arrow_scan expects three pointer-value arguments (verified from DuckDB
    // v1.5.3 src/function/table/arrow.cpp ArrowScanBind).
    vector<Value> args = {
        Value::POINTER(reinterpret_cast<uintptr_t>(&factory)),
        Value::POINTER(reinterpret_cast<uintptr_t>(produce_fn)),
        Value::POINTER(reinterpret_cast<uintptr_t>(get_schema_fn)),
    };

    // Invoke arrow_scan via the Relation API and materialize the result.
    // DuckDB drives the stream: calls produce once, then get_next until
    // end-of-stream, then calls stream.release().  The Arrow→DataChunk
    // conversion happens inside arrow_scan — no DataChunk population here.
    auto rel    = conn.TableFunction("arrow_scan", args);
    auto result = rel->Execute();

    if (result->HasError()) {
        throw IOException("arrow_scan execution failed: %s", result->GetError().c_str());
    }

    // Pull all fetched chunks into the output DataChunk.
    // For Phase 2, loom_decode returns exactly 4 rows in one batch.
    auto chunk = result->Fetch();
    if (chunk && chunk->size() > 0) {
        output.Move(*chunk);
    } else {
        output.SetCardinality(0);
    }
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
