use std::fs;
use std::path::{Path, PathBuf};

fn collect_rs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs(&path));
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
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

fn cjk_in_string_literals(src: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut end = idx + 1;
        let mut escaped = false;
        for (j, next) in src[idx + 1..].char_indices() {
            if escaped {
                escaped = false;
                end = idx + 1 + j + next.len_utf8();
                continue;
            }
            if next == '\\' {
                escaped = true;
                end = idx + 1 + j + next.len_utf8();
                continue;
            }
            if next == '"' {
                end = idx + 1 + j;
                break;
            }
            end = idx + 1 + j + next.len_utf8();
        }
        let lit = &src[idx + 1..end];
        if lit.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
            hits.push(lit.to_string());
        }
    }
    hits
}

#[test]
fn shell_and_chat_have_no_hardcoded_chinese_literals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/components");
    let mut files = collect_rs(&root.join("shell"));
    files.extend(collect_rs(&root.join("chat")));
    files.extend(collect_rs(&root.join("share")));
    let mut leftovers = Vec::new();
    for file in files {
        let raw = fs::read_to_string(&file).unwrap_or_default();
        let stripped = strip_comments(&raw);
        for hit in cjk_in_string_literals(&stripped) {
            leftovers.push(format!("{}: {hit}", file.display()));
        }
    }
    assert!(
        leftovers.is_empty(),
        "hardcoded Chinese remains in shell/chat:\n{}",
        leftovers.join("\n")
    );
}
