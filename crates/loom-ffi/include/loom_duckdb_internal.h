/* DuckDB adapter internal header.
 *
 * This header is non-public and exists only for the DuckDB extension adapter.
 * It is not a frozen loom_runtime.h ABI and must not be documented as a user
 * surface. The stable public SQL API remains loom_scan(path).
 */

#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LoomDuckDbPlan LoomDuckDbPlan;
typedef struct LoomDuckDbPrepared LoomDuckDbPrepared;

typedef struct LoomDuckDbDiagnostic {
    const char *code;
    const char *path;
    const char *message;
} LoomDuckDbDiagnostic;

typedef struct LoomDuckDbNativeBuffer {
    const char *builder_id;
    const char *arrow_type;
    const uint8_t *value_ptr;
    uintptr_t value_len;
} LoomDuckDbNativeBuffer;

int32_t loom_duckdb_plan_create(const uint8_t *artifact_ptr,
                                uintptr_t artifact_len,
                                bool allow_interpreter_fallback,
                                bool use_test_native_facts,
                                LoomDuckDbPlan **out_plan);

int32_t loom_duckdb_plan_create_projected(const uint8_t *artifact_ptr,
                                          uintptr_t artifact_len,
                                          const uint32_t *projection_ptr,
                                          uintptr_t projection_len,
                                          bool allow_interpreter_fallback,
                                          bool use_test_native_facts,
                                          LoomDuckDbPlan **out_plan);

int32_t loom_duckdb_plan_destroy(LoomDuckDbPlan *plan);

int32_t loom_duckdb_plan_decision(const LoomDuckDbPlan *plan,
                                  const char **out_decision);

int32_t loom_duckdb_plan_cache_key(const LoomDuckDbPlan *plan,
                                   const char **out_cache_key);

int32_t loom_duckdb_plan_cache_input(const LoomDuckDbPlan *plan,
                                     const char **out_cache_input);

int32_t loom_duckdb_plan_diagnostic_count(const LoomDuckDbPlan *plan,
                                          uintptr_t *out_count);

int32_t loom_duckdb_plan_diagnostic(const LoomDuckDbPlan *plan,
                                    uintptr_t index,
                                    LoomDuckDbDiagnostic *out_diagnostic);

int32_t loom_duckdb_prepare_create(const LoomDuckDbPlan *plan,
                                   bool cancelled,
                                   LoomDuckDbPrepared **out_prepared);

int32_t loom_duckdb_prepare_destroy(LoomDuckDbPrepared *prepared);

int32_t loom_duckdb_prepare_status(const LoomDuckDbPrepared *prepared,
                                   const char **out_status);

int32_t loom_duckdb_prepare_route(const LoomDuckDbPrepared *prepared,
                                  const char **out_route);

int32_t loom_duckdb_prepare_diagnostic_count(const LoomDuckDbPrepared *prepared,
                                             uintptr_t *out_count);

int32_t loom_duckdb_prepare_diagnostic(const LoomDuckDbPrepared *prepared,
                                       uintptr_t index,
                                       LoomDuckDbDiagnostic *out_diagnostic);

int32_t loom_duckdb_prepare_native_buffer_count(const LoomDuckDbPrepared *prepared,
                                                uintptr_t *out_count);

int32_t loom_duckdb_prepare_native_buffer(const LoomDuckDbPrepared *prepared,
                                          uintptr_t index,
                                          LoomDuckDbNativeBuffer *out_buffer);

#ifdef __cplusplus
}
#endif
