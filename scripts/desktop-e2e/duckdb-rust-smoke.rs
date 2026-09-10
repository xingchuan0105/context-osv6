// Link against the product's already-built duckdb rlib and dependencies.
// No providers, services or desktop operations.
fn main() {
    eprintln!("RUST OPEN");
    let con = duckdb::Connection::open_in_memory().unwrap();
    eprintln!("RUST CREATE/INSERT");
    con.execute_batch("CREATE TABLE probe(item VARCHAR, units INTEGER)")
        .unwrap();
    con.execute(
        "INSERT INTO probe VALUES (?, ?)",
        duckdb::params!["sample", 42],
    )
    .unwrap();
    eprintln!("RUST QUERY");
    let value: i64 = con
        .query_row("SELECT units FROM probe", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, 42);
    drop(con);
    eprintln!("PASS Rust open/insert/query/close");
}
