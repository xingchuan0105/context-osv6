// Diagnostic probe for the exact statically linked Windows DuckDB build.
// No API, credentials, providers or desktop operations.
#include <windows.h>
#include <cstdio>
#include "duckdb.h"

static LONG WINAPI crashed(EXCEPTION_POINTERS *exception) {
    std::fprintf(stderr, "EXCEPTION code=%lx address=%p module=%p\n",
        exception->ExceptionRecord->ExceptionCode,
        exception->ExceptionRecord->ExceptionAddress, GetModuleHandleW(nullptr));
    void *frames[48];
    USHORT count = CaptureStackBackTrace(0, 48, frames, nullptr);
    for (USHORT i = 0; i < count; ++i) std::fprintf(stderr, "FRAME %p\n", frames[i]);
    std::fflush(stderr);
    return EXCEPTION_EXECUTE_HANDLER;
}

int main() {
    SetUnhandledExceptionFilter(crashed);
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    duckdb_database database = nullptr;
    duckdb_connection connection = nullptr;
    duckdb_config config = nullptr;
    if (duckdb_create_config(&config) != DuckDBSuccess) return 1;
    // Acceptance must use staged dependencies; do not download into a user cache.
    if (duckdb_set_config(config, "autoinstall_known_extensions", "false") != DuckDBSuccess) return 1;
    if (duckdb_set_config(config, "extension_directory", "./probe-extensions") != DuckDBSuccess) return 1;
    std::fprintf(stderr, "OPEN %s\n", duckdb_library_version());
    if (duckdb_open_ext(nullptr, &database, config, nullptr) != DuckDBSuccess) return 1;
    duckdb_destroy_config(&config);
    std::fprintf(stderr, "CONNECT\n");
    if (duckdb_connect(database, &connection) != DuckDBSuccess) return 2;
    std::fprintf(stderr, "QUERY\n");
    duckdb_result result;
    if (duckdb_query(connection, "SELECT 42", &result) != DuckDBSuccess) return 3;
    if (duckdb_value_int64(&result, 0, 0) != 42) return 4;
    duckdb_destroy_result(&result);
    std::fprintf(stderr, "JSON TABLE\n");
    if (duckdb_query(connection, "CREATE TABLE meta(checks JSON, notes JSON)", &result) != DuckDBSuccess) {
        std::fprintf(stderr, "JSON ERROR %s\n", duckdb_result_error(&result));
        return 5;
    }
    duckdb_destroy_result(&result);
    std::fprintf(stderr, "JSON INSERT\n");
    if (duckdb_query(connection, "INSERT INTO meta VALUES ('[]', '[]')", &result) != DuckDBSuccess) return 6;
    duckdb_destroy_result(&result);
    std::fprintf(stderr, "FTS TABLE/INDEX\n");
    if (duckdb_query(connection, "CREATE TABLE docs(id INTEGER, body VARCHAR); INSERT INTO docs VALUES (1, 'searchable sample'); PRAGMA create_fts_index('docs', 'id', 'body');", &result) != DuckDBSuccess) {
        std::fprintf(stderr, "FTS ERROR %s\n", duckdb_result_error(&result));
        return 7;
    }
    duckdb_destroy_result(&result);
    if (duckdb_query(connection, "SELECT COUNT(*) FROM docs WHERE fts_main_docs.match_bm25(id, 'searchable') IS NOT NULL", &result) != DuckDBSuccess) return 8;
    if (duckdb_value_int64(&result, 0, 0) != 1) return 9;
    duckdb_destroy_result(&result);
    duckdb_disconnect(&connection);
    duckdb_close(&database);
    std::fprintf(stderr, "PASS open/query/json/fts/close\n");
}
