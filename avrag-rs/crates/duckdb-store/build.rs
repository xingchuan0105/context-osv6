use std::{env, fs, path::Path};

fn main() {
    let mut build = cc::Build::new();
    // libduckdb-sys publishes the headers of the exact native build being linked.
    build.include(env::var("DEP_DUCKDB_INCLUDE").expect("DuckDB native headers"));
    build
        .include("vendor/fts/include")
        .include("vendor/snowball/libstemmer");
    build.file("src/fts.cpp");
    for dir in [
        "vendor/fts",
        "vendor/snowball/libstemmer",
        "vendor/snowball/runtime",
        "vendor/snowball/src_c",
    ] {
        let mut sources: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "cpp"))
            .collect();
        sources.sort();
        build.files(sources);
    }
    build
        .cpp(true)
        .define("DUCKDB_STATIC_BUILD", None)
        .define("EXT_VERSION_FTS", Some("\"6814ec9\""))
        .flag_if_supported("-std=c++11")
        .flag_if_supported("/utf-8")
        .flag_if_supported("/bigobj")
        .warnings(false);
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        build.flag("/EHsc");
    }
    if matches!(env::var("DEBUG").as_deref(), Ok("false" | "0")) {
        build.define("NDEBUG", None);
    }
    build.compile("avrag_duckdb_fts");
    for path in ["src/fts.cpp", "vendor", "build.rs"] {
        println!("cargo:rerun-if-changed={}", Path::new(path).display());
    }
}
