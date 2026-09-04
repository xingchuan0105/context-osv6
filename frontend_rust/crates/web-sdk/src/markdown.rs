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
    html_out.replace("<a href=\"", "<a rel=\"noopener noreferrer\" href=\"")
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
            Event::Start(Tag::Image { .. }) => {
                skip_image = true;
                None
            }
            Event::End(TagEnd::Image) => None,
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
