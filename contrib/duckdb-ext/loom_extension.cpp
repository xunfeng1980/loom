// loom_extension.cpp — Loom DuckDB extension (Phase 2, extended Phase 51)
//
// Exports: loom_duckdb_cpp_init (via DUCKDB_CPP_EXTENSION_ENTRY macro)
// Registers: loom_scan(VARCHAR) — table function.
//
// Build modes (Phase 51):
//   LOOM_SIDECAR_ONLY=OFF (default): full Loom decode path — calls loom_decode
//     and populates DuckDB's DataChunk from decoded Arrow arrays, including
//     native codegen and Arrow semantic paths.
//   LOOM_SIDECAR_ONLY=ON: sidecar-only path — extracts Loom sidecar overlay from
//     Parquet files via loom_sidecar_extract, evaluates routing decisions via
//     loom_sidecar_route, and returns diagnostic information. Links only
//     libloom_sidecar_ffi.a (no container/codec/native-lowering dependencies).
//
// Thread-safety: each query creates a fresh state; no shared mutable
// state is used in this extension.

#define DUCKDB_EXTENSION_MAIN
#include "vendor/duckdb-src/duckdb.hpp"  // DuckDB v1.5.3 amalgamated header

#ifdef LOOM_SIDECAR_ONLY
extern "C" {
#include "../../crates/loom-sidecar-ffi/include/loom_sidecar.h"
}
#else
extern "C" {
#include "../../crates/loom-ffi/include/loom.h"  // Phase 1: loom_decode signature
#include "../../crates/loom-ffi/include/loom_duckdb_internal.h"
}
#endif

#include <cstdint>
#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <limits>
#include <memory>
#include <sstream>

using namespace duckdb;

// ===========================================================================
// Sidecar-only mode (LOOM_SIDECAR_ONLY=ON)
//
// In this mode the extension links only libloom_sidecar_ffi.a (zero container
// or native-lowering dependencies).  loom_scan(path) extracts any embedded Loom
// sidecar overlay from the Parquet file at `path`, evaluates the 4-gate routing
// decision, and returns a single VARCHAR diagnostic row describing the result.
//
// Full Arrow decode / native codegen / container codec paths are NOT available
// in this build — those require the full libloom_ffi.a (LOOM_SIDECAR_ONLY=OFF).
// ===========================================================================
#ifdef LOOM_SIDECAR_ONLY

#include <string>

static string CStringOrEmpty(const char *value) {
    return value == nullptr ? string() : string(value);
}

struct SidecarBindData : TableFunctionData {
    string file_path;
    string diagnostic;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<SidecarBindData>();
        copy->file_path = file_path;
        copy->diagnostic = diagnostic;
        return std::move(copy);
    }

    bool Equals(const FunctionData &other_p) const override {
        auto &other = other_p.Cast<SidecarBindData>();
        return file_path == other.file_path && diagnostic == other.diagnostic;
    }
};

struct SidecarScanState : GlobalTableFunctionState {
    bool batch_emitted = false;

    idx_t MaxThreads() const override {
        return 1;
    }
};

static unique_ptr<FunctionData> SidecarBind(
    ClientContext & /*ctx*/,
    TableFunctionBindInput &input,
    vector<LogicalType> &return_types,
    vector<string> &names)
{
    if (input.inputs.empty() || input.inputs[0].IsNull()) {
        throw BinderException("loom_scan requires a non-null file path");
    }

    auto bind_data = make_uniq<SidecarBindData>();
    bind_data->file_path = input.inputs[0].GetValue<string>();

    // Attempt sidecar extraction.
    uint8_t *overlay_bytes = nullptr;
    uintptr_t overlay_len = 0;
    int32_t extract_rc = loom_sidecar_extract(bind_data->file_path.c_str(),
                                              &overlay_bytes, &overlay_len);

    if (extract_rc == 0 && overlay_bytes != nullptr && overlay_len > 0) {
        // Sidecar found — evaluate routing decision.
        const char *decision_json = nullptr;
        int32_t route_rc = loom_sidecar_route(overlay_bytes, overlay_len,
                                              nullptr, 0, &decision_json);
        if (route_rc == 0 && decision_json != nullptr) {
            string decision = CStringOrEmpty(decision_json);
            loom_sidecar_free_cstr(const_cast<char *>(decision_json));
            if (decision.find("\"decision\":\"LoomNative\"") != string::npos) {
                bind_data->diagnostic =
                    "loom_scan[sidecar/LoomNative]: file has a Loom sidecar overlay "
                    "routing to LoomNative track. Full Arrow decode requires the "
                    "full loom_scan build (LOOM_SIDECAR_ONLY=OFF). "
                    "Use DuckDB's native Parquet reader for this file instead.";
            } else if (decision.find("\"decision\":\"HostNativeReader\"") != string::npos) {
                bind_data->diagnostic =
                    "loom_scan[sidecar/HostNativeReader]: Loom sidecar overlay found "
                    "and routed to HostNativeReader. Use DuckDB's native Parquet "
                    "reader for this file.";
            } else {
                bind_data->diagnostic =
                    "loom_scan[sidecar/unknown-route]: Loom sidecar overlay found "
                    "with unrecognized routing decision: " + decision;
            }
        } else {
            bind_data->diagnostic =
                "loom_scan[sidecar/route-failed]: Loom sidecar overlay found but "
                "routing evaluation failed with code " +
                std::to_string(static_cast<int>(route_rc));
        }

        // Free the overlay bytes allocated by loom_sidecar_extract.
        loom_sidecar_free_bytes(overlay_bytes, overlay_len);
    } else if (extract_rc == 5) {
        // NoSidecar — file has no embedded Loom sidecar.
        bind_data->diagnostic =
            "loom_scan[sidecar/NoSidecar]: no Loom sidecar overlay found in file. "
            "Use DuckDB's native Parquet reader for this file.";
    } else {
        // IoError, DecodeFailed, Panicked, or NullPointer.
        bind_data->diagnostic =
            "loom_scan[sidecar/extract-failed]: sidecar extraction failed with "
            "code " + std::to_string(static_cast<int>(extract_rc));
    }

    // Return a single VARCHAR column with the diagnostic message.
    return_types.push_back(LogicalType::VARCHAR);
    names.push_back("diagnostic");

    return std::move(bind_data);
}

static unique_ptr<GlobalTableFunctionState> SidecarInit(
    ClientContext & /*ctx*/,
    TableFunctionInitInput & /*input*/)
{
    return make_uniq<SidecarScanState>();
}

static void SidecarScan(
    ClientContext & /*ctx*/,
    TableFunctionInput &data,
    DataChunk &output)
{
    auto &state = data.global_state->Cast<SidecarScanState>();

    if (state.batch_emitted) {
        output.SetCardinality(0);
        return;
    }

    auto &bind_data = data.bind_data->Cast<SidecarBindData>();
    output.SetCardinality(1);
    FlatVector::GetData<string_t>(output.data[0])[0] =
        StringVector::AddString(output.data[0], bind_data.diagnostic);
    state.batch_emitted = true;
}

static void LoadInternal(ExtensionLoader &loader) {
    TableFunction fn(
        "loom_scan",
        {LogicalType::VARCHAR},
        SidecarScan,
        SidecarBind,
        SidecarInit);
    fn.projection_pushdown = false;
    loader.RegisterFunction(fn);
}

// ===========================================================================
// Full mode (LOOM_SIDECAR_ONLY=OFF) — existing implementation below.
// ===========================================================================
#else

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

struct LoomDuckDbArrowSemanticHolder {
    LoomDuckDbArrowSemantic *handle = nullptr;

    LoomDuckDbArrowSemanticHolder() = default;
    explicit LoomDuckDbArrowSemanticHolder(LoomDuckDbArrowSemantic *handle_p) : handle(handle_p) {
    }
    LoomDuckDbArrowSemanticHolder(const LoomDuckDbArrowSemanticHolder &) = delete;
    LoomDuckDbArrowSemanticHolder &operator=(const LoomDuckDbArrowSemanticHolder &) = delete;

    LoomDuckDbArrowSemanticHolder(LoomDuckDbArrowSemanticHolder &&other) noexcept : handle(other.handle) {
        other.handle = nullptr;
    }

    LoomDuckDbArrowSemanticHolder &operator=(LoomDuckDbArrowSemanticHolder &&other) noexcept {
        if (this != &other) {
            Reset();
            handle = other.handle;
            other.handle = nullptr;
        }
        return *this;
    }

    ~LoomDuckDbArrowSemanticHolder() {
        Reset();
    }

    void Reset() {
        if (handle != nullptr) {
            loom_duckdb_arrow_semantic_destroy(handle);
            handle = nullptr;
        }
    }

    LoomDuckDbArrowSemantic *Get() const {
        return handle;
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

static vector<LoomRouteDiagnostic> CollectPreparedDiagnostics(const LoomDuckDbPreparedHolder &holder) {
    uintptr_t count = 0;
    RequireDuckDbRuntimeOk(loom_duckdb_prepare_diagnostic_count(holder.Get(), &count),
                           "loom_duckdb_prepare_diagnostic_count");
    vector<LoomRouteDiagnostic> diagnostics;
    diagnostics.reserve(static_cast<idx_t>(count));
    for (uintptr_t i = 0; i < count; i++) {
        LoomDuckDbDiagnostic diagnostic {};
        RequireDuckDbRuntimeOk(loom_duckdb_prepare_diagnostic(holder.Get(), i, &diagnostic),
                               "loom_duckdb_prepare_diagnostic");
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

static string ReadPlanCacheInput(const LoomDuckDbPlanHolder &holder) {
    const char *cache_input = nullptr;
    RequireDuckDbRuntimeOk(loom_duckdb_plan_cache_input(holder.Get(), &cache_input),
                           "loom_duckdb_plan_cache_input");
    return CStringOrEmpty(cache_input);
}

static string ReadPreparedRoute(const LoomDuckDbPreparedHolder &holder) {
    const char *route = nullptr;
    RequireDuckDbRuntimeOk(loom_duckdb_prepare_route(holder.Get(), &route),
                           "loom_duckdb_prepare_route");
    return CStringOrEmpty(route);
}

static string FormatRouteDiagnostics(const vector<LoomRouteDiagnostic> &diagnostics) {
    if (diagnostics.empty()) {
        return "diagnostic code=none path=$ message=no route diagnostics";
    }

    std::ostringstream out;
    for (idx_t i = 0; i < diagnostics.size(); i++) {
        if (i > 0) {
            out << "; ";
        }
        out << "diagnostic code=" << diagnostics[i].code
            << " path=" << diagnostics[i].path
            << " message=" << diagnostics[i].message;
    }
    return out.str();
}

static string FormatRouteError(const char *reason,
                               const string &route,
                               const vector<LoomRouteDiagnostic> &diagnostics) {
    std::ostringstream out;
    out << "loom_scan[" << reason << "]: route=" << route << " "
        << FormatRouteDiagnostics(diagnostics);
    return out.str();
}

static bool TestEnvEnabled(const char *name, bool default_value = false) {
    const char *value = std::getenv(name);
    if (value == nullptr) {
        return default_value;
    }
    return string(value) == "1" || string(value) == "true" || string(value) == "yes";
}

static bool TestEnvDisabled(const char *name, bool default_value = false) {
    const char *value = std::getenv(name);
    if (value == nullptr) {
        return default_value;
    }
    return string(value) == "0" || string(value) == "false" || string(value) == "no";
}

static void AppendTestRouteReport(const char *phase,
                                  const string &route,
                                  const vector<LoomRouteDiagnostic> &diagnostics,
                                  const string &cache_key = string()) {
    const char *report_path = std::getenv("LOOM_DUCKDB_TEST_ROUTE_REPORT");
    if (report_path == nullptr || report_path[0] == '\0') {
        return;
    }

    std::ofstream out(report_path, std::ios::app);
    if (!out) {
        throw IOException("loom_scan: could not append LOOM_DUCKDB_TEST_ROUTE_REPORT");
    }
    out << phase << "\troute=" << route;
    if (!cache_key.empty()) {
        out << "\tcache_key=" << cache_key;
    }
    out << "\t" << FormatRouteDiagnostics(diagnostics) << "\n";
}

static std::shared_ptr<LoomDuckDbPlanHolder> CreateRuntimePlan(const vector<uint8_t> &payload,
                                                              bool allow_interpreter_fallback) {
    LoomDuckDbPlan *plan = nullptr;
    const auto *payload_ptr = payload.empty() ? nullptr : payload.data();
    RequireDuckDbRuntimeOk(
        loom_duckdb_plan_create(payload_ptr, payload.size(), allow_interpreter_fallback, &plan),
        "loom_duckdb_plan_create");
    return std::make_shared<LoomDuckDbPlanHolder>(plan);
}

static std::shared_ptr<LoomDuckDbPlanHolder> CreateProjectedRuntimePlan(const vector<uint8_t> &payload,
                                                                       const vector<idx_t> &projected_source_ids,
                                                                       bool allow_interpreter_fallback) {
    vector<uint32_t> projection;
    projection.reserve(projected_source_ids.size());
    for (auto source_id : projected_source_ids) {
        if (source_id > std::numeric_limits<uint32_t>::max()) {
            throw IOException("loom_scan[D-10/unsupported-projection]: diagnostic code=unsupported-projection path=$.projection.columns message=DuckDB projected column id %llu exceeds internal ABI width",
                              static_cast<unsigned long long>(source_id));
        }
        projection.push_back(static_cast<uint32_t>(source_id));
    }

    LoomDuckDbPlan *plan = nullptr;
    const auto *payload_ptr = payload.empty() ? nullptr : payload.data();
    const auto *projection_ptr = projection.empty() ? nullptr : projection.data();
    RequireDuckDbRuntimeOk(
        loom_duckdb_plan_create_projected(payload_ptr,
                                          payload.size(),
                                          projection_ptr,
                                          projection.size(),
                                          allow_interpreter_fallback,
                                          &plan),
        "loom_duckdb_plan_create_projected");
    return std::make_shared<LoomDuckDbPlanHolder>(plan);
}

static LoomDuckDbPreparedHolder CreatePreparedRoute(const LoomDuckDbPlanHolder &plan, bool cancelled) {
    LoomDuckDbPrepared *prepared = nullptr;
    RequireDuckDbRuntimeOk(loom_duckdb_prepare_create(plan.Get(), cancelled, &prepared),
                           "loom_duckdb_prepare_create");
    return LoomDuckDbPreparedHolder(prepared);
}

struct LoomBindData : TableFunctionData {
    string payload_path;
    vector<uint8_t> payload;
    vector<string> column_names;
    vector<LogicalType> column_types;
    vector<LoomValueKind> column_kinds;
    vector<vector<uint8_t>> column_payloads;
    bool arrow_semantic = false;
    bool allow_interpreter_fallback = false;
    std::shared_ptr<LoomDuckDbPlanHolder> runtime_plan;
    string route_decision;
    string route_cache_key;
    string route_cache_input;
    vector<LoomRouteDiagnostic> route_diagnostics;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<LoomBindData>();
        copy->payload_path = payload_path;
        copy->payload = payload;
        copy->column_names = column_names;
        copy->column_types = column_types;
        copy->column_kinds = column_kinds;
        copy->column_payloads = column_payloads;
        copy->arrow_semantic = arrow_semantic;
        copy->allow_interpreter_fallback = allow_interpreter_fallback;
        copy->runtime_plan = runtime_plan;
        copy->route_decision = route_decision;
        copy->route_cache_key = route_cache_key;
        copy->route_cache_input = route_cache_input;
        copy->route_diagnostics = route_diagnostics;
        return std::move(copy);
    }

    bool Equals(const FunctionData &other_p) const override {
        auto &other = other_p.Cast<LoomBindData>();
        return payload_path == other.payload_path && column_names == other.column_names &&
               column_types == other.column_types && column_kinds == other.column_kinds &&
               column_payloads == other.column_payloads &&
               arrow_semantic == other.arrow_semantic &&
               allow_interpreter_fallback == other.allow_interpreter_fallback &&
               route_decision == other.route_decision &&
               route_cache_key == other.route_cache_key &&
               route_cache_input == other.route_cache_input;
    }
};

struct LoomRuntimePlanSelection {
    std::shared_ptr<LoomDuckDbPlanHolder> runtime_plan;
    string route_decision;
    string route_cache_key;
    string route_cache_input;
    vector<LoomRouteDiagnostic> route_diagnostics;
    vector<idx_t> projected_source_ids;
    bool reused_bind_plan = true;
};

static vector<idx_t> AllSourceColumnIds(idx_t column_count) {
    vector<idx_t> ids;
    ids.reserve(column_count);
    for (idx_t i = 0; i < column_count; i++) {
        ids.push_back(i);
    }
    return ids;
}

static vector<idx_t> ProjectedSourceColumnIds(const TableFunctionInitInput &input, idx_t column_count) {
    // D-10: DuckDB exposes concrete projection ids to global init through
    // TableFunctionInitInput::column_ids once projection_pushdown is enabled.
    if (input.column_ids.size() == column_count) {
        bool all_columns = true;
        for (idx_t i = 0; i < column_count; i++) {
            if (input.column_ids[i] != static_cast<column_t>(i)) {
                all_columns = false;
                break;
            }
        }
        if (all_columns) {
            return AllSourceColumnIds(column_count);
        }
    }

    vector<idx_t> ids;
    ids.reserve(input.column_ids.size());
    for (auto column_id : input.column_ids) {
        if (column_id >= static_cast<column_t>(column_count)) {
            throw IOException("loom_scan[D-10/unsupported-projection]: diagnostic code=unsupported-projection path=$.projection.columns message=DuckDB projected column id %llu is outside %llu source columns",
                              static_cast<unsigned long long>(column_id),
                              static_cast<unsigned long long>(column_count));
        }
        ids.push_back(static_cast<idx_t>(column_id));
    }
    return ids;
}

static bool IsAllColumnProjection(const vector<idx_t> &ids, idx_t column_count) {
    if (ids.size() != column_count) {
        return false;
    }
    for (idx_t i = 0; i < column_count; i++) {
        if (ids[i] != i) {
            return false;
        }
    }
    return true;
}

static LoomRuntimePlanSelection BuildProjectedRuntimePlan(const LoomBindData &bind_data,
                                                          const TableFunctionInitInput &input) {
    auto projected_ids = ProjectedSourceColumnIds(input, bind_data.column_payloads.size());
    if (IsAllColumnProjection(projected_ids, bind_data.column_payloads.size())) {
        return {
            bind_data.runtime_plan,
            bind_data.route_decision,
            bind_data.route_cache_key,
            bind_data.route_cache_input,
            bind_data.route_diagnostics,
            std::move(projected_ids),
            true,
        };
    }

    auto projected_plan =
        CreateProjectedRuntimePlan(bind_data.payload, projected_ids, bind_data.allow_interpreter_fallback);
    auto route_decision = ReadPlanDecision(*projected_plan);
    auto route_cache_key = ReadPlanCacheKey(*projected_plan);
    auto route_cache_input = ReadPlanCacheInput(*projected_plan);
    auto route_diagnostics = CollectPlanDiagnostics(*projected_plan);
    return {
        projected_plan,
        std::move(route_decision),
        std::move(route_cache_key),
        std::move(route_cache_input),
        std::move(route_diagnostics),
        std::move(projected_ids),
        false,
    };
}

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

static bool IsArrowSemanticPayload(const vector<uint8_t> &payload) {
    return payload.size() >= 4 && payload[0] == 'L' && payload[1] == 'M' && payload[2] == 'A' && payload[3] == '1';
}

static bool IsArrowSemanticContainerPayload(const vector<uint8_t> &payload) {
    return payload.size() >= 4 && payload[0] == 'L' && payload[1] == 'M' && payload[2] == 'C' && payload[3] == '2';
}

static bool IsArrowSemanticArtifact(const vector<uint8_t> &payload) {
    return IsArrowSemanticPayload(payload) || IsArrowSemanticContainerPayload(payload);
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

static LoomValueKind PayloadKindFromArrowSchemaFormat(const char *format) {
    if (format == nullptr) {
        throw IOException("loom_scan: decoded Arrow semantic schema has null format");
    }
    if (std::strcmp(format, "b") == 0) {
        return LoomValueKind::BOOL;
    }
    if (std::strcmp(format, "i") == 0) {
        return LoomValueKind::I32;
    }
    if (std::strcmp(format, "l") == 0) {
        return LoomValueKind::I64;
    }
    if (std::strcmp(format, "u") == 0) {
        return LoomValueKind::UTF8;
    }
    if (std::strcmp(format, "f") == 0) {
        return LoomValueKind::F32;
    }
    if (std::strcmp(format, "g") == 0) {
        return LoomValueKind::F64;
    }
    throw IOException("loom_scan: unsupported Arrow semantic schema format '%s'", format);
}

static LoomDuckDbArrowSemanticHolder CreateArrowSemanticHandle(const vector<uint8_t> &payload) {
    LoomDuckDbArrowSemantic *handle = nullptr;
    const auto *payload_ptr = payload.empty() ? nullptr : payload.data();
    int32_t rc = loom_duckdb_arrow_semantic_create(payload_ptr, payload.size(), &handle);
    if (rc != 0) {
        throw IOException("loom_scan: failed to inspect Arrow semantic artifact with code %d",
                          static_cast<int>(rc));
    }
    return LoomDuckDbArrowSemanticHolder(handle);
}

static bool TryPopulateArrowSemanticColumnSpecs(LoomBindData &bind_data) {
    LoomDuckDbArrowSemantic *raw_handle = nullptr;
    const auto *payload_ptr = bind_data.payload.empty() ? nullptr : bind_data.payload.data();
    int32_t rc = loom_duckdb_arrow_semantic_create(payload_ptr, bind_data.payload.size(), &raw_handle);
    if (rc != 0) {
        if (IsArrowSemanticArtifact(bind_data.payload)) {
            throw IOException("loom_scan: failed to inspect Arrow semantic artifact with code %d",
                              static_cast<int>(rc));
        }
        return false;
    }

    LoomDuckDbArrowSemanticHolder handle(raw_handle);
    uintptr_t column_count = 0;
    RequireDuckDbRuntimeOk(loom_duckdb_arrow_semantic_column_count(handle.Get(), &column_count),
                           "loom_duckdb_arrow_semantic_column_count");
    if (column_count == 0) {
        throw IOException("loom_scan: Arrow semantic artifact has no columns");
    }

    bind_data.arrow_semantic = true;
    bind_data.column_names.reserve(static_cast<idx_t>(column_count));
    bind_data.column_types.reserve(static_cast<idx_t>(column_count));
    bind_data.column_kinds.reserve(static_cast<idx_t>(column_count));
    bind_data.column_payloads.reserve(static_cast<idx_t>(column_count));
    for (uintptr_t i = 0; i < column_count; i++) {
        const char *name = nullptr;
        const char *format = nullptr;
        RequireDuckDbRuntimeOk(loom_duckdb_arrow_semantic_column_name(handle.Get(), i, &name),
                               "loom_duckdb_arrow_semantic_column_name");
        RequireDuckDbRuntimeOk(loom_duckdb_arrow_semantic_column_format(handle.Get(), i, &format),
                               "loom_duckdb_arrow_semantic_column_format");
        auto kind = PayloadKindFromArrowSchemaFormat(format);
        bind_data.column_names.push_back(CStringOrEmpty(name));
        bind_data.column_kinds.push_back(kind);
        bind_data.column_types.push_back(LogicalTypeForKind(kind));
        bind_data.column_payloads.emplace_back();
    }
    return true;
}

static void PopulateColumnSpecs(LoomBindData &bind_data) {
    if (TryPopulateArrowSemanticColumnSpecs(bind_data)) {
        return;
    }

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
    vector<idx_t> projected_source_ids;
    idx_t output_column_count = 0;
    std::unique_ptr<LoomDuckDbPreparedHolder> prepared_route;
    string route_decision;
    string route_cache_key;
    string route_cache_input;
    vector<LoomRouteDiagnostic> route_diagnostics;
    vector<LoomDuckDbNativeBuffer> native_buffers;
    bool batch_emitted = false;      // true after the single batch is delivered

    idx_t MaxThreads() const override {
        return 1;
    }

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
    bind_data->allow_interpreter_fallback =
        TestEnvEnabled("LOOM_DUCKDB_ALLOW_INTERPRETER_FALLBACK", false);
    bind_data->runtime_plan =
        CreateRuntimePlan(bind_data->payload, bind_data->allow_interpreter_fallback);
    bind_data->route_decision = ReadPlanDecision(*bind_data->runtime_plan);
    bind_data->route_cache_key = ReadPlanCacheKey(*bind_data->runtime_plan);
    bind_data->route_cache_input = ReadPlanCacheInput(*bind_data->runtime_plan);
    bind_data->route_diagnostics = CollectPlanDiagnostics(*bind_data->runtime_plan);
    AppendTestRouteReport("bind",
                          bind_data->route_decision,
                          bind_data->route_diagnostics,
                          bind_data->route_cache_input);

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
    auto runtime_plan = BuildProjectedRuntimePlan(bind_data, input);

    state->projected_source_ids = runtime_plan.projected_source_ids;
    state->output_column_count = state->projected_source_ids.size();
    state->route_decision = runtime_plan.route_decision;
    state->route_cache_key = runtime_plan.route_cache_key;
    state->route_cache_input = runtime_plan.route_cache_input;
    state->route_diagnostics = runtime_plan.route_diagnostics;
    state->column_kinds.reserve(state->projected_source_ids.size());
    for (auto source_idx : state->projected_source_ids) {
        state->column_kinds.push_back(bind_data.column_kinds[source_idx]);
    }

    if (state->route_decision == "fail-closed" || state->route_decision == "diagnostic-only") {
        AppendTestRouteReport("init",
                              state->route_decision,
                              state->route_diagnostics,
                              state->route_cache_input);
        throw IOException("%s", FormatRouteError("D-07/fail-closed",
                                                 state->route_decision,
                                                 state->route_diagnostics).c_str());
    }

    if (state->route_decision == "native-candidate" && state->output_column_count > 0) {
        const bool cancelled = TestEnvEnabled("LOOM_DUCKDB_TEST_CANCEL_PREPARE", false);
        auto prepared = CreatePreparedRoute(*runtime_plan.runtime_plan, cancelled);
        auto prepared_route = ReadPreparedRoute(prepared);
        auto prepared_diagnostics = CollectPreparedDiagnostics(prepared);

        if (prepared_route == "native-candidate") {
            uintptr_t native_buffer_count = 0;
            RequireDuckDbRuntimeOk(loom_duckdb_prepare_native_buffer_count(prepared.Get(), &native_buffer_count),
                                   "loom_duckdb_prepare_native_buffer_count");
            state->native_buffers.reserve(static_cast<idx_t>(native_buffer_count));
            for (uintptr_t i = 0; i < native_buffer_count; i++) {
                LoomDuckDbNativeBuffer buffer {};
                RequireDuckDbRuntimeOk(loom_duckdb_prepare_native_buffer(prepared.Get(), i, &buffer),
                                       "loom_duckdb_prepare_native_buffer");
                state->native_buffers.push_back(buffer);
            }
            state->prepared_route = std::make_unique<LoomDuckDbPreparedHolder>(std::move(prepared));
            state->route_decision = prepared_route;
            state->route_diagnostics = std::move(prepared_diagnostics);
            AppendTestRouteReport("init",
                                  state->route_decision,
                                  state->route_diagnostics,
                                  state->route_cache_input);
            return state;
        }

        if (prepared_route == "interpreter-fallback" || prepared_route == "diagnostic-only") {
            state->route_decision = "interpreter-fallback";
            state->route_diagnostics = std::move(prepared_diagnostics);
            AppendTestRouteReport("init",
                                  state->route_decision,
                                  state->route_diagnostics,
                                  state->route_cache_input);
        } else if (prepared_route == "cancelled") {
            AppendTestRouteReport("init", prepared_route, prepared_diagnostics, state->route_cache_input);
            throw IOException("%s", FormatRouteError("D-09/cancelled",
                                                     prepared_route,
                                                     prepared_diagnostics).c_str());
        } else {
            AppendTestRouteReport("init", prepared_route, prepared_diagnostics, state->route_cache_input);
            throw IOException("%s", FormatRouteError("D-08/native-output-mismatch",
                                                     prepared_route,
                                                     prepared_diagnostics).c_str());
        }
    }

    auto decode_ids = state->projected_source_ids;
    if (decode_ids.empty() && !bind_data.column_payloads.empty()) {
        decode_ids.push_back(0);
    }
    state->arrow_arrays.resize(decode_ids.size());
    state->arrow_schemas.resize(decode_ids.size());

    if (bind_data.arrow_semantic) {
        auto handle = CreateArrowSemanticHandle(bind_data.payload);
        for (idx_t output_idx = 0; output_idx < decode_ids.size(); output_idx++) {
            const auto source_idx = decode_ids[output_idx];
            int32_t rc = loom_duckdb_arrow_semantic_export_column(
                handle.Get(),
                source_idx,
                reinterpret_cast<FFI_ArrowArray *>(&state->arrow_arrays[output_idx]),
                reinterpret_cast<FFI_ArrowSchema *>(&state->arrow_schemas[output_idx]));

            if (rc != 0) {
                throw IOException("loom_scan: failed to export Arrow semantic column %llu with code %d",
                                  static_cast<unsigned long long>(source_idx),
                                  static_cast<int>(rc));
            }
        }
        return state;
    }

    for (idx_t output_idx = 0; output_idx < decode_ids.size(); output_idx++) {
        const auto source_idx = decode_ids[output_idx];
        auto &payload = bind_data.column_payloads[source_idx];
        int32_t rc = loom_decode(payload.data(),
                                 payload.size(),
                                 reinterpret_cast<FFI_ArrowArray *>(&state->arrow_arrays[output_idx]),
                                 reinterpret_cast<FFI_ArrowSchema *>(&state->arrow_schemas[output_idx]));

        // PITFALLS P5 / panic-safety: check the return code BEFORE touching outputs.
        // On nonzero the output pointers contain uninitialized data — never use them.
        if (rc != 0) {
            throw IOException("loom_decode failed for column %llu with code %d",
                              static_cast<unsigned long long>(source_idx),
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

static bool NativeValueIsValid(const LoomDuckDbNativeBuffer &buffer, idx_t i, idx_t count);

template <class T>
static void FillFixedWidthNativeBytes(const LoomDuckDbNativeBuffer &buffer,
                                      Vector &vec,
                                      idx_t count,
                                      const char *kind) {
    if (buffer.value_ptr == nullptr) {
        throw IOException("loom_scan: native %s values buffer is null", kind);
    }
    const auto expected_len = count * sizeof(T);
    if (buffer.value_len != expected_len) {
        throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.buffers message=native %s buffer has %llu bytes, expected exactly %llu",
                          kind,
                          static_cast<unsigned long long>(buffer.value_len),
                          static_cast<unsigned long long>(expected_len));
    }

    auto *out_data = FlatVector::GetData<T>(vec);
    auto &validity = FlatVector::Validity(vec);
    for (idx_t i = 0; i < count; i++) {
        if (!NativeValueIsValid(buffer, i, count)) {
            validity.SetInvalid(i);
            continue;
        }
        T value;
        std::memcpy(&value, buffer.value_ptr + (i * sizeof(T)), sizeof(T));
        out_data[i] = value;
    }
}

static bool NativeValueIsValid(const LoomDuckDbNativeBuffer &buffer, idx_t i, idx_t count) {
    if (buffer.validity_ptr == nullptr || buffer.validity_len == 0) {
        return true;
    }
    const auto expected_len = (count + 7) / 8;
    if (buffer.validity_len != expected_len) {
        throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.buffers.validity message=native validity buffer has %llu bytes, expected exactly %llu",
                          static_cast<unsigned long long>(buffer.validity_len),
                          static_cast<unsigned long long>(expected_len));
    }
    return ((buffer.validity_ptr[i / 8] >> (i % 8)) & 1u) != 0u;
}

static void FillBooleanNativeBitmap(const LoomDuckDbNativeBuffer &buffer, Vector &vec, idx_t count) {
    const auto expected_len = (count + 7) / 8;
    if (buffer.value_ptr == nullptr) {
        throw IOException("loom_scan: native Boolean values buffer is null");
    }
    if (buffer.value_len != expected_len) {
        throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.buffers message=native Boolean buffer has %llu bytes, expected exactly %llu",
                          static_cast<unsigned long long>(buffer.value_len),
                          static_cast<unsigned long long>(expected_len));
    }
    auto *out_data = FlatVector::GetData<bool>(vec);
    auto &validity = FlatVector::Validity(vec);
    for (idx_t i = 0; i < count; i++) {
        if (!NativeValueIsValid(buffer, i, count)) {
            validity.SetInvalid(i);
            continue;
        }
        out_data[i] = ((buffer.value_ptr[i / 8] >> (i % 8)) & 1u) != 0u;
    }
}

static const char *NativeArrowTypeForKind(LoomValueKind kind) {
    switch (kind) {
    case LoomValueKind::BOOL:
        return "Boolean";
    case LoomValueKind::I32:
        return "Int32";
    case LoomValueKind::I64:
        return "Int64";
    case LoomValueKind::F32:
        return "Float32";
    case LoomValueKind::F64:
        return "Float64";
    case LoomValueKind::UTF8:
        throw IOException("loom_scan[D-12/unsupported-native-output]: diagnostic code=unsupported-native-output path=$.native.buffers message=native route returned unsupported DuckDB output type");
    }
    throw InternalException("unknown LoomValueKind");
}

static LogicalTypeId NativeDuckDbTypeForKind(LoomValueKind kind) {
    switch (kind) {
    case LoomValueKind::BOOL:
        return LogicalTypeId::BOOLEAN;
    case LoomValueKind::I32:
        return LogicalTypeId::INTEGER;
    case LoomValueKind::I64:
        return LogicalTypeId::BIGINT;
    case LoomValueKind::F32:
        return LogicalTypeId::FLOAT;
    case LoomValueKind::F64:
        return LogicalTypeId::DOUBLE;
    case LoomValueKind::UTF8:
        throw IOException("loom_scan[D-12/unsupported-native-output]: diagnostic code=unsupported-native-output path=$.native.buffers message=native route returned unsupported DuckDB output type");
    }
    throw InternalException("unknown LoomValueKind");
}

static const LoomDuckDbNativeBuffer &NativeBufferForOutput(const LoomScanState &state,
                                                          idx_t output_idx);

static idx_t NativeRowCount(const LoomScanState &state) {
    if (state.native_buffers.empty()) {
        return 0;
    }
    return static_cast<idx_t>(NativeBufferForOutput(state, 0).row_count);
}

static const LoomDuckDbNativeBuffer &NativeBufferForOutput(const LoomScanState &state,
                                                          idx_t output_idx) {
    const auto source_idx = state.projected_source_ids[output_idx];
    if (source_idx < state.native_buffers.size()) {
        return state.native_buffers[source_idx];
    }
    throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.buffers message=native buffer count does not match projected source columns");
}

static void FillNativeBufferIntoVector(const LoomDuckDbNativeBuffer &buffer,
                                       LoomValueKind kind,
                                       Vector &vec,
                                       idx_t count) {
    const auto *expected_arrow_type = NativeArrowTypeForKind(kind);
    if (buffer.arrow_type == nullptr || std::strcmp(buffer.arrow_type, expected_arrow_type) != 0) {
        throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.buffers.arrow_type message=native buffer Arrow type does not match projected DuckDB column kind");
    }
    const auto expected_duckdb_type = NativeDuckDbTypeForKind(kind);
    if (vec.GetType().id() != expected_duckdb_type) {
        throw IOException("loom_scan[D-08/native-output-mismatch]: diagnostic code=native-output-mismatch path=$.native.duckdb_type message=native buffer kind does not match DuckDB output vector type");
    }

    switch (kind) {
    case LoomValueKind::BOOL:
        FillBooleanNativeBitmap(buffer, vec, count);
        break;
    case LoomValueKind::I32:
        FillFixedWidthNativeBytes<int32_t>(buffer, vec, count, "Int32");
        break;
    case LoomValueKind::I64:
        FillFixedWidthNativeBytes<int64_t>(buffer, vec, count, "Int64");
        break;
    case LoomValueKind::F32:
        FillFixedWidthNativeBytes<float>(buffer, vec, count, "Float32");
        break;
    case LoomValueKind::F64:
        FillFixedWidthNativeBytes<double>(buffer, vec, count, "Float64");
        break;
    case LoomValueKind::UTF8:
        throw IOException("loom_scan[D-12/unsupported-native-output]: diagnostic code=unsupported-native-output path=$.native.buffers message=native route returned unsupported DuckDB output type");
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

    if (state.route_decision == "fail-closed" || state.route_decision == "diagnostic-only" ||
        state.route_decision == "cancelled") {
        AppendTestRouteReport("scan",
                              state.route_decision,
                              state.route_diagnostics,
                              state.route_cache_input);
        throw IOException("%s", FormatRouteError("D-08/no-row-emission",
                                                 state.route_decision,
                                                 state.route_diagnostics).c_str());
    }

    if (state.route_decision == "native-candidate") {
        if (state.native_buffers.empty()) {
            throw IOException("%s", FormatRouteError("D-12/native-claim-without-buffers",
                                                     state.route_decision,
                                                     state.route_diagnostics).c_str());
        }

        const auto count = NativeRowCount(state);
        for (idx_t col = 0; col < state.output_column_count; col++) {
            const auto &buffer = NativeBufferForOutput(state, col);
            FillNativeBufferIntoVector(buffer, state.column_kinds[col], output.data[col], count);
        }
        output.SetCardinality(count);
        state.batch_emitted = true;
        AppendTestRouteReport("scan",
                              state.route_decision,
                              state.route_diagnostics,
                              state.route_cache_input);
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

    for (idx_t col = 0; col < state.output_column_count; col++) {
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

    output.SetCardinality(count);

    // The array stays owned by LoomScanState and is released in ~LoomScanState()
    // on every teardown path (DUCK-03). We only mark the batch as delivered.
    state.batch_emitted = true;
    AppendTestRouteReport("scan",
                          state.route_decision,
                          state.route_diagnostics,
                          state.route_cache_input);
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

#endif  // LOOM_SIDECAR_ONLY
