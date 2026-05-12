use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::parser::{NormalizedDocument, ParsedUnitKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Pdf,
    Docx,
    Xlsx,
    Ppt,
    Pptx,
    Html,
    Text,
    Code,
    Image,
    #[default]
    Unknown,
}

impl DocumentType {
    pub fn from_filename(filename: &str) -> Self {
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();

        match extension.as_str() {
            "pdf" => Self::Pdf,
            "docx" | "doc" => Self::Docx,
            "xlsx" | "xls" => Self::Xlsx,
            "ppt" => Self::Ppt,
            "pptx" => Self::Pptx,
            "html" | "htm" => Self::Html,
            "rs" | "py" | "js" | "ts" | "tsx" | "go" | "java" | "c" | "cpp" | "h" => Self::Code,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Self::Image,
            "txt" | "md" | "rst" | "csv" | "json" | "toml" | "yaml" | "yml" => Self::Text,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParseBackend {
    EdgeParsePdf,
    MineruPdfOcr,
    MineruImage,
    Docx4jDocx,
    PoiXlsx,
    PoiPptx,
    PoiPpt,
    HtmlLocal,
    TextLocal,
    CodeLocal,
    #[default]
    Unknown,
}

impl ParseBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EdgeParsePdf => "edge_parse_pdf",
            Self::MineruPdfOcr => "mineru_pdf_ocr",
            Self::MineruImage => "mineru_image",
            Self::Docx4jDocx => "docx4j_docx",
            Self::PoiXlsx => "poi_xlsx",
            Self::PoiPptx => "poi_pptx",
            Self::PoiPpt => "poi_ppt",
            Self::HtmlLocal => "html_local",
            Self::TextLocal => "text_local",
            Self::CodeLocal => "code_local",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DocumentIr {
    pub document_id: String,
    pub title: String,
    pub doc_type: DocumentType,
    pub primary_backend: ParseBackend,
    pub backend_version: Option<String>,
    pub language: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub pages: Vec<PageIr>,
    pub blocks: Vec<BlockIr>,
    pub assets: Vec<AssetIr>,
    pub warnings: Vec<ParseWarning>,
}

impl DocumentIr {
    pub fn new(
        document_id: impl Into<String>,
        title: impl Into<String>,
        doc_type: DocumentType,
        primary_backend: ParseBackend,
    ) -> Self {
        Self {
            document_id: document_id.into(),
            title: title.into(),
            doc_type,
            primary_backend,
            backend_version: None,
            language: None,
            metadata: BTreeMap::new(),
            pages: Vec::new(),
            blocks: Vec::new(),
            assets: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn from_normalized_document(
        document_id: impl Into<String>,
        doc_type: DocumentType,
        primary_backend: ParseBackend,
        normalized: &NormalizedDocument,
    ) -> Self {
        let mut document = Self::new(
            document_id,
            normalized.title.clone(),
            doc_type,
            primary_backend.clone(),
        );
        document.metadata = normalized.metadata.clone();

        let mut page_numbers = BTreeSet::new();
        let mut page_text_chars = BTreeMap::<u32, usize>::new();
        let mut page_image_count = BTreeMap::<u32, usize>::new();

        for unit in &normalized.units {
            page_numbers.insert(unit.page);
            match unit.kind {
                ParsedUnitKind::Text => {
                    *page_text_chars.entry(unit.page).or_default() += unit.text.chars().count();
                    document.blocks.push(BlockIr {
                        block_id: unit.unit_id.clone(),
                        page: Some(unit.page),
                        block_type: BlockType::Paragraph,
                        modality: BlockModality::TextOnly,
                        text: unit.text.clone(),
                        summary_text: None,
                        asset_refs: Vec::new(),
                        caption: None,
                        section_path: Vec::new(),
                        source_locator: SourceLocator {
                            page: Some(unit.page),
                            ..SourceLocator::default()
                        },
                        parser_backend: primary_backend.clone(),
                        metadata: unit.metadata.clone(),
                    });
                }
                ParsedUnitKind::ImageWithContext => {
                    *page_text_chars.entry(unit.page).or_default() += unit.text.chars().count();
                    *page_image_count.entry(unit.page).or_default() += 1;

                    let asset_id = format!("{}-asset", unit.unit_id);
                    let image_path = unit.image_path.clone().unwrap_or_default();
                    document.assets.push(AssetIr {
                        asset_id: asset_id.clone(),
                        page: Some(unit.page),
                        asset_kind: AssetKind::Image,
                        storage_path: image_path.clone(),
                        mime_type: None,
                        width: None,
                        height: None,
                        parser_backend: primary_backend.clone(),
                        metadata: BTreeMap::new(),
                    });
                    document.blocks.push(BlockIr {
                        block_id: unit.unit_id.clone(),
                        page: Some(unit.page),
                        block_type: BlockType::Figure,
                        modality: BlockModality::ImageWithContext,
                        text: unit.text.clone(),
                        summary_text: Some(unit.text.clone()),
                        asset_refs: vec![asset_id],
                        caption: unit.caption.clone(),
                        section_path: Vec::new(),
                        source_locator: SourceLocator {
                            page: Some(unit.page),
                            ..SourceLocator::default()
                        },
                        parser_backend: primary_backend.clone(),
                        metadata: unit.metadata.clone(),
                    });
                }
            }
        }

        document.pages = page_numbers
            .into_iter()
            .map(|page_number| PageIr {
                page_number,
                width: None,
                height: None,
                backend: primary_backend.clone(),
                text_char_count: page_text_chars.remove(&page_number).unwrap_or_default(),
                image_count: page_image_count.remove(&page_number).unwrap_or_default(),
                metadata: BTreeMap::new(),
            })
            .collect();

        document
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PageIr {
    pub page_number: u32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub backend: ParseBackend,
    pub text_char_count: usize,
    pub image_count: usize,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Heading,
    #[default]
    Paragraph,
    ListItem,
    Table,
    Quote,
    Code,
    Figure,
    Caption,
    SlideText,
    SlideNotes,
    SlideImage,
    SheetTable,
    SheetCellRange,
}

impl BlockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::Paragraph => "paragraph",
            Self::ListItem => "list_item",
            Self::Table => "table",
            Self::Quote => "quote",
            Self::Code => "code",
            Self::Figure => "figure",
            Self::Caption => "caption",
            Self::SlideText => "slide_text",
            Self::SlideNotes => "slide_notes",
            Self::SlideImage => "slide_image",
            Self::SheetTable => "sheet_table",
            Self::SheetCellRange => "sheet_cell_range",
        }
    }

    pub fn supports_text_chunking(&self) -> bool {
        matches!(
            self,
            Self::Heading
                | Self::Paragraph
                | Self::ListItem
                | Self::Table
                | Self::Quote
                | Self::Code
                | Self::SlideText
                | Self::SlideNotes
                | Self::SheetTable
                | Self::SheetCellRange
        )
    }

    pub fn supports_multimodal_chunking(&self) -> bool {
        matches!(self, Self::Figure | Self::SlideImage)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlockModality {
    #[default]
    TextOnly,
    ImageWithContext,
}

impl BlockModality {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextOnly => "text_only",
            Self::ImageWithContext => "image_with_context",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BlockIr {
    pub block_id: String,
    pub page: Option<u32>,
    pub block_type: BlockType,
    pub modality: BlockModality,
    pub text: String,
    pub summary_text: Option<String>,
    pub asset_refs: Vec<String>,
    pub caption: Option<String>,
    pub section_path: Vec<String>,
    pub source_locator: SourceLocator,
    pub parser_backend: ParseBackend,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    #[default]
    Image,
    SlideRender,
}

impl AssetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::SlideRender => "slide_render",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AssetIr {
    pub asset_id: String,
    pub page: Option<u32>,
    pub asset_kind: AssetKind,
    pub storage_path: String,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub parser_backend: ParseBackend,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SourceLocator {
    pub page: Option<u32>,
    pub bbox: Option<[f32; 4]>,
    pub paragraph_index: Option<usize>,
    pub table_index: Option<usize>,
    pub sheet_name: Option<String>,
    pub row_range: Option<(u32, u32)>,
    pub col_range: Option<(u32, u32)>,
    pub slide_index: Option<u32>,
    pub shape_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ParseWarning {
    pub code: String,
    pub message: String,
    pub page: Option<u32>,
    pub backend: ParseBackend,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::parser::{NormalizedDocument, ParsedUnit};

    #[test]
    fn from_normalized_document_projects_text_and_image_units() {
        let normalized = NormalizedDocument {
            title: "spec".to_string(),
            units: vec![
                ParsedUnit::new_text(1, "hello".to_string(), "local".to_string()),
                ParsedUnit::new_image_with_context(
                    2,
                    "figure text".to_string(),
                    "image.png".to_string(),
                    Some("Figure 1".to_string()),
                    Some("nearby context".to_string()),
                    "local".to_string(),
                ),
            ],
            metadata: BTreeMap::new(),
        };

        let document = DocumentIr::from_normalized_document(
            "doc-1",
            DocumentType::Pdf,
            ParseBackend::EdgeParsePdf,
            &normalized,
        );

        assert_eq!(document.blocks.len(), 2);
        assert_eq!(document.assets.len(), 1);
        assert_eq!(document.pages.len(), 2);
        assert_eq!(document.blocks[0].modality, BlockModality::TextOnly);
        assert_eq!(document.blocks[1].modality, BlockModality::ImageWithContext);
        assert_eq!(document.blocks[1].asset_refs.len(), 1);
    }

    #[test]
    fn test_document_ir_snapshot() {
        let normalized = NormalizedDocument {
            title: "Snapshot Test Doc".to_string(),
            units: vec![
                ParsedUnit {
                    unit_id: "block-1".to_string(),
                    page: 1,
                    kind: ParsedUnitKind::Text,
                    text: "Page 1 text content".to_string(),
                    image_path: None,
                    caption: None,
                    context: None,
                    parser_backend: "local".to_string(),
                    metadata: BTreeMap::new(),
                },
                ParsedUnit {
                    unit_id: "block-2".to_string(),
                    page: 2,
                    kind: ParsedUnitKind::ImageWithContext,
                    text: "Page 2 image context".to_string(),
                    image_path: Some("img2.png".to_string()),
                    caption: Some("Figure 2.1".to_string()),
                    context: None,
                    parser_backend: "local".to_string(),
                    metadata: BTreeMap::new(),
                },
            ],
            metadata: BTreeMap::from([("author".to_string(), "Gemini".to_string())]),
        };

        let document = DocumentIr::from_normalized_document(
            "doc-snapshot",
            DocumentType::Pdf,
            ParseBackend::EdgeParsePdf,
            &normalized,
        );

        insta::assert_json_snapshot!(document);
    }
}
