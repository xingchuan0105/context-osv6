use std::collections::BTreeMap;

use async_trait::async_trait;
use scraper::{Html, Selector};
use serde::Serialize;

use super::{DocumentParser, Page, ParsedDocument};

pub struct HtmlParser;

#[derive(Debug, Clone, Serialize)]
struct EmbeddedImageMeta {
    src: String,
    alt: Option<String>,
}

#[async_trait]
impl DocumentParser for HtmlParser {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument> {
        let html_str = String::from_utf8_lossy(bytes);
        let document = Html::parse_document(&html_str);

        let title = extract_title(&document).unwrap_or_else(|| filename.to_string());
        let content = extract_text_content(&document);
        let embedded_images = extract_images(&document);

        let pages = vec![Page {
            number: 1,
            content,
            cursor: "chunk-0".to_string(),
        }];

        let mut metadata = BTreeMap::new();
        metadata.insert("source_file".to_string(), filename.to_string());
        metadata.insert("parser".to_string(), "html".to_string());
        if !embedded_images.is_empty() {
            metadata.insert(
                "embedded_images_json".to_string(),
                serde_json::to_string(&embedded_images)?,
            );
        }

        Ok(ParsedDocument {
            title,
            pages,
            metadata,
        })
    }
}

fn extract_title(document: &Html) -> Option<String> {
    let title_selector = Selector::parse("title").ok()?;
    document
        .select(&title_selector)
        .next()
        .map(|el| el.text().collect::<String>())
        .filter(|s| !s.is_empty())
}

fn extract_text_content(document: &Html) -> String {
    let mut text_parts = Vec::new();

    let text_selector = match Selector::parse(
        "p, h1, h2, h3, h4, h5, h6, li, td, th, div, span, a, strong, em, b, i, code, pre, blockquote",
    ) {
        Ok(selector) => selector,
        Err(_) => return String::new(),
    };

    for element in document.select(&text_selector) {
        let text = element.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            text_parts.push(text);
        }
    }

    text_parts.join("\n")
}

fn extract_images(document: &Html) -> Vec<EmbeddedImageMeta> {
    let img_selector = match Selector::parse("img") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };

    document
        .select(&img_selector)
        .filter_map(|element| {
            let src = element.value().attr("src")?.trim();
            if src.is_empty() {
                return None;
            }
            let alt = element
                .value()
                .attr("alt")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            Some(EmbeddedImageMeta {
                src: src.to_string(),
                alt,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn html_parser_extracts_embedded_images_metadata() {
        let html = br#"<html><head><title>Deck</title></head><body><h1>Hello</h1><p>World</p><img src="https://example.com/a.png" alt="diagram" /></body></html>"#;
        let doc = HtmlParser.parse(html, "deck.html").await.unwrap();
        assert_eq!(doc.title, "Deck");
        assert_eq!(doc.metadata.get("parser").map(String::as_str), Some("html"));
        let embedded = doc.metadata.get("embedded_images_json").unwrap();
        assert!(embedded.contains("https://example.com/a.png"));
        assert!(embedded.contains("diagram"));
    }
}
