// finetype 数值规整探针（fts_probe 同款骨架）：
// 路线1 bundled 内建？路线2 社区扩展在加固只读连接（先 LOAD 后锁）可用？
// 路线3 无扩展纯 SQL 规整（TRY_CAST/strptime）在加固连接内可用？
fn main() {
    let path = "/tmp/finetype_rust_probe.duckdb";
    let _ = std::fs::remove_file(path);
    let con = duckdb::Connection::open(path).unwrap();
    con.execute_batch(
        "CREATE TABLE t0 (row_ord INTEGER, 金额 VARCHAR, 日期 VARCHAR);
         INSERT INTO t0 VALUES
           (0, '1,234.56', '2024-01-15'),
           (1, '¥9,876,543.21', '2024/1/5'),
           (2, '12%', '2024年3月8日'),
           (3, '不适用', 'N/A');",
    )
    .unwrap();

    // ── 路线1：bundled 内建？不 INSTALL/LOAD 直接调用 ──
    let r1 = con.prepare("SELECT ft_version()");
    println!(
        "R1 bundled ft_version without LOAD: {}",
        match r1 {
            Ok(_) => "OK (内建)".to_string(),
            Err(e) => format!("FAIL: {}", e.to_string().lines().next().unwrap_or("")),
        }
    );

    // ── 路线2：社区扩展 INSTALL/LOAD（需网络），再走加固只读连接 ──
    let r2i = con.execute_batch("INSTALL finetype FROM community");
    println!(
        "R2 INSTALL finetype FROM community: {}",
        match &r2i {
            Ok(()) => "OK".to_string(),
            Err(e) => format!("FAIL: {}", e.to_string().lines().next().unwrap_or("")),
        }
    );
    if r2i.is_ok() {
        let r2l = con.execute_batch("LOAD finetype");
        println!("R2 LOAD finetype: {r2l:?}");
        for sql in [
            "SELECT ft_version()",
            "SELECT ft_infer('2024-01-15')",
            "SELECT ft_cast('01/15/2024')",
        ] {
            match con
                .prepare(sql)
                .and_then(|mut s| s.query_row([], |r| r.get::<_, String>(0)))
            {
                Ok(v) => println!("R2 {sql} => {v}"),
                Err(e) => println!(
                    "R2 {sql} => FAIL: {}",
                    e.to_string().lines().next().unwrap_or("")
                ),
            }
        }
    }
    drop(con);

    // ── 加固只读连接（struct_query 同款）：路线3 在此连接内验证 ──
    let config = duckdb::Config::default()
        .access_mode(duckdb::AccessMode::ReadOnly)
        .unwrap();
    let ro = duckdb::Connection::open_with_flags(path, config).unwrap();
    ro.execute_batch("SET enable_external_access=false; SET lock_configuration=true;")
        .unwrap();

    // ── 路线3：无扩展纯 SQL 规整（加固只读连接内）──
    let probe_sqls = [
        (
            "num comma",
            "SELECT TRY_CAST(replace('1,234,567.89', ',', '') AS DOUBLE)",
        ),
        (
            "num currency",
            "SELECT TRY_CAST(regexp_replace('¥9,876.21', '[^0-9.\\-]', '', 'g') AS DOUBLE)",
        ),
        (
            "pct",
            "SELECT TRY_CAST(replace('12%', '%', '') AS DOUBLE) / 100",
        ),
        ("date iso", "SELECT TRY_CAST('2024-01-15' AS DATE)"),
        (
            "date slash",
            "SELECT TRY_CAST(strptime('2024/1/5', '%Y/%-m/%-d') AS DATE)",
        ),
        (
            "date cjk",
            "SELECT TRY_CAST(strptime('2024年3月8日', '%Y年%m月%d日') AS DATE)",
        ),
        ("garbage", "SELECT TRY_CAST('不适用' AS DOUBLE)"),
    ];
    for (name, sql) in probe_sqls {
        match ro.prepare(sql).and_then(|mut s| {
            s.query_row([], |r| {
                let v: duckdb::types::Value = r.get(0)?;
                Ok(format!("{v:?}"))
            })
        }) {
            Ok(v) => println!("R3 {name}: {sql} => {v}"),
            Err(e) => println!(
                "R3 {name}: FAIL: {}",
                e.to_string().lines().next().unwrap_or("")
            ),
        }
    }
    // 列级判定：一列非空值 ≥90% 可转数值（影子列判定逻辑的 SQL 形态验证）
    let col_check = ro
        .prepare(
            "SELECT COUNT(*) FILTER (v IS NOT NULL AND TRY_CAST(regexp_replace(v, '[^0-9.\\-]', '', 'g') AS DOUBLE) IS NOT NULL) * 1.0 / COUNT(*) FROM (SELECT 金额 AS v FROM t0)",
        )
        .and_then(|mut s| s.query_row([], |r| r.get::<_, f64>(0)));
    println!("R3 col numeric ratio (金额): {col_check:?}");
    let guarded = ro.prepare("SELECT * FROM read_csv('/etc/passwd')").is_err();
    println!("read_csv blocked in locked conn: {guarded}");

    // ── 路线2 续（已知致命，放最后）：只读库上 LOAD finetype —— init 需 CREATE
    // 注册 table macro，READ_ONLY 库拒 CREATE → 扩展 init 非 unwinding panic →
    // 进程 abort（exit 134）。此调用必然崩进程，仅作否决证据保留。
    println!("R2 readonly LOAD finetype: attempting (expected to abort)...");
    let ro_load = ro.execute_batch("LOAD finetype");
    println!("R2 readonly LOAD finetype: {ro_load:?} (unreachable if aborted)");
}
