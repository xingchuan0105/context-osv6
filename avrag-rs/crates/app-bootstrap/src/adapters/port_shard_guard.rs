pub fn parse_shard_list(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

pub fn assert_shards_exist(adapter_rel_dir: &str, shards: &[String]) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(adapter_rel_dir);
    for shard in shards {
        assert!(
            base.join(shard).is_file(),
            "{shard} must exist under {adapter_rel_dir}"
        );
    }
}

pub fn assert_port_impl_includes_out_dir(port_impl: &str, out_file: &str) {
    assert!(
        port_impl.contains(out_file),
        "port_impl.rs must include assembled port impl ({out_file})"
    );
}

pub fn assert_no_orphan_rs_files(adapter_rel_dir: &str, shards: &[String]) {
    let mut allowed: std::collections::HashSet<String> = ["mod.rs", "mappers.rs", "port_impl.rs"]
        .into_iter()
        .map(str::to_string)
        .collect();
    for shard in shards {
        allowed.insert(shard.clone());
    }

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(adapter_rel_dir);
    for entry in std::fs::read_dir(&dir).expect("read adapter dir") {
        let name = entry
            .expect("dir entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name.ends_with(".rs") {
            assert!(
                allowed.contains(&name),
                "orphan shard file in {adapter_rel_dir}: {name}"
            );
        }
    }
}

pub fn assert_shards_lst_exists(adapter_rel_dir: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(adapter_rel_dir)
        .join("shards.lst");
    assert!(path.is_file(), "shards.lst must exist in {adapter_rel_dir}");
}
