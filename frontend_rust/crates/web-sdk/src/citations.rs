use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::markdown::{is_safe_href, render_assistant_markdown};

/// 助手答案里的一条来源（从 citations 事件 / Done payload 的 JSON 读出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationView {
    pub citation_id: i64,
    pub doc_id: String,
    pub chunk_id: Option<String>,
    pub doc_name: String,
    pub preview: Option<String>,
    pub layer: Option<String>,
    pub chunk_type: Option<String>,
    pub url: Option<String>,
    pub tombstone: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCard {
    pub key: String,
    pub seq: u32,
    pub title: String,
    pub preview: String,
    pub href: Option<String>,
    pub tombstone: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedAnswer {
    pub html: String,
    pub cards: Vec<SourceCard>,
}

impl CitationView {
    pub fn from_value(value: &Value) -> Self {
        let doc_id = json_string(value, "doc_id");
        let locator_url = value
            .get("source_locator")
            .and_then(|locator| locator.get("url"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| is_safe_href(url))
            .map(str::to_string);
        let url = locator_url.or_else(|| is_safe_href(&doc_id).then(|| doc_id.clone()));
        let doc_name = {
            let name = json_string(value, "doc_name");
            if name.is_empty() {
                json_string(value, "title")
            } else {
                name
            }
        };
        Self {
            citation_id: json_i64(value, "citation_id"),
            chunk_id: json_opt_string(value, "chunk_id"),
            doc_id,
            doc_name,
            preview: json_opt_string(value, "preview"),
            layer: json_opt_string(value, "layer"),
            chunk_type: json_opt_string(value, "chunk_type"),
            url,
            tombstone: value
                .get("citation_status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "source_deleted"),
        }
    }

    pub fn identity_key(&self) -> String {
        if let Some(chunk_id) = self
            .chunk_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return safe_key(&format!("doc:{chunk_id}"));
        }
        if let Some(url) = &self.url {
            return safe_key(&format!("web:{url}"));
        }
        safe_key(&format!("id:{}", self.citation_id))
    }

    fn title(&self) -> String {
        let name = self.doc_name.trim();
        if name.is_empty() {
            "来源".to_string()
        } else {
            name.to_string()
        }
    }

    fn to_card(&self, seq: u32) -> SourceCard {
        SourceCard {
            key: self.identity_key(),
            seq,
            title: self.title(),
            preview: self.preview.as_deref().unwrap_or("").trim().to_string(),
            href: self.url.clone(),
            tombstone: self.tombstone,
        }
    }
}

/// 先把引用 marker 换成占位符，再走 Markdown，最后注入我们生成的 chip。
/// `[[image:…]]` 本层不处理。
pub fn render_assistant_answer(src: &str, citations: &[CitationView]) -> RenderedAnswer {
    if src.is_empty() && citations.is_empty() {
        return RenderedAnswer {
            html: String::new(),
            cards: Vec::new(),
        };
    }
    let (tokenized, chips, mut next_seq, mut seq_by_key) = tokenize_markers(src, citations);
    let mut html = render_assistant_markdown(&tokenized);
    for (index, chip) in chips.iter().enumerate() {
        let token = format!("CITATIONTOKEN{index}END");
        html = html.replace(&token, &chip.html);
    }

    let mut cards = Vec::new();
    let mut seen = HashSet::new();
    for chip in &chips {
        let Some(key) = &chip.key else {
            continue;
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(citation) = citations.iter().find(|citation| citation.identity_key() == *key)
        {
            cards.push(citation.to_card(chip.seq));
        }
    }
    for citation in citations {
        let key = citation.identity_key();
        if !seen.insert(key.clone()) {
            continue;
        }
        let seq = *seq_by_key.entry(key).or_insert_with(|| {
            let seq = next_seq;
            next_seq += 1;
            seq
        });
        cards.push(citation.to_card(seq));
    }

    RenderedAnswer { html, cards }
}

struct PendingChip {
    html: String,
    seq: u32,
    key: Option<String>,
}

struct ParsedMarker<'a> {
    chunk_id: Option<String>,
    number: Option<String>,
    rest: &'a str,
}

fn tokenize_markers<'a>(
    src: &'a str,
    citations: &[CitationView],
) -> (String, Vec<PendingChip>, u32, HashMap<String, u32>) {
    let mut out = String::with_capacity(src.len());
    let mut chips = Vec::new();
    let mut seq_by_key = HashMap::new();
    let mut next_seq = 1_u32;
    let mut rest = src;

    while !rest.is_empty() {
        if rest.starts_with("[[image:") {
            if let Some(end) = rest.find("]]") {
                out.push_str(&rest[..end + 2]);
                rest = &rest[end + 2..];
                continue;
            }
        }
        if let Some(marker) = parse_marker(rest) {
            let chip = resolve_chip(&marker, citations, &mut seq_by_key, &mut next_seq);
            out.push_str(&format!("CITATIONTOKEN{}END", chips.len()));
            chips.push(chip);
            rest = marker.rest;
            continue;
        }
        let ch = rest.chars().next().expect("rest is non-empty");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }

    (out, chips, next_seq, seq_by_key)
}

fn parse_marker(rest: &str) -> Option<ParsedMarker<'_>> {
    if let Some(after) = rest.strip_prefix("[[cite:") {
        let (id, rest) = split_until(after, "]]")?;
        if id.is_empty() {
            return None;
        }
        return Some(ParsedMarker {
            chunk_id: Some(id.to_string()),
            number: None,
            rest,
        });
    }
    if let Some(after) = rest.strip_prefix("[[web:") {
        let (id, rest) = split_digits(after, "]]")?;
        return Some(ParsedMarker {
            chunk_id: None,
            number: Some(id),
            rest,
        });
    }
    if let Some(after) = rest.strip_prefix("[[") {
        let (id, rest) = split_digits(after, "]]")?;
        return Some(ParsedMarker {
            chunk_id: None,
            number: Some(id),
            rest,
        });
    }
    if let Some(after) = rest.strip_prefix("[web:") {
        let (id, rest) = split_digits(after, "]")?;
        return Some(ParsedMarker {
            chunk_id: None,
            number: Some(id),
            rest,
        });
    }
    if let Some(after) = rest.strip_prefix("[citation:") {
        let (id, rest) = split_digits(after, "]")?;
        return Some(ParsedMarker {
            chunk_id: None,
            number: Some(id),
            rest,
        });
    }
    if let Some(after) = rest.strip_prefix("[") {
        let (id, rest) = split_digits(after, "]")?;
        return Some(ParsedMarker {
            chunk_id: None,
            number: Some(id),
            rest,
        });
    }
    None
}

fn split_until<'a>(input: &'a str, delim: &str) -> Option<(&'a str, &'a str)> {
    let index = input.find(delim)?;
    Some((input[..index].trim(), &input[index + delim.len()..]))
}

fn split_digits<'a>(input: &'a str, delim: &str) -> Option<(String, &'a str)> {
    let index = input.find(delim)?;
    let inner = input[..index].trim();
    if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((inner.to_string(), &input[index + delim.len()..]))
}

fn resolve_chip(
    marker: &ParsedMarker<'_>,
    citations: &[CitationView],
    seq_by_key: &mut HashMap<String, u32>,
    next_seq: &mut u32,
) -> PendingChip {
    if let Some(citation) = resolve_citation(marker, citations) {
        let key = citation.identity_key();
        let seq = *seq_by_key.entry(key.clone()).or_insert_with(|| {
            let seq = *next_seq;
            *next_seq += 1;
            seq
        });
        PendingChip {
            html: chip_html(seq, Some(&key), &citation.title(), citation.tombstone),
            seq,
            key: Some(key),
        }
    } else {
        let fallback_key = marker
            .chunk_id
            .as_deref()
            .map(|chunk| format!("cite:{chunk}"))
            .or_else(|| marker.number.as_deref().map(|number| format!("obs:{number}")))
            .unwrap_or_else(|| "obs".to_string());
        let seq = *seq_by_key
            .entry(format!("fallback:{fallback_key}"))
            .or_insert_with(|| {
                let seq = *next_seq;
                *next_seq += 1;
                seq
            });
        PendingChip {
            html: chip_html(seq, None, "", false),
            seq,
            key: None,
        }
    }
}

fn resolve_citation<'a>(
    marker: &ParsedMarker<'_>,
    citations: &'a [CitationView],
) -> Option<&'a CitationView> {
    if let Some(chunk_id) = marker.chunk_id.as_deref().map(str::trim).filter(|id| !id.is_empty())
    {
        if let Some(hit) = citations.iter().find(|citation| {
            citation
                .chunk_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|id| id == chunk_id)
        }) {
            return Some(hit);
        }
    }
    let raw = marker.number.as_deref()?;
    let number: i64 = raw.parse().ok()?;
    if number < 1 {
        return None;
    }
    let web = citations.iter().find(|citation| {
        citation.citation_id == number
            && (citation.layer.as_deref() == Some("search")
                || citation.chunk_type.as_deref() == Some("web"))
    });
    if web.is_some() {
        return web;
    }
    if let Some(hit) = citations
        .iter()
        .find(|citation| citation.citation_id == number)
    {
        return Some(hit);
    }
    citations.get(usize::try_from(number).ok()?.checked_sub(1)?)
}

fn chip_html(seq: u32, key: Option<&str>, title: &str, tombstone: bool) -> String {
    let seq_text = escape_text(&seq.to_string());
    match key {
        Some(key) if tombstone => format!(
            "<button type=\"button\" class=\"chat-cite-chip\" data-cite-key=\"{}\" data-testid=\"citation-chip\" disabled aria-label=\"来源已删除\">{}</button>",
            escape_attr(key),
            seq_text
        ),
        Some(key) => {
            let label = if title.is_empty() {
                format!("来源 {seq}")
            } else {
                format!("来源 {seq} {title}")
            };
            format!(
                "<button type=\"button\" class=\"chat-cite-chip\" data-cite-key=\"{}\" data-testid=\"citation-chip\" aria-label=\"{}\">{}</button>",
                escape_attr(key),
                escape_attr(&label),
                seq_text
            )
        }
        None => format!(
            "<span class=\"chat-cite-chip chat-cite-chip-fallback\" data-testid=\"citation-chip\">{seq_text}</span>"
        ),
    }
}

fn json_string(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn json_opt_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn json_i64(value: &Value, field: &str) -> i64 {
    let Some(raw) = value.get(field) else {
        return 0;
    };
    raw.as_i64()
        .or_else(|| raw.as_u64().map(|n| n as i64))
        .or_else(|| raw.as_str()?.parse().ok())
        .unwrap_or(0)
}

fn safe_key(raw: &str) -> String {
    let key: String = raw
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-' | '/' | '?' | '=' | '&' | '%')
        })
        .collect();
    if key.is_empty() {
        "cite".to_string()
    } else {
        key
    }
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
