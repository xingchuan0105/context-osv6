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

fn is_token_css(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains("token"))
}

fn strip_css_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

struct StyleCorpus {
    css_files: Vec<PathBuf>,
    rs_files: Vec<PathBuf>,
    product_css: String,
}

impl StyleCorpus {
    fn load() -> Self {
        let root_ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let style_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../style");
        let assets_style_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/style");

        let mut css_files = collect_files_with_ext(&style_dir, &["css"]);
        css_files.extend(collect_files_with_ext(&assets_style_dir, &["css"]));
        css_files.extend(collect_files_with_ext(&root_ui, &["css"]));
        let rs_files = collect_files_with_ext(&root_ui, &["rs"]);

        let mut product_css = String::new();
        for file in &css_files {
            if is_token_css(file) {
                continue;
            }
            product_css.push_str(&strip_css_comments(
                &fs::read_to_string(file).unwrap_or_default(),
            ));
            product_css.push('\n');
        }

        Self {
            css_files,
            rs_files,
            product_css,
        }
    }
}

#[test]
fn test_style_baseline_guard_rules() {
    let corpus = StyleCorpus::load();

    // 1. 无 font-weight >= 500
    let weight_regex =
        regex_lite::Regex::new(r"font-weight:\s*([5-9]\d{2}\b|bold\b|bolder\b)").unwrap();
    let mut weight_violations = Vec::new();
    for file in &corpus.css_files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if weight_regex.is_match(line) {
                weight_violations.push(format!("{}:{} -> {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        weight_violations.is_empty(),
        "Found font-weight >= 500 violations:\n{weight_violations:#?}"
    );

    // 2. CSS 无裸 hex（排除 token 定义文件）
    let hex_regex = regex_lite::Regex::new(r"#[0-9a-fA-F]{3,8}\b").unwrap();
    let mut css_hex = Vec::new();
    for file in &corpus.css_files {
        if is_token_css(file) {
            continue;
        }
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if hex_regex.is_match(line) {
                css_hex.push(format!("{}:{} -> {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        css_hex.is_empty(),
        "Found bare hex color violations in CSS:\n{css_hex:#?}"
    );

    // 3. 组件无裸 hex
    let mut rs_hex = Vec::new();
    for file in &corpus.rs_files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if hex_regex.is_match(line) {
                rs_hex.push(format!("{}:{} -> {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        rs_hex.is_empty(),
        "Found bare hex color violations in Rust UI components:\n{rs_hex:#?}"
    );

    // 4. 阴影仅白名单浮层。hairline / inset / var(--shadow-*) 放行。
    // regex-lite 无 lookahead：先抓 `0 <offset> <blur>` 再排除 none/var/inset。
    let shadow_regex =
        regex_lite::Regex::new(r"box-shadow:\s*0(?:px)?\s+\d+px\s+\d+px").unwrap();
    let mut shadow_violations = Vec::new();
    for file in &corpus.css_files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if !shadow_regex.is_match(trimmed) {
                continue;
            }
            if trimmed.contains("none") || trimmed.contains("var(") || trimmed.contains("inset") {
                continue;
            }
            shadow_violations.push(format!("{}:{} -> {}", file.display(), i + 1, trimmed));
        }
    }
    assert!(
        shadow_violations.is_empty(),
        "Found literal drop shadows outside allowlisted overlays:\n{shadow_violations:#?}"
    );

    // 5. 无内联样式逃逸
    let inline_regex = regex_lite::Regex::new(r"\bstyle\s*=").unwrap();
    let mut inline_violations = Vec::new();
    for file in &corpus.rs_files {
        let content = fs::read_to_string(file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if inline_regex.is_match(line) {
                inline_violations.push(format!("{}:{} -> {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        inline_violations.is_empty(),
        "Found inline style= escapes in Rust UI components:\n{inline_violations:#?}"
    );
}

#[test]
fn test_no_dangling_css_variables() {
    let corpus = StyleCorpus::load();
    let def_re = regex_lite::Regex::new(r"--([A-Za-z0-9-]+)\s*:").unwrap();
    let use_re = regex_lite::Regex::new(r"var\(\s*--([A-Za-z0-9-]+)").unwrap();

    let mut defined = std::collections::BTreeSet::new();
    for file in &corpus.css_files {
        let content = strip_css_comments(&fs::read_to_string(file).unwrap_or_default());
        for cap in def_re.captures_iter(&content) {
            defined.insert(cap[1].to_string());
        }
    }

    let mut used = std::collections::BTreeSet::new();
    for cap in use_re.captures_iter(&corpus.product_css) {
        used.insert(cap[1].to_string());
    }

    let dangling: Vec<_> = used.difference(&defined).cloned().collect();
    assert!(
        dangling.is_empty(),
        "Found dangling CSS variables (used but not defined):\n{dangling:#?}"
    );
}

#[test]
fn test_visual_foundation_floors() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/style");
    assert!(
        !assets.join("chat-poc.css").exists(),
        "obsolete chat-poc.css must be deleted"
    );
    assert!(
        assets.join("base.css").exists() && assets.join("app.css").exists(),
        "base.css and app.css must exist"
    );

    let corpus = StyleCorpus::load();
    let css = &corpus.product_css;
    assert!(
        css.contains("body {") || css.contains("body{"),
        "base layer must include a body {{ }} rule"
    );
    assert!(
        css.contains(":focus-visible"),
        "base layer must include :focus-visible"
    );

    let focus = css.matches(":focus").count();
    let transition = css.matches("transition").count();
    let keyframes = css.matches("@keyframes").count();
    let kf_re = regex_lite::Regex::new(r"@keyframes\s+([A-Za-z0-9_-]+)").unwrap();
    let unique_keyframes: std::collections::BTreeSet<_> = kf_re
        .captures_iter(css)
        .map(|cap| cap[1].to_string())
        .collect();
    assert!(
        focus >= 20,
        ":focus* count {focus} is below floor 20"
    );
    assert!(
        transition >= 20,
        "transition count {transition} is below floor 20"
    );
    assert!(
        keyframes >= 3,
        "@keyframes count {keyframes} is below floor 3"
    );
    assert!(
        unique_keyframes.len() >= 10,
        "unique @keyframes names {} is below floor 10: {unique_keyframes:?}",
        unique_keyframes.len()
    );
}
