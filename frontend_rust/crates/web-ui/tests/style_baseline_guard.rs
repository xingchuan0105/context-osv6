use std::fs;
use std::path::{Path, PathBuf};

fn collect_files_with_ext(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.exists() {
        return results;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(collect_files_with_ext(&path, exts));
            } else if let Some(ext) = path.extension() {
                if exts.iter().any(|e| ext == *e) {
                    results.push(path);
                }
            }
        }
    }
    results
}

#[test]
fn test_style_baseline_guard_rules() {
    let root_ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let style_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../style");

    let mut css_files = collect_files_with_ext(&style_dir, &["css"]);
    css_files.extend(collect_files_with_ext(&root_ui, &["css"]));

    let rs_files = collect_files_with_ext(&root_ui, &["rs"]);

    // 1. 无 font-weight >= 500
    let weight_regex =
        regex_lite::Regex::new(r"font-weight:\s*([5-9]\d{2}\b|bold\b|bolder\b)").unwrap();
    let mut weight_violations = Vec::new();
    for file in &css_files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if weight_regex.is_match(line) {
                weight_violations.push(format!("{}:{} -> {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        weight_violations.is_empty(),
        "Found font-weight >= 500 violations:\n{:#?}",
        weight_violations
    );

    // 2. 无裸露十六进制颜色（排除 design-tokens.css 自身定义）
    let hex_regex = regex_lite::Regex::new(r"#[0-9a-fA-F]{3,8}\b").unwrap();
    let mut hex_violations = Vec::new();
    for file in &rs_files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if hex_regex.is_match(line) {
                hex_violations.push(format!("{}:{} -> {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        hex_violations.is_empty(),
        "Found bare hex color violations in Rust UI components:\n{:#?}",
        hex_violations
    );
}
