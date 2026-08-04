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
            "docx" | "doc" | "docm" | "odt" | "rtf" => Self::Docx,
            "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => Self::Xlsx,
            "ppt" | "pps" | "pot" => Self::Ppt,
            "pptx" | "pptm" | "ppsx" | "ppsm" | "odp" => Self::Pptx,
            "html" | "htm" => Self::Html,
            "rs" | "py" | "js" | "ts" | "tsx" | "go" | "java" | "c" | "cpp" | "h" => Self::Code,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Self::Image,
            "txt" | "md" | "rst" | "csv" | "tsv" | "json" | "toml" | "yaml" | "yml" | "epub" => {
                Self::Text
            }
            _ => Self::Unknown,
        }
    }
}

/// Parser backend recorded on document IR blocks and pages.
///
/// 2026-08-05 起按格式分工（见 `docs/plans/2026-08-05-parser-pipeline-anydoc.md`）：
/// PDF→`liteparse_v2_pdf`、Office/ODF/RTF/EPUB/CSV 等→`anydoc`、文本/代码→`markitdown`。
/// Variants prefixed with historical wire names (including `LiteParsePdf`,
/// `LiteParseFigure`, `CalamineExcel`, `EdgeParsePdf`, `Mineru*`, `OfficeDirect`) remain
/// for deserializing stored IR only — do not select them in new ingest paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParseBackend {
    /// Historical IR only (`edge_parse_pdf`). Do not emit on new ingest.
    EdgeParsePdf,
    /// Historical IR only (raster-render PDF pages). Do not emit on new ingest.
    VisualRasterPdf,
    /// Standalone image files via PaddleOCR（现役，图片路径）。
    PaddleOcrPdf,
    /// Historical IR only (LiteParse digital text). Do not emit on new ingest.
    LiteParsePdf,
    /// Historical IR only (LiteParse figure enrichment). Do not emit on new ingest.
    LiteParseFigure,
    /// Historical IR only (pre-P4 MinerU OCR PDF). Do not emit on new ingest.
    MineruPdfOcr,
    /// Historical IR only (pre-P4 MinerU image OCR). Do not emit on new ingest.
    MineruImage,
    /// Historical IR only (office service 时代 docx 后端)。Do not emit on new ingest.
    Docx4jDocx,
    /// Historical IR only (office service 时代 xlsx/pptx/ppt 后端)。Do not emit on new ingest.
    PoiXlsx,
    PoiPptx,
    PoiPpt,
    HtmlLocal,
    TextLocal,
    CodeLocal,
    /// Historical IR only (calamine 进程内 Excel 解析)。Do not emit on new ingest.
    CalamineExcel,
    /// markitdown subprocess parse → markdown（现役：anydoc 不支持的文本/代码长尾；
    /// txt/md/rst/tsv/json/toml/yaml/yml/html/htm/code route here）。
    Markitdown,
    /// liteparse PDFium native parse → markdown（现役：PDF 路径）。
    /// Wire name 与历史 `LiteParsePdf` 区分——新路径产 markdown→Paragraph/Heading
    /// 块（无 bbox），与旧 liteparse 行块形态不同，需可区分。
    LiteparseV2Pdf,
    /// Historical IR only（2026-08-02 office-direct 路径）。Do not emit on new ingest.
    OfficeDirect,
    /// anydoc subprocess → GFM markdown（现役：Office/ODF/RTF/EPUB/CSV 等，非 PDF）。
    Anydoc,
    #[default]
    Unknown,
}

impl ParseBackend {
    /// Whether this variant is retained only for historical stored IR / metadata.
    pub const fn is_historical_ir_only(self) -> bool {
        matches!(
            self,
            Self::EdgeParsePdf
                | Self::VisualRasterPdf
                | Self::LiteParsePdf
                | Self::LiteParseFigure
                | Self::CalamineExcel
                | Self::Docx4jDocx
                | Self::PoiXlsx
                | Self::PoiPptx
                | Self::PoiPpt
                | Self::MineruPdfOcr
                | Self::MineruImage
                | Self::OfficeDirect
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EdgeParsePdf => "edge_parse_pdf",
            Self::VisualRasterPdf => "visual_raster_pdf",
            Self::PaddleOcrPdf => "paddle_ocr_pdf",
            Self::LiteParsePdf => "liteparse_pdf",
            Self::LiteParseFigure => "liteparse_figure",
            Self::MineruPdfOcr => "mineru_pdf_ocr",
            Self::MineruImage => "mineru_image",
            Self::Docx4jDocx => "docx4j_docx",
            Self::PoiXlsx => "poi_xlsx",
            Self::PoiPptx => "poi_pptx",
            Self::PoiPpt => "poi_ppt",
            Self::HtmlLocal => "html_local",
            Self::TextLocal => "text_local",
            Self::CodeLocal => "code_local",
            Self::CalamineExcel => "calamine_excel",
            Self::Markitdown => "markitdown",
            Self::LiteparseV2Pdf => "liteparse_v2_pdf",
            Self::OfficeDirect => "office_direct",
            Self::Anydoc => "anydoc",
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
                    // T3: CSV/TSV pre-parsed by TextParser → ONE Table block.
                    if primary_backend == ParseBackend::TextLocal
                        && let Some(json) = normalized.metadata.get("csv_table_ir")
                        && let Ok(table) = serde_json::from_str::<TableIr>(json)
                    {
                        let mut metadata = unit.metadata.clone();
                        metadata.insert(TableIr::METADATA_KEY.to_string(), json.clone());
                        metadata.insert("table_parser".to_string(), "csv-v1".to_string());
                        document.blocks.push(BlockIr {
                            block_id: unit.unit_id.clone(),
                            page: Some(unit.page),
                            block_type: BlockType::Table,
                            modality: BlockModality::TextOnly,
                            text: table.to_markdown(),
                            alt_text: None,
                            asset_refs: Vec::new(),
                            caption: table.caption.clone(),
                            section_path: Vec::new(),
                            source_locator: SourceLocator {
                                page: Some(unit.page),
                                table_index: Some(0),
                                row_range: Some((1, table.rows.len().max(1) as u32)),
                                ..SourceLocator::default()
                            },
                            parser_backend: primary_backend.clone(),
                            metadata,
                        });
                        continue;
                    }
                    // T1: the TextLocal path segments the flat text so table
                    // regions become BlockType::Table blocks (structured
                    // TableIr in metadata) while prose stays paragraphs.
                    // Every other backend keeps the one-block-per-unit shape.
                    if primary_backend == ParseBackend::TextLocal {
                        let segments = crate::parser::text_table::segment_text(&unit.text);
                        for (seg_index, segment) in segments.into_iter().enumerate() {
                            let is_table = segment.table.is_some();
                            let mut metadata = unit.metadata.clone();
                            let mut source_locator = SourceLocator {
                                page: Some(unit.page),
                                ..SourceLocator::default()
                            };
                            let mut caption = None;
                            if let Some(table) = &segment.table {
                                metadata.insert(
                                    TableIr::METADATA_KEY.to_string(),
                                    serde_json::to_string(table)
                                        .unwrap_or_else(|_| "{}".to_string()),
                                );
                                metadata.insert(
                                    "table_parser".to_string(),
                                    "text-table-v1".to_string(),
                                );
                                source_locator.table_index = Some(seg_index);
                                caption = table.caption.clone();
                            }
                            document.blocks.push(BlockIr {
                                block_id: format!("{}-seg{seg_index}", unit.unit_id),
                                page: Some(unit.page),
                                block_type: if is_table {
                                    BlockType::Table
                                } else {
                                    BlockType::Paragraph
                                },
                                modality: BlockModality::TextOnly,
                                text: segment.text,
                                alt_text: None,
                                asset_refs: Vec::new(),
                                caption,
                                section_path: Vec::new(),
                                source_locator,
                                parser_backend: primary_backend.clone(),
                                metadata,
                            });
                        }
                    } else {
                        document.blocks.push(BlockIr {
                            block_id: unit.unit_id.clone(),
                            page: Some(unit.page),
                            block_type: BlockType::Paragraph,
                            modality: BlockModality::TextOnly,
                            text: unit.text.clone(),
                            alt_text: None,
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
                        alt_text: Some(unit.text.clone()),
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

/// T1 (2026-07-28, table-aware ingestion §4.1): parse confidence of a
/// structured table. High = deterministic source (txt/md whitelist, csv,
/// xlsx grid); Medium = reconstructed (PDF bbox); Low = best-effort.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TableConfidence {
    #[default]
    High,
    Medium,
    Low,
}

/// T1: structured table content carried by a `BlockType::Table` block.
///
/// The block's flat `text` stays the markdown serialization (embedding /
/// lexical search surface); this structured form rides in
/// `block.metadata["table_ir"]` as JSON (document_blocks.metadata_json —
/// zero schema migration). Never emitted unless the parser's self-validation
/// passed — degraded parses stay plain text instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TableIr {
    /// Table title/caption from neighbouring text, when identifiable.
    pub caption: Option<String>,
    /// Column headers (positional `col_1..n` when the source has none).
    pub headers: Vec<String>,
    /// Grid cells, one Vec per data row.
    pub rows: Vec<Vec<String>>,
    pub parse_confidence: TableConfidence,
    /// Parse diagnostics (source gaps / irregular rows / merged cells).
    pub notes: Vec<String>,
}

/// W6 行级证据：block/chunk `metadata` 的 md 源行区间键。
///
/// 值为 0-based 行号，**闭区间** `[md_line_start, md_line_end]`，坐标系是与
/// struct-supervision 表格行 `__src_line` 相同的**同一份 markitdown markdown
/// 文本**（仅 markitdown 解析路径写入；其它 backend 的 block 无此键，chunk
/// 聚合时缺键即降级不写）。语义与同文件 `row_range` 的 inclusive 约定一致。
pub const MD_LINE_START_KEY: &str = "md_line_start";
pub const MD_LINE_END_KEY: &str = "md_line_end";

impl TableIr {
    /// Metadata key under which the JSON-serialized TableIr rides on a block.
    pub const METADATA_KEY: &'static str = "table_ir";

    /// Deserialize the structured table from a block's metadata, if present.
    pub fn from_block(block: &BlockIr) -> Option<Self> {
        serde_json::from_str(block.metadata.get(Self::METADATA_KEY)?).ok()
    }

    /// Full markdown serialization (header + separator + all data rows).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        if let Some(caption) = &self.caption {
            out.push_str(caption);
            out.push('\n');
        }
        out.push_str(&self.header_markdown());
        for row in &self.rows {
            out.push('\n');
            out.push_str(&Self::cells_markdown(row));
        }
        out
    }

    /// Header row + markdown separator line.
    pub fn header_markdown(&self) -> String {
        let mut out = Self::cells_markdown(&self.headers);
        out.push('\n');
        out.push_str(&Self::cells_markdown(
            &self
                .headers
                .iter()
                .map(|_| "---".to_string())
                .collect::<Vec<_>>(),
        ));
        out
    }

    /// One data row as a markdown table line.
    pub fn row_markdown(&self, row_index: usize) -> String {
        Self::cells_markdown(&self.rows[row_index])
    }

    fn cells_markdown(cells: &[String]) -> String {
        let mut out = String::from("|");
        for cell in cells {
            out.push_str(&cell.replace('|', "\\|"));
            out.push('|');
        }
        out
    }
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
    PageRaster,
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
            Self::PageRaster => "page_raster",
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
        matches!(self, Self::Figure | Self::SlideImage | Self::PageRaster)
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
    #[serde(rename = "summary_text")]
    pub alt_text: Option<String>,
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
