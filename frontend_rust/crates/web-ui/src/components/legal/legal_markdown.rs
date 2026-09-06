use web_sdk::render_assistant_markdown;

/// 法务 Markdown → (HTML, TOC)。与 Next `renderLegalMarkdown` 对齐：
/// GFM 表格 + h2/h3 提取目录 + 标题注入 id（slug 规则：小写、空白转 `-`、保留 CJK）。
pub struct TocEntry {
    pub id: String,
    pub text: String,
    pub depth: usize,
}

pub fn render_legal_markdown(markdown: &str) -> (String, Vec<TocEntry>) {
    let html = render_assistant_markdown(markdown);
    inject_heading_ids(&html)
}

/// 在已渲染 HTML 上为 `<h2>`/`<h3>` 注入 id，并收集目录。
/// 标题文本由 pulldown_cmark 输出为纯文本（本仓法务文档不含内联标签标题）。
fn inject_heading_ids(html: &str) -> (String, Vec<TocEntry>) {
    let mut toc = Vec::new();
    let mut used_ids = std::collections::HashSet::new();
    let mut out = String::with_capacity(html.len() + 256);
    let mut rest = html;

    while let Some(start) = find_heading_start(rest) {
        let tag = &rest[start..start + 4]; // "<h2>" 或 "<h3>"
        let depth = if tag.starts_with("<h2>") { 2 } else { 3 };
        let after_tag = &rest[start + 4..];
        let close = format!("</h{depth}>");
        let Some(end) = after_tag.find(&close) else {
            break;
        };
        let text = strip_tags(&after_tag[..end]);
        let mut id = slugify(&text);
        if id.is_empty() {
            id = format!("section-{}", toc.len() + 1);
        }
        let mut unique = id.clone();
        let mut counter = 2usize;
        while !used_ids.insert(unique.clone()) {
            unique = format!("{id}-{counter}");
            counter += 1;
        }
        toc.push(TocEntry {
            id: unique.clone(),
            text: text.clone(),
            depth,
        });

        out.push_str(&rest[..start]);
        out.push_str(&format!("<h{depth} id=\"{unique}\">"));
        out.push_str(&after_tag[..end]);
        out.push_str(&close);
        rest = &after_tag[end + close.len()..];
    }
    out.push_str(rest);
    (out, toc)
}

fn find_heading_start(html: &str) -> Option<usize> {
    let h2 = html.find("<h2>");
    let h3 = html.find("<h3>");
    match (h2, h3) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn strip_tags(fragment: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in fragment.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = true;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_dash {
                slug.push('-');
                last_dash = true;
            }
        } else if ch.is_alphanumeric() || ch == '-' || ch == '_' {
            slug.extend(ch.to_lowercase());
            last_dash = false;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_ids_and_builds_toc() {
        let md = "# 标题\n\n## 服务内容\n\n正文。\n\n### 费用与支付\n\n表格。\n\n## 服务内容\n\n重复标题。\n";
        let (html, toc) = render_legal_markdown(md);
        assert!(html.contains("<h2 id=\"服务内容\">"));
        assert!(html.contains("<h3 id=\"费用与支付\">"));
        assert!(html.contains("<h2 id=\"服务内容-2\">"));
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].text, "服务内容");
        assert_eq!(toc[1].depth, 3);
        assert_eq!(toc[2].id, "服务内容-2");
    }

    #[test]
    fn ascii_slugs_are_lowercased() {
        assert_eq!(slugify("Authentication & Scope"), "authentication-scope");
    }
}
