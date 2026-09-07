use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

/// 助手答案：CommonMark（表 / 删除线）→ HTML。
/// 不经过 ammonia/html5ever（会拉 ICU，wasm 体积与本机 target 都不划算）。
/// 原始 HTML 事件直接丢掉；链接只保留 http(s)。
pub fn render_assistant_markdown(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, options);
    let mut html_out = String::new();
    html::push_html(&mut html_out, filter_markdown_events(parser));
    let html_out = html_out.replace("<a href=\"", "<a rel=\"noopener noreferrer\" href=\"");
    wrap_figures(&decorate_code_blocks(&html_out))
}

fn decorate_code_blocks(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;
    while let Some(idx) = rest.find("<pre><code") {
        out.push_str(&rest[..idx]);
        let after = &rest[idx..];
        let Some(gt) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let open = &after[..=gt];
        let lang = code_language(open).unwrap_or("code");
        let inner_and_rest = &after[gt + 1..];
        let Some(end) = inner_and_rest.find("</code></pre>") else {
            out.push_str(after);
            return out;
        };
        out.push_str("<div class=\"chat-code-block\" data-testid=\"chat-code-block\">");
        out.push_str("<div class=\"chat-code-toolbar\">");
        out.push_str("<span class=\"chat-code-lang\" data-testid=\"chat-code-lang\">");
        out.push_str(lang);
        out.push_str("</span>");
        out.push_str(
            "<button type=\"button\" class=\"chat-code-copy\" data-testid=\"chat-code-copy\">复制</button>",
        );
        out.push_str("</div>");
        out.push_str(open);
        out.push_str(&inner_and_rest[..end]);
        out.push_str("</code></pre></div>");
        rest = &inner_and_rest[end + "</code></pre>".len()..];
    }
    out.push_str(rest);
    out
}

fn code_language(open_tag: &str) -> Option<&'static str> {
    const KNOWN: &[&str] = &[
        "rust", "ts", "tsx", "js", "json", "py", "python", "bash", "sh", "sql", "go", "yaml",
        "toml", "html", "css", "md", "text",
    ];
    let class = open_tag.split("language-").nth(1)?;
    let token = class
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .next()
        .unwrap_or("");
    KNOWN
        .iter()
        .copied()
        .find(|name| name.eq_ignore_ascii_case(token))
}

fn wrap_figures(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 32);
    let mut rest = html;
    while let Some(idx) = rest.find("<img ") {
        out.push_str(&rest[..idx]);
        let after = &rest[idx..];
        let Some(end) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let tag = &after[..=end];
        out.push_str("<figure class=\"chat-figure\" data-testid=\"chat-figure\">");
        out.push_str(tag);
        out.push_str("</figure>");
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn filter_markdown_events<'a, I>(events: I) -> impl Iterator<Item = Event<'a>>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut skip_image = false;
    let mut skip_html_block = false;
    let mut skip_metadata = false;
    let mut drop_link_end = false;

    events.filter_map(move |event| {
        if skip_image {
            if matches!(event, Event::End(TagEnd::Image)) {
                skip_image = false;
            }
            return None;
        }
        if skip_html_block {
            if matches!(event, Event::End(TagEnd::HtmlBlock)) {
                skip_html_block = false;
            }
            return None;
        }
        if skip_metadata {
            if matches!(event, Event::End(TagEnd::MetadataBlock(_))) {
                skip_metadata = false;
            }
            return None;
        }

        match event {
            Event::Html(_) | Event::InlineHtml(_) => None,
            Event::InlineMath(_) | Event::DisplayMath(_) => None,
            Event::Start(Tag::HtmlBlock) => {
                skip_html_block = true;
                None
            }
            Event::End(TagEnd::HtmlBlock) => None,
            Event::Start(Tag::MetadataBlock(_)) => {
                skip_metadata = true;
                None
            }
            Event::End(TagEnd::MetadataBlock(_)) => None,
            Event::Start(Tag::Image {
                dest_url,
                title,
                id,
                link_type,
            }) => {
                if is_safe_href(&dest_url) {
                    Some(Event::Start(Tag::Image {
                        dest_url,
                        title,
                        id,
                        link_type,
                    }))
                } else {
                    skip_image = true;
                    None
                }
            }
            Event::End(TagEnd::Image) => {
                if skip_image {
                    None
                } else {
                    Some(Event::End(TagEnd::Image))
                }
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                if is_safe_href(&dest_url) {
                    Some(Event::Start(Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        id,
                    }))
                } else {
                    drop_link_end = true;
                    None
                }
            }
            Event::End(TagEnd::Link) => {
                if drop_link_end {
                    drop_link_end = false;
                    None
                } else {
                    Some(Event::End(TagEnd::Link))
                }
            }
            Event::Start(_)
            | Event::End(_)
            | Event::Text(_)
            | Event::Code(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::FootnoteReference(_)
            | Event::TaskListMarker(_) => Some(event),
        }
    })
}

pub(crate) fn is_safe_href(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.bytes().any(|b| b < 0x20) {
        return false;
    }
    let Some(scheme_end) = url.find("://") else {
        return false;
    };
    let scheme = &url[..scheme_end];
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return false;
    }
    !url[scheme_end + 3..].is_empty()
}
