use web_sdk::render_assistant_markdown;

#[test]
fn empty_input_is_empty_html() {
    assert_eq!(render_assistant_markdown(""), "");
}

#[test]
fn renders_headings_lists_tables_code_and_emphasis() {
    let src = [
        "# 标题",
        "",
        "- 一项",
        "",
        "| 列 | 值 |",
        "| --- | --- |",
        "| 置信 | 高 |",
        "",
        "这是 **粗体** 和 `code`。",
        "",
        "```",
        "const value = 1;",
        "```",
        "",
        "~~删除~~",
    ]
    .join("\n");
    let html = render_assistant_markdown(&src);
    assert!(html.contains("<h1>标题</h1>"), "{html}");
    assert!(html.contains("<li>一项</li>"), "{html}");
    assert!(html.contains("<table>"), "{html}");
    assert!(html.contains("<th>列</th>"), "{html}");
    assert!(html.contains("<strong>粗体</strong>"), "{html}");
    assert!(html.contains("<code>code</code>"), "{html}");
    assert!(html.contains("const value = 1;"), "{html}");
    assert!(html.contains("<del>删除</del>"), "{html}");
}

#[test]
fn strips_script_event_handlers_and_javascript_urls() {
    let src = [
        "# 安全",
        "",
        "<script>alert(1)</script>",
        "<img src=x onerror=\"alert(1)\">",
        "<p onclick=\"alert(1)\">点我</p>",
        "[xss](javascript:alert(1))",
        "",
        "安全段落。",
        "",
        "[ok](https://ok.example)",
    ]
    .join("\n");
    let html = render_assistant_markdown(&src);
    let lower = html.to_ascii_lowercase();
    assert!(html.contains("<h1>安全</h1>"), "{html}");
    assert!(html.contains("安全段落"), "{html}");
    assert!(html.contains("https://ok.example"), "{html}");
    assert!(html.contains("rel=\"noopener noreferrer\""), "{html}");
    assert!(!lower.contains("<script"), "{html}");
    assert!(!lower.contains("onerror"), "{html}");
    assert!(!lower.contains("onclick"), "{html}");
    assert!(!lower.contains("javascript:"), "{html}");
    assert!(!lower.contains("<img"), "{html}");
    assert!(!lower.contains("<iframe"), "{html}");
}

#[test]
fn drops_raw_html_blocks_including_iframe() {
    let src = "<iframe src=\"https://evil.example\"></iframe>\n\n[ok](https://ok.example)";
    let html = render_assistant_markdown(src);
    let lower = html.to_ascii_lowercase();
    assert!(!lower.contains("<iframe"), "{html}");
    assert!(!lower.contains("evil.example"), "{html}");
    assert!(html.contains("https://ok.example"), "{html}");
    assert!(html.contains("rel=\"noopener noreferrer\""), "{html}");
}
