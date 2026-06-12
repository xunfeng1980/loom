// loom_extension.cpp — Loom DuckDB extension
//
// Exports: loom_duckdb_cpp_init (via DUCKDB_CPP_EXTENSION_ENTRY macro)
// Registers: loom_scan(VARCHAR) — table function.
//
// Sidecar decode path: extracts a Loom sidecar overlay from a file via
// loom_sidecar_extract, evaluates the 4-gate routing decision via
// loom_sidecar_route (with host data), and — when the file routes LoomNative —
// decodes it through the production L2Core interpreter
// (loom_sidecar_decode_carray) and materializes the decoded columns as the
// table function's typed result rows via the Arrow C Data Interface.
//
// Files routing to HostNativeReader (or with no sidecar) yield a single
// VARCHAR `diagnostic` column instructing the caller to use DuckDB's native
// reader. Links only libloom_ffi.a (JIT/LLVM excluded — see CMakeLists.txt).
//
// Thread-safety: each query creates fresh bind+global state; no shared mutable
// state is used in this extension.

#define DUCKDB_EXTENSION_MAIN
#include "vendor/duckdb-src/duckdb.hpp"  // DuckDB v1.5.3 amalgamated header

extern "C" {
#include "../../crates/loom-ffi/include/loom.h"
}

#include <cstdint>
#include <cstddef>
#include <cstring>
#include <fstream>
#include <memory>
#include <string>
#include <vector>

using namespace duckdb;

static string CStringOrEmpty(const char *value) {
    return value == nullptr ? string() : string(value);
}

// Read an entire file into a byte buffer. Returns false on failure.
static bool ReadFileBytes(const string &path, std::vector<uint8_t> &out) {
    std::ifstream in(path, std::ios::binary | std::ios::ate);
    if (!in) {
        return false;
    }
    std::streamsize size = in.tellg();
    if (size < 0) {
        return false;
    }
    in.seekg(0, std::ios::beg);
    out.resize(static_cast<size_t>(size));
    if (size == 0) {
        return true;
    }
    return static_cast<bool>(in.read(reinterpret_cast<char *>(out.data()), size));
}

// Owns a decoded Arrow C struct array (schema + array); releases both via their
// C Data Interface release callbacks on destruction.
struct DecodedArrow {
    ArrowSchema schema;
    ArrowArray array;

    DecodedArrow() {
        schema.release = nullptr;
        array.release = nullptr;
    }
    ~DecodedArrow() {
        if (array.release) {
            array.release(&array);
        }
        if (schema.release) {
            schema.release(&schema);
        }
    }
    DecodedArrow(const DecodedArrow &) = delete;
    DecodedArrow &operator=(const DecodedArrow &) = delete;
};

// Map an Arrow C Data Interface format string to a DuckDB LogicalType.
// Covers the types the Loom interpreter currently emits.
static bool ArrowFormatToLogicalType(const char *format, LogicalType &out) {
    if (format == nullptr) {
        return false;
    }
    string f(format);
    if (f == "i") { out = LogicalType::INTEGER; return true; }   // int32
    if (f == "l") { out = LogicalType::BIGINT; return true; }    // int64
    if (f == "f") { out = LogicalType::FLOAT; return true; }     // float32
    if (f == "g") { out = LogicalType::DOUBLE; return true; }    // float64
    if (f == "b") { out = LogicalType::BOOLEAN; return true; }   // bool
    if (f == "u") { out = LogicalType::VARCHAR; return true; }   // utf8
    return false;
}

struct SidecarBindData : TableFunctionData {
    string file_path;
    // Fallback (non-loom-native) single-column diagnostic.
    bool loom_native = false;
    string diagnostic;
    // Loom-native decoded columns.
    shared_ptr<DecodedArrow> decoded;
    vector<LogicalType> col_types;
    idx_t row_count = 0;

    unique_ptr<FunctionData> Copy() const override {
        auto copy = make_uniq<SidecarBindData>();
        copy->file_path = file_path;
        copy->loom_native = loom_native;
        copy->diagnostic = diagnostic;
        copy->decoded = decoded;  // shared ownership
        copy->col_types = col_types;
        copy->row_count = row_count;
        return std::move(copy);
    }

    bool Equals(const FunctionData &other_p) const override {
        auto &other = other_p.Cast<SidecarBindData>();
        return file_path == other.file_path && decoded == other.decoded;
    }
};

struct SidecarScanState : GlobalTableFunctionState {
    idx_t row_offset = 0;     // loom-native: next row to emit
    bool diag_emitted = false;  // fallback: single row emitted

    idx_t MaxThreads() const override {
        return 1;
    }
};

// Read a bit from an Arrow validity bitmap (1 = valid). Null bitmap => valid.
static inline bool ArrowBitSet(const uint8_t *bitmap, int64_t index) {
    if (bitmap == nullptr) {
        return true;
    }
    return (bitmap[index >> 3] >> (index & 7)) & 1;
}

// Materialize `n` rows starting at `src_row` from one Arrow child array into a
// DuckDB output vector of the given logical type.
static void FillVector(Vector &vec, const LogicalType &type, const ArrowArray *child,
                       int64_t src_row, idx_t n) {
    const int64_t base = child->offset + src_row;
    const uint8_t *validity =
        child->n_buffers > 0 ? static_cast<const uint8_t *>(child->buffers[0]) : nullptr;
    auto &out_validity = FlatVector::Validity(vec);

    auto mark_nulls = [&](void) {
        if (child->null_count == 0 || validity == nullptr) {
            return;
        }
        for (idx_t i = 0; i < n; i++) {
            if (!ArrowBitSet(validity, base + static_cast<int64_t>(i))) {
                out_validity.SetInvalid(i);
            }
        }
    };

    switch (type.id()) {
    case LogicalTypeId::INTEGER: {
        auto src = static_cast<const int32_t *>(child->buffers[1]);
        auto dst = FlatVector::GetData<int32_t>(vec);
        for (idx_t i = 0; i < n; i++) dst[i] = src[base + static_cast<int64_t>(i)];
        mark_nulls();
        break;
    }
    case LogicalTypeId::BIGINT: {
        auto src = static_cast<const int64_t *>(child->buffers[1]);
        auto dst = FlatVector::GetData<int64_t>(vec);
        for (idx_t i = 0; i < n; i++) dst[i] = src[base + static_cast<int64_t>(i)];
        mark_nulls();
        break;
    }
    case LogicalTypeId::FLOAT: {
        auto src = static_cast<const float *>(child->buffers[1]);
        auto dst = FlatVector::GetData<float>(vec);
        for (idx_t i = 0; i < n; i++) dst[i] = src[base + static_cast<int64_t>(i)];
        mark_nulls();
        break;
    }
    case LogicalTypeId::DOUBLE: {
        auto src = static_cast<const double *>(child->buffers[1]);
        auto dst = FlatVector::GetData<double>(vec);
        for (idx_t i = 0; i < n; i++) dst[i] = src[base + static_cast<int64_t>(i)];
        mark_nulls();
        break;
    }
    case LogicalTypeId::BOOLEAN: {
        auto src = static_cast<const uint8_t *>(child->buffers[1]);  // bit-packed
        auto dst = FlatVector::GetData<bool>(vec);
        for (idx_t i = 0; i < n; i++) {
            dst[i] = ArrowBitSet(src, base + static_cast<int64_t>(i));
        }
        mark_nulls();
        break;
    }
    case LogicalTypeId::VARCHAR: {
        auto offsets = static_cast<const int32_t *>(child->buffers[1]);
        auto chars = static_cast<const char *>(child->buffers[2]);
        auto dst = FlatVector::GetData<string_t>(vec);
        for (idx_t i = 0; i < n; i++) {
            int64_t k = base + static_cast<int64_t>(i);
            int32_t start = offsets[k];
            int32_t end = offsets[k + 1];
            dst[i] = StringVector::AddString(vec, chars + start,
                                             static_cast<idx_t>(end - start));
        }
        mark_nulls();
        break;
    }
    default:
        // Unsupported materialization type — mark all rows null (fail-soft).
        for (idx_t i = 0; i < n; i++) out_validity.SetInvalid(i);
        break;
    }
}

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

    auto fallback = [&](const string &msg) {
        bind_data->loom_native = false;
        bind_data->diagnostic = msg;
        return_types.clear();
        names.clear();
        return_types.push_back(LogicalType::VARCHAR);
        names.push_back("diagnostic");
    };

    uint8_t *overlay_bytes = nullptr;
    uintptr_t overlay_len = 0;
    int32_t extract_rc = loom_sidecar_extract(bind_data->file_path.c_str(),
                                              &overlay_bytes, &overlay_len);

    if (extract_rc != 0 || overlay_bytes == nullptr || overlay_len == 0) {
        if (extract_rc == 5) {
            fallback("loom_scan[sidecar/NoSidecar]: no Loom sidecar overlay found. "
                     "Use DuckDB's native reader for this file.");
        } else {
            fallback("loom_scan[sidecar/extract-failed]: sidecar extraction failed "
                     "with code " + std::to_string(static_cast<int>(extract_rc)));
        }
        return std::move(bind_data);
    }

    // Read host bytes so routing can verify content-hash bindings.
    std::vector<uint8_t> host;
    bool host_ok = ReadFileBytes(bind_data->file_path, host);
    const uint8_t *host_ptr = host_ok && !host.empty() ? host.data() : nullptr;
    uintptr_t host_len = host_ok ? host.size() : 0;

    const char *decision_json = nullptr;
    int32_t route_rc = loom_sidecar_route(overlay_bytes, overlay_len,
                                          host_ptr, host_len, &decision_json);
    string decision = (route_rc == 0) ? CStringOrEmpty(decision_json) : string();
    if (decision_json != nullptr) {
        loom_sidecar_free_cstr(const_cast<char *>(decision_json));
    }

    bool is_loom_native = decision.find("\"decision\":\"LoomNative\"") != string::npos;

    if (!is_loom_native) {
        loom_sidecar_free_bytes(overlay_bytes, overlay_len);
        fallback("loom_scan[sidecar/host-native]: routed to host-native reader. "
                 "Use DuckDB's native reader for this file. (" + decision + ")");
        return std::move(bind_data);
    }

    // Loom-native: decode through the interpreter and export columns as an
    // Arrow C struct array.
    auto decoded = make_shared_ptr<DecodedArrow>();
    int32_t decode_rc = loom_sidecar_decode_carray(overlay_bytes, overlay_len,
                                                    host_ptr, host_len,
                                                    &decoded->schema, &decoded->array);
    loom_sidecar_free_bytes(overlay_bytes, overlay_len);

    if (decode_rc != 0 || decoded->schema.release == nullptr) {
        fallback("loom_scan[decode/failed]: loom_sidecar_decode_carray returned code " +
                 std::to_string(static_cast<int>(decode_rc)));
        return std::move(bind_data);
    }

    // The exported struct array's children are the output columns.
    int64_t n_children = decoded->schema.n_children;
    for (int64_t c = 0; c < n_children; c++) {
        const ArrowSchema *child = decoded->schema.children[c];
        LogicalType type;
        if (!ArrowFormatToLogicalType(child->format, type)) {
            fallback("loom_scan[decode/unsupported-type]: column '" +
                     CStringOrEmpty(child->name) + "' has Arrow format '" +
                     CStringOrEmpty(child->format) + "' not yet materializable");
            return std::move(bind_data);
        }
        bind_data->col_types.push_back(type);
        return_types.push_back(type);
        names.push_back(child->name != nullptr ? string(child->name)
                                               : ("col" + std::to_string(c)));
    }

    bind_data->loom_native = true;
    bind_data->row_count = static_cast<idx_t>(decoded->array.length);
    bind_data->decoded = decoded;
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
    auto &bind_data = data.bind_data->Cast<SidecarBindData>();

    if (!bind_data.loom_native) {
        // Fallback: emit the single diagnostic row once.
        if (state.diag_emitted) {
            output.SetCardinality(0);
            return;
        }
        output.SetCardinality(1);
        FlatVector::GetData<string_t>(output.data[0])[0] =
            StringVector::AddString(output.data[0], bind_data.diagnostic);
        state.diag_emitted = true;
        return;
    }

    // Loom-native: emit decoded rows in STANDARD_VECTOR_SIZE chunks.
    idx_t remaining = bind_data.row_count - state.row_offset;
    if (remaining == 0) {
        output.SetCardinality(0);
        return;
    }
    idx_t n = MinValue<idx_t>(remaining, STANDARD_VECTOR_SIZE);

    const ArrowArray &array = bind_data.decoded->array;
    for (idx_t c = 0; c < bind_data.col_types.size(); c++) {
        const ArrowArray *child = array.children[c];
        FillVector(output.data[c], bind_data.col_types[c], child,
                   static_cast<int64_t>(state.row_offset), n);
    }
    output.SetCardinality(n);
    state.row_offset += n;
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
extern "C" {
DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) {
    LoadInternal(loader);
}
}  // extern "C"
