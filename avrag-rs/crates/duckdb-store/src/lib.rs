//! Structured-store native connection ownership and statically compiled FTS.
//!
//! The underlying borrowed duckdb-rs Connection stays private: exposing it via
//! Deref would let try_clone() escape the lifetime of the native database owner.
use anyhow::{Context, Result, bail};
use libduckdb_sys as ffi;
use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
    path::Path,
    ptr,
};

unsafe extern "C" {
    fn avrag_duckdb_load_fts(database: ffi::duckdb_database, error: *mut *mut c_char) -> i32;
}

struct Database(ffi::duckdb_database);
// Like duckdb-rs's DatabaseHandle, this exclusively owned handle may move to
// another thread. There is no concurrent access and no unguarded clone API.
unsafe impl Send for Database {}
impl Drop for Database {
    fn drop(&mut self) {
        unsafe { ffi::duckdb_close(&mut self.0) }
    }
}

pub struct Connection {
    // Rust drops fields in declaration order: disconnect before closing the DB.
    inner: duckdb::Connection,
    _database: Database,
}

impl Connection {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_mode(Some(path.as_ref()), false)
    }
    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_mode(Some(path.as_ref()), true)
    }
    pub fn open_in_memory() -> Result<Self> {
        Self::open_mode(None, false)
    }

    fn open_mode(path: Option<&Path>, readonly: bool) -> Result<Self> {
        let path = path
            .map(|p| -> Result<CString> {
                Ok(CString::new(
                    p.to_str().context("DuckDB path is not UTF-8")?,
                )?)
            })
            .transpose()?;
        unsafe {
            let mut config = ptr::null_mut();
            if ffi::duckdb_create_config(&mut config) != ffi::DuckDBSuccess {
                bail!("DuckDB config creation failed");
            }
            let mut config_ok = true;
            for (name, value) in [
                (c"autoinstall_known_extensions", c"false"),
                (c"autoload_known_extensions", c"false"),
            ] {
                config_ok &= ffi::duckdb_set_config(config, name.as_ptr(), value.as_ptr())
                    == ffi::DuckDBSuccess;
            }
            if readonly {
                config_ok &=
                    ffi::duckdb_set_config(config, c"access_mode".as_ptr(), c"READ_ONLY".as_ptr())
                        == ffi::DuckDBSuccess;
            }
            if !config_ok {
                ffi::duckdb_destroy_config(&mut config);
                bail!("DuckDB configuration failed");
            }
            let mut raw = ptr::null_mut();
            let mut error = ptr::null_mut();
            let state = ffi::duckdb_open_ext(
                path.as_ref().map_or(ptr::null(), |p| p.as_ptr()),
                &mut raw,
                config,
                &mut error,
            );
            ffi::duckdb_destroy_config(&mut config);
            if state != ffi::DuckDBSuccess {
                bail!("DuckDB open failed: {}", take_error(error));
            }
            let database = Database(raw);
            if avrag_duckdb_load_fts(raw, &mut error) != 0 {
                bail!("DuckDB FTS initialization failed: {}", take_error(error));
            }
            // This connection borrows raw. Database is retained until after its
            // destructor; prepared statements borrow inner and cannot outlive it.
            let inner = duckdb::Connection::open_from_raw(raw)?;
            Ok(Self {
                inner,
                _database: database,
            })
        }
    }

    pub fn execute_batch(&self, sql: &str) -> duckdb::Result<()> {
        self.inner.execute_batch(sql)
    }
    pub fn execute<P: duckdb::Params>(&self, sql: &str, params: P) -> duckdb::Result<usize> {
        self.inner.execute(sql, params)
    }
    pub fn prepare(&self, sql: &str) -> duckdb::Result<duckdb::Statement<'_>> {
        self.inner.prepare(sql)
    }
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> duckdb::Result<T>
    where
        P: duckdb::Params,
        F: FnOnce(&duckdb::Row<'_>) -> duckdb::Result<T>,
    {
        self.inner.query_row(sql, params, f)
    }
}

unsafe fn take_error(error: *mut c_char) -> String {
    if error.is_null() {
        return "unknown native error".into();
    }
    let text = unsafe { CStr::from_ptr(error) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::duckdb_free(error.cast()) };
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_and_fts_persist_without_extension_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("办公资料.duckdb");
        {
            let con = Connection::open(&path).unwrap();
            con.execute_batch("CREATE TABLE docs(id INTEGER, body VARCHAR, checks JSON); INSERT INTO docs VALUES (1, 'searchable sample', '[]');").unwrap();
            // FTS expands its PRAGMA while statements are extracted, so the
            // referenced table must already exist (as in the product writer).
            con.execute_batch("PRAGMA create_fts_index('docs', 'id', 'body')")
                .unwrap();
            let count: i64 = con.query_row("SELECT count(*) FROM duckdb_extensions() WHERE extension_name IN ('fts', 'json') AND loaded AND install_mode='STATICALLY_LINKED'", [], |r| r.get(0)).unwrap();
            assert_eq!(count, 2);
        }
        let con = Connection::open_readonly(&path).unwrap();
        con.execute_batch("SET enable_external_access=false; SET lock_configuration=true")
            .unwrap();
        let count: i64 = con.query_row("SELECT count(*) FROM docs WHERE fts_main_docs.match_bm25(id, 'searchable') IS NOT NULL AND json_array_length(checks)=0", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        assert!(
            con.execute_batch("CREATE TABLE denied(id INTEGER)")
                .is_err()
        );
        assert!(
            con.execute_batch("SET autoinstall_known_extensions=true")
                .is_err()
        );
    }

    #[test]
    fn native_ownership_survives_thread_move_and_repeated_close() {
        for _ in 0..8 {
            let con = Connection::open_in_memory().unwrap();
            std::thread::spawn(move || {
                assert_eq!(
                    con.query_row("SELECT 42", [], |r| r.get::<_, i32>(0))
                        .unwrap(),
                    42
                );
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn missing_readonly_database_fails_without_creating_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.duckdb");
        assert!(Connection::open_readonly(&path).is_err());
        assert!(!path.exists());
    }
}
