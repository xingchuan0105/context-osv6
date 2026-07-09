use std::env;
use std::fs;
use std::path::Path;

fn read_shard_list(adapter_dir: &str) -> Vec<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let list_path = Path::new(&manifest_dir)
        .join(adapter_dir)
        .join("shards.lst");
    println!("cargo:rerun-if-changed={}", list_path.display());
    let content = fs::read_to_string(&list_path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", list_path.display());
    });
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn assemble_port_impl(adapter_dir: &str, trait_name: &str, adapter_name: &str, out_file: &str) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let shards = read_shard_list(adapter_dir);
    assert!(
        !shards.is_empty(),
        "shards.lst in {adapter_dir} must list at least one shard"
    );

    let mut body = String::new();
    for shard in &shards {
        let path = Path::new(&manifest_dir).join(adapter_dir).join(shard);
        println!("cargo:rerun-if-changed={}", path.display());
        body.push_str(&fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("failed to read {}: {error}", path.display());
        }));
        if !body.ends_with('\n') {
            body.push('\n');
        }
    }

    let generated = format!("#[async_trait]\nimpl {trait_name} for {adapter_name} {{\n{body}}}\n");
    let out_path = Path::new(&out_dir).join(out_file);
    fs::write(&out_path, generated).unwrap_or_else(|error| {
        panic!("failed to write {}: {error}", out_path.display());
    });
}

fn main() {
    assemble_port_impl(
        "src/adapters/pg_share_store",
        "ShareStorePort",
        "PgShareStoreAdapter",
        "pg_share_store_port_impl.rs",
    );

    assemble_port_impl(
        "src/adapters/pg_admin_store",
        "AdminStorePort",
        "PgAdminStoreAdapter",
        "pg_admin_store_port_impl.rs",
    );
}
