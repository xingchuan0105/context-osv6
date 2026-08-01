fn main() {
    let path = "/tmp/fts_rust_probe.duckdb";
    let _ = std::fs::remove_file(path);
    let con = duckdb::Connection::open(path).unwrap();
    // bundled 内建 fts？不 LOAD 直接建索引
    con.execute_batch(
        "CREATE TABLE t0 (row_ord INTEGER, 活动 VARCHAR, 角色 VARCHAR);
         INSERT INTO t0 VALUES (0, 'accept concept LPDT', 'LPDT'), (1, 'verify PQA', 'PQA');",
    )
    .unwrap();
    let r = con.execute_batch("PRAGMA create_fts_index('t0', 'row_ord', '活动', '角色')");
    match r {
        Ok(()) => println!("RUST bundled: create_fts_index OK without LOAD"),
        Err(e) => {
            println!("RUST bundled: create_fts_index failed without LOAD: {e}");
            let r2 = con.execute_batch("LOAD fts");
            println!("LOAD fts: {r2:?}");
            if let Err(e) =
                con.execute_batch("PRAGMA create_fts_index('t0', 'row_ord', '活动', '角色')")
            {
                println!("after LOAD still failed: {e}");
                return;
            }
        }
    }
    let rows: Vec<(i64, String)> = con
        .prepare("SELECT row_ord, 活动 FROM t0 WHERE fts_main_t0.match_bm25(row_ord, 'concept') IS NOT NULL")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(|x| x.ok())
        .collect();
    println!("match concept: {rows:?}");
    drop(con);

    // 只读 + 加固连接（struct_query 同款配置）：先 LOAD fts（需要扩展目录访问），
    // 再 SET enable_external_access=false + lock_configuration=true（查询期文件访问全禁）。
    let config = duckdb::Config::default()
        .access_mode(duckdb::AccessMode::ReadOnly)
        .unwrap();
    let ro = duckdb::Connection::open_with_flags(path, config).unwrap();
    ro.execute_batch("LOAD fts").unwrap();
    ro.execute_batch("SET enable_external_access=false; SET lock_configuration=true;")
        .unwrap();
    let rows2: Vec<(i64, String)> = ro
        .prepare(
            "SELECT row_ord, 活动 FROM t0 WHERE fts_main_t0.match_bm25(row_ord, 'PQA') IS NOT NULL",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(|x| x.ok())
        .collect();
    println!("readonly+locked match PQA: {rows2:?}");
    // 加固连接下 read_csv 仍被拦（纵深防御保留）
    let guarded = ro.prepare("SELECT * FROM read_csv('/etc/passwd')").is_err();
    println!("read_csv blocked in locked conn: {guarded}");
}
