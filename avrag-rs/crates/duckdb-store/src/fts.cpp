#include "duckdb/main/capi/capi_internal.hpp"
#include "fts_extension.hpp"
#include <cstring>

// Compiled against libduckdb-sys's exact headers and C++ runtime. No downloaded
// extension, unsigned-extension setting, or alternate database instance.
extern "C" int avrag_duckdb_load_fts(duckdb_database database, char **error) noexcept {
    try {
        auto wrapper = reinterpret_cast<duckdb::DatabaseWrapper *>(database);
        wrapper->database->LoadStaticExtension<duckdb::FtsExtension>();
        return 0;
    } catch (const std::exception &e) {
        auto size = std::strlen(e.what()) + 1;
        *error = static_cast<char *>(duckdb_malloc(size));
        if (*error) std::memcpy(*error, e.what(), size);
        return 1;
    } catch (...) {
        return 1;
    }
}
