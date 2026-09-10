# Structured DuckDB connections

This crate owns the native database and its Rust connection. JSON and FTS are
compiled with the same DuckDB headers, compiler and C++ runtime. Opening a store
registers FTS before exposing the connection, with automatic extension download
and loading disabled. No unsigned extension policy is enabled.

The pinned native version is DuckDB 1.5.5 (`duckdb` / `libduckdb-sys` 1.10505.0).
The C++ bridge uses that version's `DatabaseWrapper` definition from the headers
published by `libduckdb-sys`; it does not duplicate the struct layout. Updating
DuckDB requires rebuilding and passing the disk reopen, JSON, FTS, read-only and
ownership tests, followed by real Office ingestion.

The inner `duckdb::Connection` is deliberately private. `open_from_raw` borrows its
database, and exposing the connection's `try_clone` would allow a clone to outlive
our owner. Statements borrow the connection. Fields drop in order: Rust
connection, then native database. The small query surface delegates SQL execution
and parameter binding to duckdb-rs.

## Vendored upstream sources

- `vendor/fts`: [duckdb/duckdb-fts](https://github.com/duckdb/duckdb-fts/tree/6814ec9a7d5fd63500176507262b0dbf7cea0095/extension/fts),
  commit `6814ec9a7d5fd63500176507262b0dbf7cea0095`. This is the source revision in
  the official DuckDB 1.5.5 Windows FTS extension metadata. Original MIT license is
  retained in `vendor/fts/LICENSE`.
- `vendor/snowball`: [DuckDB v1.5.5](https://github.com/duckdb/duckdb/tree/v1.5.5/third_party/snowball).
  Original license is retained in `vendor/snowball/LICENSE`.
- Sources are unchanged. `vendor/SHA256SUMS` records the checked-in source files.
  Build scripts do not fetch code or binaries from the network.

Windows diagnosis showed that the official precompiled JSON / FTS extension
crashed in our MinGW native build. A same-compiler static FTS probe passed. The
product uses that static registration path and fails the write if index creation
fails, rather than silently producing a store without the index.

The Windows GNU backend linker (`scripts/link-windows-gnu.sh`, configured in
`avrag-rs/.cargo/config.toml`) resolves the selected compiler's static
`libstdc++.a`. A minimal C++ `std::call_once` callback invoked from a Rust thread
crashed with that compiler's Win32 DLL, and passed with its static library. This
also isolates the C++ callback/TLS state within the executable. The reproducer is
in `scripts/desktop-e2e/mingw-once-smoke.{cpp,rs}`; full JSON/FTS tests remain the
product gate. Other Windows system libraries retain their normal link behavior.
For executables written to WSL's Windows mount, the linker writes to a temporary
local directory and copies the completed executable to Cargo's destination. This
avoids PE link-time random writes across the Windows mount; failed links never
copy a partial executable. Cargo's arguments and the final filename are retained.
