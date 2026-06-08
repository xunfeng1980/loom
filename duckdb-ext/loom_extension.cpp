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
//   The original D-01 fed DuckDB via a one-shot Arrow stream + the built-in
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
#include "../crates/loom-ffi/include/loom_duckdb_internal.h"
}

#include <cstdint>
#include <cstddef>
#include <cstring>
#include <fstream>
#include <limits>
#include <memory>
#include <sstream>

using namespace duckdb;

enum class LoomValueKind : uint8_t {
    BOOL,
    I32,
    I64,
    UTF8,
    F32,
    F64,
};

static LogicalType LogicalTypeForKind(LoomValueKind kind);

static string CStringOrEmpty(const char *value) {
    return value == nullptr ? string() : string(value);
}

struct LoomRouteDiagnostic {
    string code;
    string path;
    string message;
};

struct LoomDuckDbPlanHolder {
    LoomDuckDbPlan *plan = nullptr;

    LoomDuckDbPlanHolder() = default;
    explicit LoomDuckDbPlanHolder(LoomDuckDbPlan *plan_p) : plan(plan_p) {
    }
    LoomDuckDbPlanHolder(const LoomDuckDbPlanHolder &) = delete;
    LoomDuckDbPlanHolder &operator=(const LoomDuckDbPlanHolder &) = delete;

    LoomDuckDbPlanHolder(LoomDuckDbPlanHolder &&other) noexcept : plan(other.plan) {
        other.plan = nullptr;
    }

    LoomDuckDbPlanHolder &operator=(LoomDuckDbPlanHolder &&other) noexcept {
        if (this != &other) {
            Reset();
            plan = other.plan;
            other.plan = nullptr;
        }
        return *this;
    }

    ~LoomDuckDbPlanHolder() {
        Reset();
    }

    void Reset() {
        if (plan != nullptr) {
            loom_duckdb_plan_destroy(plan);
            plan = nullptr;
        }
    }

    LoomDuckDbPlan *Get() const {
        return plan;
    }
};

struct LoomDuckDbPreparedHolder {
    LoomDuckDbPrepared *prepared = nullptr;

    LoomDuckDbPreparedHolder() = default;
    explicit LoomDuckDbPreparedHolder(LoomDuckDbPrepared *prepared_p) : prepared(prepared_p) {
    }
    LoomDuckDbPreparedHolder(const LoomDuckDbPreparedHolder &) = delete;
    LoomDuckDbPreparedHolder &operator=(const LoomDuckDbPreparedHolder &) = delete;

    LoomDuckDbPreparedHolder(LoomDuckDbPreparedHolder &&other) noexcept : prepared(other.prepared) {
        other.prepared = nullptr;
    }

    LoomDuckDbPreparedHolder &operator=(LoomDuckDbPreparedHolder &&other) noexcept {
        if (this != &other) {
            Reset();
            prepared = other.prepared;
            other.prepared = nullptr;
        }
        return *this;
    }

    ~LoomDuckDbPreparedHolder() {
        Reset();
    }

    void Reset() {
        if (prepared != nullptr) {
            loom_duckdb_prepare_destroy(prepared);
            prepared = nullptr;
        }
    }

    LoomDuckDbPrepared *Get() const {
        return prepared;
    }
};

static void RequireDuckDbRuntimeOk(int32_t status, const char *operation) {
    if (status != 0) {
        throw IOException("loom_scan: internal DuckDB runtime call %s failed with status %d",
                          operation,
                          static_cast<int>(status));
    }
}

static vector<LoomRouteDiagnostic> CollectPlanDiagnostics(const LoomDuckDbPlanHolder &holder) {
    uintptr_t count = 0;
    RequireDuckDbRuntimeOk(loom_duckdb_plan_diagnostic_count(holder.Get(), &count),
                           "loom_duckdb_plan_diagnostic_count");
    vector<LoomRouteDiagnostic> diagnostics;
    diagnostics.reserve(static_cast<idx_t>(count));
    for (uintptr_t i = 0; i < count; i++) {
        LoomDuckDbDiagnostic diagnostic {};
        RequireDuckDbRuntimeOk(loom_duckdb_plan_diagnostic(holder.Get(), i, &diagnostic),
                               "loom_duckdb_plan_diagnostic");
        diagnostics.push_back({
            CStringOrEmpty(diagnostic.code),
            CStringOrEmpty(diagnostic.path),
            CStringOrEmpty(diagnostic.message),
        });
    }
    return diagnostics;
}

static string ReadPlanDecision(const LoomDuckDbPlanHolder &holder) {
    const char *decision = nullptr;
    RequireDuckDbRuntimeOk(loom_duckdb_plan_decision(holder.Get(), &decision),
                           "loom_duckdb_plan_decision");
    return CStringOrEmpty(decision);
}

static string ReadPlanCacheKey(const LoomDuckDbPlanHolder &holder) {
    const char *cache_key = nullptr;
    RequireDuckDbRuntimeOk(loom_duckdb_plan_cache_key(holder.Get(), &cache_key),
                           "loom_duckdb_plan_cache_key");
    return CStringOrEmpty(cache_key);
}

static std::shared_ptr<LoomDuckDbPlanHolder> CreateRuntimePlan(const vector<uint8_t> &payload,
                                                              bool allow_interpreter_fallback) {
    LoomDuckDbPlan *plan = nullptr;
    const auto *payload_ptr = payload.empty() ? nullptr : payload.data();
    RequireDuckDbRuntimeOk(
        loom_duckdb_plan_create(payload_ptr, payload.size(), allow_interpreter_fallback, false, &plan),
        "loom_duckdb_plan_create");
    return std::make_shared<LoomDuckDbPlanHolder>(plan);
}

struct LoomBindData : TableFunctionData {
    string payload_path;
    vector<uint8_t> payload;
    vector<string> column_names;
    vector<LogicalType> column_types;
    vector<LoomValueKind> column_kinds;
    vector<vector<uint8_t>> column_payloads;
    std::shared_ptr<LoomDuckDbPlanHolder> runtime_plan;
    string route_decision;
    string route_cache_key;
    vector<LoomRouteDiagnostic> route_diagnostics;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<LoomBindData>();
        copy->payload_path = payload_path;
        copy->payload = payload;
        copy->column_names = column_names;
        copy->column_types = column_types;
        copy->column_kinds = column_kinds;
        copy->column_payloads = column_payloads;
        copy->runtime_plan = runtime_plan;
        copy->route_decision = route_decision;
        copy->route_cache_key = route_cache_key;
        copy->route_diagnostics = route_diagnostics;
        return std::move(copy);
    }

    bool Equals(const FunctionData &other_p) const override {
        auto &other = other_p.Cast<LoomBindData>();
        return payload_path == other.payload_path && column_names == other.column_names &&
               column_types == other.column_types && column_kinds == other.column_kinds &&
               column_payloads == other.column_payloads && route_decision == other.route_decision &&
               route_cache_key == other.route_cache_key;
    }
};

static vector<uint8_t> ReadPayloadFile(const string &path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        throw IOException("loom_scan: could not open payload file '%s'", path.c_str());
    }
    file.seekg(0, std::ios::end);
    auto size = file.tellg();
    if (size < 0) {
        throw IOException("loom_scan: could not determine payload size for '%s'", path.c_str());
    }
    file.seekg(0, std::ios::beg);
    vector<uint8_t> payload(static_cast<idx_t>(size));
    if (!payload.empty()) {
        file.read(reinterpret_cast<char *>(payload.data()), static_cast<std::streamsize>(payload.size()));
        if (!file) {
            throw IOException("loom_scan: failed to read payload file '%s'", path.c_str());
        }
    }
    return payload;
}

static LoomValueKind PayloadKindFromHeader(const vector<uint8_t> &payload) {
    if (payload.size() < 7 || payload[0] != 'L' || payload[1] != 'M' || payload[2] != 'P' || payload[3] != '1') {
        throw IOException("loom_scan: payload is not an LMP1 layout payload");
    }
    const auto version = static_cast<uint16_t>(payload[4]) | (static_cast<uint16_t>(payload[5]) << 8);
    if (version != 1) {
        throw IOException("loom_scan: unsupported LMP1 payload version %d", static_cast<int>(version));
    }
    switch (payload[6]) {
    case 1:
        return LoomValueKind::BOOL;
    case 2:
        return LoomValueKind::I32;
    case 3:
        return LoomValueKind::I64;
    case 4:
        return LoomValueKind::UTF8;
    case 5:
        return LoomValueKind::F32;
    case 6:
        return LoomValueKind::F64;
    default:
        throw IOException("loom_scan: unknown LMP1 data type tag %d", static_cast<int>(payload[6]));
    }
}

static uint16_t ReadU16LEAt(const vector<uint8_t> &payload, idx_t pos) {
    if (pos + 2 > payload.size()) {
        throw IOException("loom_scan: truncated payload while reading u16");
    }
    return static_cast<uint16_t>(payload[pos]) | (static_cast<uint16_t>(payload[pos + 1]) << 8);
}

static uint16_t ReadU16LE(const vector<uint8_t> &payload, idx_t &pos) {
    auto value = ReadU16LEAt(payload, pos);
    pos += 2;
    return value;
}

static uint32_t ReadU32LE(const vector<uint8_t> &payload, idx_t &pos) {
    if (pos + 4 > payload.size()) {
        throw IOException("loom_scan: truncated payload while reading u32");
    }
    uint32_t value = 0;
    for (idx_t i = 0; i < 4; i++) {
        value |= static_cast<uint32_t>(payload[pos + i]) << (8 * i);
    }
    pos += 4;
    return value;
}

static uint64_t ReadU64LE(const vector<uint8_t> &payload, idx_t &pos) {
    if (pos + 8 > payload.size()) {
        throw IOException("loom_scan: truncated payload while reading u64");
    }
    uint64_t value = 0;
    for (idx_t i = 0; i < 8; i++) {
        value |= static_cast<uint64_t>(payload[pos + i]) << (8 * i);
    }
    pos += 8;
    return value;
}

static vector<uint8_t> ReadBytes(const vector<uint8_t> &payload, idx_t &pos) {
    const auto len = ReadU64LE(payload, pos);
    if (len > payload.size() || pos + static_cast<idx_t>(len) > payload.size()) {
        throw IOException("loom_scan: truncated length-prefixed payload segment");
    }
    vector<uint8_t> out(payload.begin() + static_cast<std::ptrdiff_t>(pos),
                        payload.begin() + static_cast<std::ptrdiff_t>(pos + static_cast<idx_t>(len)));
    pos += static_cast<idx_t>(len);
    return out;
}

static string ReadString(const vector<uint8_t> &payload, idx_t &pos) {
    auto bytes = ReadBytes(payload, pos);
    if (bytes.empty()) {
        throw IOException("loom_scan: empty table column name");
    }
    return string(reinterpret_cast<const char *>(bytes.data()), bytes.size());
}

static bool IsTablePayload(const vector<uint8_t> &payload) {
    return payload.size() >= 4 && payload[0] == 'L' && payload[1] == 'M' && payload[2] == 'T' && payload[3] == '1';
}

static bool IsContainerPayload(const vector<uint8_t> &payload) {
    return payload.size() >= 4 && payload[0] == 'L' && payload[1] == 'M' && payload[2] == 'C' && payload[3] == '1';
}

static vector<uint8_t> SlicePayload(const vector<uint8_t> &payload, uint64_t offset, uint64_t len) {
    if (offset > payload.size() || len > payload.size() - offset) {
        throw IOException("loom_scan: LMC1 section is outside payload bounds");
    }
    return vector<uint8_t>(payload.begin() + static_cast<std::ptrdiff_t>(offset),
                           payload.begin() + static_cast<std::ptrdiff_t>(offset + len));
}

static vector<uint8_t> ExtractContainerPayload(const vector<uint8_t> &payload) {
    if (!IsContainerPayload(payload)) {
        return payload;
    }
    constexpr idx_t header_prefix_len = 28;
    constexpr idx_t section_entry_len = 28;
    constexpr uint64_t known_required_features = 0x1f;
    constexpr uint16_t section_required = 1;
    constexpr uint16_t section_schema = 1;
    constexpr uint16_t section_layout_payload = 2;
    constexpr uint16_t section_table_payload = 3;

    if (payload.size() < header_prefix_len) {
        throw IOException("loom_scan: truncated LMC1 header");
    }

    idx_t pos = 4;
    const auto version = ReadU16LE(payload, pos);
    if (version != 1) {
        throw IOException("loom_scan: unsupported LMC1 container version %d", static_cast<int>(version));
    }
    const auto header_len = ReadU16LE(payload, pos);
    const auto required_features = ReadU64LE(payload, pos);
    ReadU64LE(payload, pos); // optional features are advisory for bind.
    const auto section_count = ReadU32LE(payload, pos);

    if ((required_features & ~known_required_features) != 0) {
        throw IOException("loom_scan: LMC1 has unknown required features");
    }
    if (section_count > (std::numeric_limits<uint16_t>::max)()) {
        throw IOException("loom_scan: LMC1 has too many sections");
    }
    const auto expected_header_len = header_prefix_len + static_cast<idx_t>(section_count) * section_entry_len;
    if (header_len != expected_header_len || header_len > payload.size()) {
        throw IOException("loom_scan: malformed LMC1 section directory");
    }

    uint32_t schema_count = 0;
    uint32_t layout_count = 0;
    uint32_t table_count = 0;
    vector<uint8_t> wrapped;

    for (uint32_t i = 0; i < section_count; i++) {
        const auto kind = ReadU16LE(payload, pos);
        const auto flags = ReadU16LE(payload, pos);
        const auto offset = ReadU64LE(payload, pos);
        const auto len = ReadU64LE(payload, pos);
        ReadU32LE(payload, pos); // crc32 placeholder; Phase 11 v0 does not enforce it.
        ReadU32LE(payload, pos); // reserved.

        if (offset < header_len) {
            throw IOException("loom_scan: LMC1 section overlaps header");
        }
        if (kind > section_table_payload && (flags & section_required) != 0) {
            throw IOException("loom_scan: LMC1 has an unknown required section");
        }
        if (kind == section_schema) {
            schema_count++;
            continue;
        }
        if (kind == section_layout_payload || kind == section_table_payload) {
            if (!wrapped.empty()) {
                throw IOException("loom_scan: LMC1 has duplicate payload sections");
            }
            wrapped = SlicePayload(payload, offset, len);
            if (kind == section_layout_payload) {
                layout_count++;
            } else {
                table_count++;
            }
        }
    }

    if (pos != header_len || schema_count != 1 || layout_count + table_count != 1) {
        throw IOException("loom_scan: malformed LMC1 container shape");
    }
    return wrapped;
}

static void PopulateColumnSpecs(LoomBindData &bind_data) {
    auto bind_payload = ExtractContainerPayload(bind_data.payload);
    if (!IsTablePayload(bind_payload)) {
        bind_data.column_names.push_back("value");
        bind_data.column_kinds.push_back(PayloadKindFromHeader(bind_payload));
        bind_data.column_types.push_back(LogicalTypeForKind(bind_data.column_kinds.back()));
        bind_data.column_payloads.push_back(IsContainerPayload(bind_data.payload) ? bind_data.payload : bind_payload);
        return;
    }

    idx_t pos = 4;
    const auto version = ReadU16LE(bind_payload, pos);
    if (version != 1) {
        throw IOException("loom_scan: unsupported LMT1 table payload version %d", static_cast<int>(version));
    }
    ReadU64LE(bind_payload, pos); // row_count; Rust validation owns semantic checks.
    const auto column_count = ReadU64LE(bind_payload, pos);
    if (column_count == 0) {
        throw IOException("loom_scan: table payload has no columns");
    }

    for (uint64_t col = 0; col < column_count; col++) {
        auto name = ReadString(bind_payload, pos);
        auto payload = ReadBytes(bind_payload, pos);
        auto kind = PayloadKindFromHeader(payload);
        bind_data.column_names.push_back(std::move(name));
        bind_data.column_types.push_back(LogicalTypeForKind(kind));
        bind_data.column_kinds.push_back(kind);
        bind_data.column_payloads.push_back(std::move(payload));
    }

    if (pos != bind_payload.size()) {
        throw IOException("loom_scan: trailing bytes in LMT1 table payload");
    }
}

static LogicalType LogicalTypeForKind(LoomValueKind kind) {
    switch (kind) {
    case LoomValueKind::BOOL:
        return LogicalType::BOOLEAN;
    case LoomValueKind::I32:
        return LogicalType::INTEGER;
    case LoomValueKind::I64:
        return LogicalType::BIGINT;
    case LoomValueKind::UTF8:
        return LogicalType::VARCHAR;
    case LoomValueKind::F32:
        return LogicalType::FLOAT;
    case LoomValueKind::F64:
        return LogicalType::DOUBLE;
    }
    throw InternalException("unknown LoomValueKind");
}

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
    vector<ArrowArray> arrow_arrays;
    vector<ArrowSchema> arrow_schemas;
    vector<LoomValueKind> column_kinds;
    bool batch_emitted = false;      // true after the single batch is delivered

    ~LoomScanState() {
        // DUCK-03: release on ALL teardown paths. The array is never transferred
        // to a consumer (direct DataChunk copy), so we always own it here.
        for (auto &arrow_array : arrow_arrays) {
            if (arrow_array.release) {
                arrow_array.release(&arrow_array);
                arrow_array.release = nullptr;
            }
        }
        for (auto &arrow_schema : arrow_schemas) {
            if (arrow_schema.release) {
                arrow_schema.release(&arrow_schema);
                arrow_schema.release = nullptr;
            }
        }
    }
};

// ===========================================================================
// LoomBind — read payload path and declare the output schema
// ===========================================================================

static unique_ptr<FunctionData> LoomBind(
    ClientContext & /*ctx*/,
    TableFunctionBindInput &input,
    vector<LogicalType> &return_types,
    vector<string> &names)
{
    if (input.inputs.empty() || input.inputs[0].IsNull()) {
        throw BinderException("loom_scan requires a non-null payload file path");
    }

    auto bind_data = make_uniq<LoomBindData>();
    bind_data->payload_path = input.inputs[0].GetValue<string>();
    bind_data->payload = ReadPayloadFile(bind_data->payload_path);
    if (bind_data->payload.empty()) {
        throw IOException("loom_scan: payload file '%s' is empty", bind_data->payload_path.c_str());
    }
    PopulateColumnSpecs(*bind_data);
    bind_data->runtime_plan = CreateRuntimePlan(bind_data->payload, true);
    bind_data->route_decision = ReadPlanDecision(*bind_data->runtime_plan);
    bind_data->route_cache_key = ReadPlanCacheKey(*bind_data->runtime_plan);
    bind_data->route_diagnostics = CollectPlanDiagnostics(*bind_data->runtime_plan);

    for (idx_t i = 0; i < bind_data->column_names.size(); i++) {
        return_types.push_back(bind_data->column_types[i]);
        names.push_back(bind_data->column_names[i]);
    }

    return std::move(bind_data);
}

// ===========================================================================
// LoomInit — call loom_decode; check return code; store Arrow structs
// ===========================================================================

static unique_ptr<GlobalTableFunctionState> LoomInit(
    ClientContext & /*ctx*/,
    TableFunctionInitInput &input)
{
    auto state = make_uniq<LoomScanState>();
    auto &bind_data = input.bind_data->Cast<LoomBindData>();

    state->column_kinds = bind_data.column_kinds;
    state->arrow_arrays.resize(bind_data.column_payloads.size());
    state->arrow_schemas.resize(bind_data.column_payloads.size());

    for (idx_t i = 0; i < bind_data.column_payloads.size(); i++) {
        auto &payload = bind_data.column_payloads[i];
        int32_t rc = loom_decode(
            payload.data(),
            payload.size(),
            reinterpret_cast<FFI_ArrowArray *>(&state->arrow_arrays[i]),
            reinterpret_cast<FFI_ArrowSchema *>(&state->arrow_schemas[i]));

        // PITFALLS P5 / panic-safety: check the return code BEFORE touching outputs.
        // On nonzero the output pointers contain uninitialized data — never use them.
        if (rc != 0) {
            throw IOException("loom_decode failed for column %llu with code %d",
                              static_cast<unsigned long long>(i),
                              static_cast<int>(rc));
        }
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
static bool ArrowValueIsValid(const ArrowArray &arr, idx_t i) {
    const auto *validity_buf = static_cast<const uint8_t *>(arr.buffers[0]);
    if (validity_buf == nullptr) {
        return true;
    }
    return ((validity_buf[i / 8] >> (i % 8)) & 1u) != 0u;
}

static void RequireArrowBuffers(const ArrowArray &arr, int64_t min_buffers, const char *kind) {
    if (arr.buffers == nullptr || arr.n_buffers < min_buffers) {
        throw IOException(
            "loom_scan: decoded Arrow %s array has too few buffers (n_buffers=%lld)",
            kind,
            static_cast<long long>(arr.n_buffers));
    }
}

template <class T>
static void FillFixedWidthVector(const ArrowArray &arr, Vector &vec, idx_t count, const char *kind) {
    RequireArrowBuffers(arr, 2, kind);
    auto *out_data = FlatVector::GetData<T>(vec);
    auto &validity = FlatVector::Validity(vec);
    const auto *values_buf = static_cast<const T *>(arr.buffers[1]);
    if (values_buf == nullptr) {
        throw IOException("loom_scan: decoded Arrow %s values buffer is null", kind);
    }

    for (idx_t i = 0; i < count; i++) {
        if (!ArrowValueIsValid(arr, i)) {
            validity.SetInvalid(i);
            continue;
        }
        out_data[i] = values_buf[i];
    }
}

static void FillBooleanVector(const ArrowArray &arr, Vector &vec, idx_t count) {
    RequireArrowBuffers(arr, 2, "Boolean");
    auto *out_data = FlatVector::GetData<bool>(vec);
    auto &validity = FlatVector::Validity(vec);
    const auto *values_buf = static_cast<const uint8_t *>(arr.buffers[1]);
    if (values_buf == nullptr) {
        throw IOException("loom_scan: decoded Arrow Boolean values buffer is null");
    }

    for (idx_t i = 0; i < count; i++) {
        if (!ArrowValueIsValid(arr, i)) {
            validity.SetInvalid(i);
            continue;
        }
        out_data[i] = ((values_buf[i / 8] >> (i % 8)) & 1u) != 0u;
    }
}

static void FillUtf8Vector(const ArrowArray &arr, Vector &vec, idx_t count) {
    RequireArrowBuffers(arr, 3, "Utf8");
    auto *out_data = FlatVector::GetData<string_t>(vec);
    auto &validity = FlatVector::Validity(vec);
    const auto *offsets = static_cast<const int32_t *>(arr.buffers[1]);
    const auto *bytes = static_cast<const char *>(arr.buffers[2]);
    if (offsets == nullptr || bytes == nullptr) {
        throw IOException("loom_scan: decoded Arrow Utf8 offsets/data buffer is null");
    }

    for (idx_t i = 0; i < count; i++) {
        if (!ArrowValueIsValid(arr, i)) {
            validity.SetInvalid(i);
            continue;
        }
        const auto start = offsets[i];
        const auto end = offsets[i + 1];
        if (start < 0 || end < start) {
            throw IOException("loom_scan: decoded Arrow Utf8 offsets are invalid");
        }
        out_data[i] = StringVector::AddString(vec, bytes + start, static_cast<idx_t>(end - start));
    }
}

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

    if (state.arrow_arrays.empty()) {
        output.SetCardinality(0);
        state.batch_emitted = true;
        return;
    }

    const auto &arr = state.arrow_arrays[0];
    const idx_t count = static_cast<idx_t>(arr.length);  // = 4 for Phase 2

    if (count == 0) {
        output.SetCardinality(0);
        state.batch_emitted = true;
        return;
    }

    output.SetCardinality(count);

    for (idx_t col = 0; col < state.arrow_arrays.size(); col++) {
        const auto &col_arr = state.arrow_arrays[col];
        if (static_cast<idx_t>(col_arr.length) != count) {
            throw IOException("loom_scan: decoded column length mismatch");
        }
        auto &vec = output.data[col];
        switch (state.column_kinds[col]) {
        case LoomValueKind::BOOL:
            FillBooleanVector(col_arr, vec, count);
            break;
        case LoomValueKind::I32:
            FillFixedWidthVector<int32_t>(col_arr, vec, count, "Int32");
            break;
        case LoomValueKind::I64:
            FillFixedWidthVector<int64_t>(col_arr, vec, count, "Int64");
            break;
        case LoomValueKind::UTF8:
            FillUtf8Vector(col_arr, vec, count);
            break;
        case LoomValueKind::F32:
            FillFixedWidthVector<float>(col_arr, vec, count, "Float32");
            break;
        case LoomValueKind::F64:
            FillFixedWidthVector<double>(col_arr, vec, count, "Float64");
            break;
        }
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
    TableFunction fn(
        "loom_scan",
        {LogicalType::VARCHAR},
        LoomScan,
        LoomBind,
        LoomInit);
    fn.projection_pushdown = true;
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
