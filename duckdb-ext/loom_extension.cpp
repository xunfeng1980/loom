// loom_extension.cpp — Loom DuckDB extension (Phase 2)
//
// Exports: loom_duckdb_cpp_init (via DUCKDB_CPP_EXTENSION_ENTRY macro)
// Registers: loom_scan(VARCHAR) — table function that calls loom_decode and
//            populates DuckDB's DataChunk directly from the decoded Arrow array.
//
// Architecture:
//   loom_scan('file.bin')
//     └─ LoomBind  : declare output schema (INTEGER "value", nullable)
//     └─ LoomInit  : call loom_decode; check rc; store Arrow array+schema in state
//     └─ LoomScan  : populate the DataChunk directly from the Arrow buffers
//     └─ LoomScanState::~LoomScanState : release array+schema on every teardown path
//
// DECISION — D-01 (revised; see 02-CONTEXT.md "D-01 revised"):
//   The original D-01 fed DuckDB via a one-shot ArrowArrayStream + the built-in
//   arrow_scan. Execution surfaced a hard blocker: arrow_scan requires a
//   STRUCT/record-batch schema at the top level, but loom_decode emits a BARE
//   primitive Int32 array (format="i", n_children=0). Wrapping a single hardcoded
//   column in a record batch purely to satisfy arrow_scan is premature for this
//   plumbing stub, so D-01 was formally revised: Phase 2 populates the DataChunk
//   directly from the Arrow buffers. The arrow_scan / streaming path is DEFERRED
//   to Phase 3, where real columnar decode produces a record-batch-shaped output
//   that arrow_scan can consume (tracked: see STATE.md and 02-CONTEXT.md).
//   No dead "stable-surface" scaffolding is kept here — when Phase 3 needs the
//   stream path it will build it against the real record-batch output.
//
// Thread-safety: each query creates a fresh LoomScanState; no shared mutable
// state is used in this extension.

#define DUCKDB_EXTENSION_MAIN
#include "vendor/duckdb-src/duckdb.hpp"  // DuckDB v1.5.3 amalgamated header

extern "C" {
#include "../crates/loom-ffi/include/loom.h"  // Phase 1: loom_decode signature
}

#include <cstdint>

using namespace duckdb;

// ===========================================================================
// LoomScanState — GlobalTableFunctionState holding the decoded Arrow structs
// ===========================================================================
//
// Lifecycle: constructed in LoomInit (after loom_decode succeeds), held alive
// by DuckDB for the duration of the query, destructed when the query ends.
//
// Ownership / DUCK-03:
//   LoomScanState owns the Arrow array and schema for its whole lifetime.
//   LoomScan reads (does NOT transfer) the array, so the destructor releases
//   BOTH structs on EVERY teardown path (success, error, query cancel),
//   exactly once each, guarded by null checks to prevent double-free
//   (PITFALLS P1/P2).
//   `batch_emitted` is only the end-of-scan sentinel for the repeated-call
//   scan protocol; it does NOT affect ownership.

struct LoomScanState : GlobalTableFunctionState {
    ArrowArray  arrow_array  = {};   // zero-init: release == nullptr until populated
    ArrowSchema arrow_schema = {};
    bool batch_emitted = false;      // true after the single batch is delivered

    ~LoomScanState() {
        // DUCK-03: release on ALL teardown paths. The array is never transferred
        // to a consumer (direct DataChunk copy), so we always own it here.
        if (arrow_array.release) {
            arrow_array.release(&arrow_array);
            arrow_array.release = nullptr;
        }
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
    // [1, 2, 3, null] array. Phase 3+ will pass the path bytes to loom_decode.
    (void)input;

    // Phase 2 stub output schema: a single nullable INTEGER column "value".
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

    // PITFALLS P5 / panic-safety: check the return code BEFORE touching outputs.
    // On nonzero the output pointers contain uninitialized data — never use them.
    // (loom_decode wraps its body in catch_unwind, so a Rust panic arrives here
    // as a nonzero rc rather than aborting the DuckDB process — DUCK-04.)
    if (rc != 0) {
        throw IOException("loom_decode failed with code %d", static_cast<int>(rc));
    }

    return state;
}

// ===========================================================================
// LoomScan — populate the DataChunk directly from the decoded Arrow buffers
// ===========================================================================
//
// D-01 (revised): direct DataChunk population. loom_decode returns a bare Int32
// primitive array, which DuckDB's arrow_scan cannot consume (it needs a
// top-level struct/record-batch schema). For this single-column plumbing stub
// we read the Arrow buffers directly. The arrow_scan/stream path is deferred to
// Phase 3 (tracked) where the decoder emits a record-batch-shaped output.
//
// Arrow C Data Interface layout for a flat Int32Array with nulls:
//   buffers[0] = validity bitmap (one bit per element; 0 = null)
//   buffers[1] = int32 values buffer (one int32 per element)
// (verified by the Phase-1 Wave-0 buffer_layout test: n_buffers==2,
//  buffers[0]!=NULL with one null element, buffers[1]!=NULL.)

static void LoomScan(
    ClientContext & /*ctx*/,
    TableFunctionInput &data,
    DataChunk &output)
{
    auto &state = data.global_state->Cast<LoomScanState>();

    // Repeated-call scan protocol: once the single batch is delivered, signal
    // end-of-scan with an empty chunk. (Ownership is unaffected — the array is
    // released in ~LoomScanState regardless; DUCK-03.)
    if (state.batch_emitted) {
        output.SetCardinality(0);
        return;
    }

    const auto &arr = state.arrow_array;
    const idx_t count = static_cast<idx_t>(arr.length);  // = 4 for Phase 2

    if (count == 0) {
        output.SetCardinality(0);
        state.batch_emitted = true;
        return;
    }

    // Defensive guards (CR-01/WR-03): the Arrow C Data Interface permits a null
    // `buffers` pointer in degenerate cases, and a primitive array must expose at
    // least [validity, values]. The Phase-2 hardcoded array always satisfies this
    // (pinned by the Wave-0 buffer_layout test), but guard so the pattern is safe
    // when Phase 3 feeds real decoded buffers here.
    if (arr.buffers == nullptr || arr.n_buffers < 2) {
        throw IOException(
            "loom_scan: decoded Arrow array has no value buffer (n_buffers=%lld)",
            static_cast<long long>(arr.n_buffers));
    }

    output.SetCardinality(count);
    auto &vec      = output.data[0];
    auto *out_data = FlatVector::GetData<int32_t>(vec);
    auto &validity = FlatVector::Validity(vec);

    const auto *validity_buf = static_cast<const uint8_t *>(arr.buffers[0]);
    const auto *values_buf   = static_cast<const int32_t *>(arr.buffers[1]);

    // The values buffer is required for a non-empty Int32 array.
    if (values_buf == nullptr) {
        throw IOException("loom_scan: decoded Arrow array values buffer is null");
    }

    for (idx_t i = 0; i < count; i++) {
        if (validity_buf != nullptr) {
            // Arrow validity bitmap: bit i set (1) = valid, clear (0) = null.
            const bool valid = ((validity_buf[i / 8] >> (i % 8)) & 1u) != 0u;
            if (!valid) {
                validity.SetInvalid(i);
                continue;  // leave the value slot untouched for the null
            }
        }
        out_data[i] = values_buf[i];
    }

    // The array stays owned by LoomScanState and is released in ~LoomScanState()
    // on every teardown path (DUCK-03). We only mark the batch as delivered.
    state.batch_emitted = true;
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
// DuckDB dlsym's "loom_duckdb_cpp_init" (extension_load.cpp):
//   auto init_fun_name = filebase + "_duckdb_cpp_init"  →  "loom_duckdb_cpp_init"
// DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) expands to that exported symbol.
// Do NOT export legacy DuckDB 0.x init/version symbols — 1.5.3 ignores them.
// ===========================================================================

extern "C" {
DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) {
    LoadInternal(loader);
}
}  // extern "C"
