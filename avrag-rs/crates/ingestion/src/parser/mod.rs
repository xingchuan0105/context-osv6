mod code;
mod html;
mod mineru;
mod office;
mod pdf;
mod probe;
mod router;
mod text;

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;

pub use code::CodeParser;
pub use html::HtmlParser;
pub use mineru::{MineruClient, MineruConfig};
pub use office::OfficeParser;
pub use pdf::PdfParser;
pub use probe::{ParseProbe, ParseProbeResult};
pub use router::{ParseRoute, ParseRouteDecision, ParseRouter, RouteReason};
pub use text::TextParser;

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub title: String,
    pub pages: Vec<Page>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub number: u32,
    pub content: String,
    pub cursor: String,
}

#[async_trait]
pub trait DocumentParser: Send + Sync {
    async fn parse(&self, bytes: &[u8], filename: &str) -> anyhow::Result<ParsedDocument>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedUnitKind {
    Text,
    ImageWithContext,
}

#[derive(Debug, Clone)]
pub struct ParsedUnit {
    pub unit_id: String,
    pub page: u32,
    pub kind: ParsedUnitKind,
    pub text: String,
    pub image_path: Option<String>,
    pub caption: Option<String>,
    pub context: Option<String>,
    pub parser_backend: String,
    pub metadata: BTreeMap<String, String>,
}

impl ParsedUnit {
    pub fn new_text(page: u32, text: String, parser_backend: String) -> Self {
        Self {
            unit_id: Uuid::new_v4().to_string(),
            page,
            kind: ParsedUnitKind::Text,
            text,
            image_path: None,
            caption: None,
            context: None,
            parser_backend,
            metadata: BTreeMap::new(),
        }
    }

    pub fn new_image_with_context(
        page: u32,
        text: String,
        image_path: String,
        caption: Option<String>,
        context: Option<String>,
        parser_backend: String,
    ) -> Self {
        Self {
            unit_id: Uuid::new_v4().to_string(),
            page,
            kind: ParsedUnitKind::ImageWithContext,
            text,
            image_path: Some(image_path),
            caption,
            context,
            parser_backend,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedDocument {
    pub title: String,
    pub units: Vec<ParsedUnit>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddedImageMeta {
    src: String,
    alt: Option<String>,
}

pub struct ParserFactory;

impl ParserFactory {
    pub fn create_parser(filename: &str) -> Option<Box<dyn DocumentParser>> {
        let extension = filename.rsplit('.').next()?.to_lowercase();
        match extension.as_str() {
            "pdf" => Some(Box::new(PdfParser)),
            "xlsx" | "xls" => Some(Box::new(OfficeParser)),
            "docx" | "doc" => Some(Box::new(OfficeParser)),
            "html" | "htm" => Some(Box::new(HtmlParser)),
            "txt" | "md" | "rst" => Some(Box::new(TextParser)),
            _ if Self::is_code_file(&extension) => Some(Box::new(CodeParser)),
            _ => Some(Box::new(TextParser)),
        }
    }

    fn is_code_file(extension: &str) -> bool {
        matches!(
            extension,
            "rs" | "py"
                | "js"
                | "ts"
                | "jsx"
                | "tsx"
                | "go"
                | "java"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "cs"
                | "rb"
                | "php"
                | "swift"
                | "kt"
                | "scala"
                | "r"
                | "lua"
                | "sh"
                | "bash"
                | "zsh"
                | "ps1"
                | "sql"
                | "yaml"
                | "yml"
                | "toml"
                | "json"
                | "xml"
                | "css"
                | "scss"
                | "sass"
                | "less"
                | "vue"
                | "svelte"
                | "graphql"
                | "proto"
                | "gradle"
                | "cmake"
                | "makefile"
                | "dockerfile"
                | "tf"
                | "hcl"
        )
    }
}

pub fn normalize_parsed_document(doc: &ParsedDocument, parser_backend: &str) -> NormalizedDocument {
    let mut units = doc
        .pages
        .iter()
        .map(|page| {
            ParsedUnit::new_text(
                page.number,
                page.content.clone(),
                parser_backend.to_string(),
            )
        })
        .collect::<Vec<_>>();

    if let Some(images_json) = doc.metadata.get("embedded_images_json") {
        if let Ok(images) = serde_json::from_str::<Vec<EmbeddedImageMeta>>(images_json) {
            let base_context = doc
                .pages
                .first()
                .map(|page| page.content.chars().take(400).collect::<String>());
            for image in images {
                let caption = image.alt.clone();
                let context = base_context.clone();
                let text = [
                    caption.clone().unwrap_or_default(),
                    context.clone().unwrap_or_default(),
                ]
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
                units.push(ParsedUnit::new_image_with_context(
                    1,
                    if text.is_empty() {
                        image.src.clone()
                    } else {
                        text
                    },
                    image.src,
                    caption,
                    context,
                    parser_backend.to_string(),
                ));
            }
        }
    }

    NormalizedDocument {
        title: doc.title.clone(),
        units,
        metadata: doc.metadata.clone(),
    }
}
