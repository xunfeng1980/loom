/* Phase 22 Loom runtime ABI sketch.
 *
 * This header is a contract sketch, not a frozen production ABI. It models the
 * host-neutral handles and callbacks that future runtime adapters should expose
 * after artifact verification and runtime planning. It intentionally avoids
 * host engine, compiler backend, and source-format types.
 */

#pragma once

#include <stdint.h>

typedef struct ArrowArray FFI_ArrowArray;
typedef struct ArrowSchema FFI_ArrowSchema;

typedef struct LoomRuntimePlan LoomRuntimePlan;
typedef struct LoomRuntimeScan LoomRuntimeScan;
typedef struct LoomRuntimeWorker LoomRuntimeWorker;
typedef struct LoomRuntimeBatch LoomRuntimeBatch;

typedef enum LoomRuntimeDecision {
    LOOM_RUNTIME_DECISION_NATIVE_CANDIDATE = 0,
    LOOM_RUNTIME_DECISION_INTERPRETER_FALLBACK = 1,
    LOOM_RUNTIME_DECISION_FAIL_CLOSED = 2,
    LOOM_RUNTIME_DECISION_DIAGNOSTIC_ONLY = 3,
} LoomRuntimeDecision;

typedef enum LoomRuntimeStatus {
    LOOM_RUNTIME_OK = 0,
    LOOM_RUNTIME_ERROR = 1,
    LOOM_RUNTIME_UNSUPPORTED = 2,
} LoomRuntimeStatus;

typedef struct LoomRuntimeAbiVersion {
    uint16_t major;
    uint16_t minor;
} LoomRuntimeAbiVersion;

typedef struct LoomRuntimeDiagnostic {
    const char *code;
    const char *path;
    const char *message;
} LoomRuntimeDiagnostic;

typedef struct LoomRuntimePlanRequest {
    LoomRuntimeAbiVersion abi_version;
    const uint8_t *artifact_bytes;
    uint64_t artifact_len;
    const char *artifact_digest;
    const char *facts_fingerprint;
    const char *cache_key;
} LoomRuntimePlanRequest;

LoomRuntimeStatus loom_runtime_plan_create(const LoomRuntimePlanRequest *request,
                                           LoomRuntimePlan **out_plan);

void loom_runtime_plan_destroy(LoomRuntimePlan *plan);

LoomRuntimeDecision loom_runtime_plan_decision(const LoomRuntimePlan *plan);

uint64_t loom_runtime_plan_diagnostic_count(const LoomRuntimePlan *plan);

LoomRuntimeStatus loom_runtime_plan_diagnostic(const LoomRuntimePlan *plan,
                                               uint64_t index,
                                               LoomRuntimeDiagnostic *out);

LoomRuntimeStatus loom_runtime_scan_open(LoomRuntimePlan *plan,
                                         uint64_t row_start,
                                         uint64_t row_end,
                                         LoomRuntimeScan **out_scan);

void loom_runtime_scan_close(LoomRuntimeScan *scan);

LoomRuntimeStatus loom_runtime_worker_open(LoomRuntimeScan *scan,
                                           uint32_t worker_index,
                                           LoomRuntimeWorker **out_worker);

void loom_runtime_worker_close(LoomRuntimeWorker *worker);

LoomRuntimeStatus loom_runtime_next_batch(LoomRuntimeWorker *worker,
                                          LoomRuntimeBatch **out_batch);

void loom_runtime_batch_release(LoomRuntimeBatch *batch);

LoomRuntimeStatus loom_runtime_batch_export_arrow(LoomRuntimeBatch *batch,
                                                  FFI_ArrowArray *out_array,
                                                  FFI_ArrowSchema *out_schema);
