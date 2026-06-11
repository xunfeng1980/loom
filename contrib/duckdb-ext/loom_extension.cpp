// loom_extension.cpp — Loom DuckDB extension (Phase 2, Phase 51, Phase 101)
//
// Exports: loom_duckdb_cpp_init (via DUCKDB_CPP_EXTENSION_ENTRY macro)
// Registers: loom_scan(VARCHAR) — table function.
//
// Phase 101: sidecar-only path — extracts Loom sidecar overlay from Parquet
// files via loom_sidecar_extract, evaluates routing decisions via
// loom_sidecar_route, and returns diagnostic information. Links only
// libloom_sidecar_ffi.a (no container/codec/native-lowering dependencies).
//
// Thread-safety: each query creates a fresh state; no shared mutable
// state is used in this extension.

#define DUCKDB_EXTENSION_MAIN
#include "vendor/duckdb-src/duckdb.hpp"  // DuckDB v1.5.3 amalgamated header

extern "C" {
#include "../../crates/loom-ffi/include/loom.h"
}

#include <cstdint>
#include <cstddef>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <limits>
#include <memory>
#include <sstream>
#include <string>

using namespace duckdb;

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

    uint8_t *overlay_bytes = nullptr;
    uintptr_t overlay_len = 0;
    int32_t extract_rc = loom_sidecar_extract(bind_data->file_path.c_str(),
                                              &overlay_bytes, &overlay_len);

    if (extract_rc == 0 && overlay_bytes != nullptr && overlay_len > 0) {
        const char *decision_json = nullptr;
        int32_t route_rc = loom_sidecar_route(overlay_bytes, overlay_len,
                                              nullptr, 0, &decision_json);
        if (route_rc == 0 && decision_json != nullptr) {
            string decision = CStringOrEmpty(decision_json);
            loom_sidecar_free_cstr(const_cast<char *>(decision_json));
            if (decision.find("\"decision\":\"LoomNative\"") != string::npos) {
                bind_data->diagnostic =
                    "loom_scan[sidecar/LoomNative]: file has a Loom sidecar overlay "
                    "routing to LoomNative track. Use DuckDB's native Parquet reader "
                    "for this file instead.";
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

        loom_sidecar_free_bytes(overlay_bytes, overlay_len);
    } else if (extract_rc == 5) {
        bind_data->diagnostic =
            "loom_scan[sidecar/NoSidecar]: no Loom sidecar overlay found in file. "
            "Use DuckDB's native Parquet reader for this file.";
    } else {
        bind_data->diagnostic =
            "loom_scan[sidecar/extract-failed]: sidecar extraction failed with "
            "code " + std::to_string(static_cast<int>(extract_rc));
    }

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

// Extension entry point — exported C symbol looked up by DuckDB v1.5.3.
// DuckDB dlsym's "loom_duckdb_cpp_init" (extension_load.cpp):
//   auto init_fun_name = filebase + "_duckdb_cpp_init"  →  "loom_duckdb_cpp_init"
// DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) expands to that exported symbol.
extern "C" {
DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) {
    LoadInternal(loader);
}
}  // extern "C"
