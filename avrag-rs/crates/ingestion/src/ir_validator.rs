use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::ir::{AssetKind, BlockModality, BlockType, DocumentIr, DocumentType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentIrValidationIssue {
    pub code: String,
    pub message: String,
    pub block_id: Option<String>,
    pub asset_id: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DocumentIrValidationReport {
    pub document: DocumentIr,
    pub warnings: Vec<DocumentIrValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentIrValidationOptions {
    pub strip_nul_bytes: bool,
    pub normalize_line_endings: bool,
}

impl Default for DocumentIrValidationOptions {
    fn default() -> Self {
        Self {
            strip_nul_bytes: true,
            normalize_line_endings: true,
        }
    }
}

#[derive(Debug, Error)]
#[error("document ir validation failed with {issues_len} issue(s): {first_message}")]
pub struct DocumentIrValidationError {
    pub issues: Vec<DocumentIrValidationIssue>,
    issues_len: usize,
    first_message: String,
}

impl DocumentIrValidationError {
    fn new(issues: Vec<DocumentIrValidationIssue>) -> Self {
        let issues_len = issues.len();
        let first_message = issues
            .first()
            .map(|issue| issue.message.clone())
            .unwrap_or_else(|| "unknown validation failure".to_string());
        Self {
            issues,
            issues_len,
            first_message,
        }
    }
}

pub fn sanitize_and_validate_document_ir(
    mut document: DocumentIr,
    options: &DocumentIrValidationOptions,
) -> Result<DocumentIrValidationReport, DocumentIrValidationError> {
    let warnings = sanitize_document_ir(&mut document, options);
    validate_document_ir(&document)?;
    Ok(DocumentIrValidationReport { document, warnings })
}

pub fn sanitize_document_ir(
    document: &mut DocumentIr,
    options: &DocumentIrValidationOptions,
) -> Vec<DocumentIrValidationIssue> {
    let mut warnings = Vec::new();

    sanitize_string_field(&mut document.title, options, &mut warnings, None, None);
    for value in document.metadata.values_mut() {
        sanitize_string_field(value, options, &mut warnings, None, None);
    }
    for warning in &mut document.warnings {
        sanitize_string_field(
            &mut warning.message,
            options,
            &mut warnings,
            None,
            warning.page,
        );
    }
    for asset in &mut document.assets {
        sanitize_string_field(
            &mut asset.storage_path,
            options,
            &mut warnings,
            None,
            asset.page,
        );
        for value in asset.metadata.values_mut() {
            sanitize_string_field(value, options, &mut warnings, None, asset.page);
        }
    }
    for block in &mut document.blocks {
        let block_id = Some(block.block_id.clone());
        sanitize_string_field(
            &mut block.text,
            options,
            &mut warnings,
            block_id.clone(),
            block.page,
        );
        if let Some(summary_text) = &mut block.summary_text {
            sanitize_string_field(
                summary_text,
                options,
                &mut warnings,
                block_id.clone(),
                block.page,
            );
        }
        if let Some(caption) = &mut block.caption {
            sanitize_string_field(
                caption,
                options,
                &mut warnings,
                block_id.clone(),
                block.page,
            );
        }
        for section in &mut block.section_path {
            sanitize_string_field(
                section,
                options,
                &mut warnings,
                block_id.clone(),
                block.page,
            );
        }
        for value in block.metadata.values_mut() {
            sanitize_string_field(value, options, &mut warnings, block_id.clone(), block.page);
        }
    }

    warnings
}

pub fn validate_document_ir(document: &DocumentIr) -> Result<(), DocumentIrValidationError> {
    let mut issues = Vec::new();

    let mut block_ids = BTreeSet::new();
    for block in &document.blocks {
        if block.block_id.trim().is_empty() {
            issues.push(issue(
                "empty_block_id",
                "block_id must not be empty",
                Some(block.block_id.clone()),
                None,
                block.page,
            ));
        } else if !block_ids.insert(block.block_id.clone()) {
            issues.push(issue(
                "duplicate_block_id",
                format!("duplicate block_id {}", block.block_id),
                Some(block.block_id.clone()),
                None,
                block.page,
            ));
        }

        if contains_nul(&block.text) {
            issues.push(issue(
                "nul_in_block_text",
                format!("block {} contains NUL bytes in text", block.block_id),
                Some(block.block_id.clone()),
                None,
                block.page,
            ));
        }
        if block.summary_text.as_deref().is_some_and(contains_nul) {
            issues.push(issue(
                "nul_in_summary_text",
                format!(
                    "block {} contains NUL bytes in summary_text",
                    block.block_id
                ),
                Some(block.block_id.clone()),
                None,
                block.page,
            ));
        }

        if matches!(block.modality, BlockModality::ImageWithContext) && block.asset_refs.is_empty()
        {
            issues.push(issue(
                "image_block_missing_asset_refs",
                format!("image block {} is missing asset refs", block.block_id),
                Some(block.block_id.clone()),
                None,
                block.page,
            ));
        }

        if document.doc_type == DocumentType::Pdf
            && block.page.is_none()
            && block.source_locator.page.is_none()
        {
            issues.push(issue(
                "pdf_block_missing_page",
                format!("pdf block {} is missing page metadata", block.block_id),
                Some(block.block_id.clone()),
                None,
                None,
            ));
        }

        if matches!(block.block_type, BlockType::SlideImage) && block.asset_refs.is_empty() {
            issues.push(issue(
                "slide_image_missing_asset",
                format!(
                    "slide image block {} is missing slide render asset",
                    block.block_id
                ),
                Some(block.block_id.clone()),
                None,
                block.page,
            ));
        }
    }

    let mut asset_ids = BTreeSet::new();
    for asset in &document.assets {
        if asset.asset_id.trim().is_empty() {
            issues.push(issue(
                "empty_asset_id",
                "asset_id must not be empty",
                None,
                Some(asset.asset_id.clone()),
                asset.page,
            ));
        } else if !asset_ids.insert(asset.asset_id.clone()) {
            issues.push(issue(
                "duplicate_asset_id",
                format!("duplicate asset_id {}", asset.asset_id),
                None,
                Some(asset.asset_id.clone()),
                asset.page,
            ));
        }
    }

    for block in &document.blocks {
        for asset_ref in &block.asset_refs {
            if !asset_ids.contains(asset_ref) {
                issues.push(issue(
                    "unknown_asset_ref",
                    format!(
                        "block {} references unknown asset {}",
                        block.block_id, asset_ref
                    ),
                    Some(block.block_id.clone()),
                    Some(asset_ref.clone()),
                    block.page,
                ));
            }
        }
    }

    if matches!(document.doc_type, DocumentType::Ppt | DocumentType::Pptx) {
        validate_presentation_contract(document, &mut issues);
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(DocumentIrValidationError::new(issues))
    }
}

fn sanitize_string_field(
    value: &mut String,
    options: &DocumentIrValidationOptions,
    warnings: &mut Vec<DocumentIrValidationIssue>,
    block_id: Option<String>,
    page: Option<u32>,
) {
    if options.strip_nul_bytes && value.contains('\0') {
        *value = value.replace('\0', "");
        warnings.push(issue(
            "nul_bytes_stripped",
            "stripped NUL bytes from document ir field".to_string(),
            block_id,
            None,
            page,
        ));
    }
    if options.normalize_line_endings && (value.contains("\r\n") || value.contains('\r')) {
        *value = value.replace("\r\n", "\n").replace('\r', "\n");
    }
}

fn contains_nul(value: &str) -> bool {
    value.contains('\0')
}

fn validate_presentation_contract(
    document: &DocumentIr,
    issues: &mut Vec<DocumentIrValidationIssue>,
) {
    let has_slide_text = document.blocks.iter().any(|block| {
        matches!(
            block.block_type,
            BlockType::SlideText | BlockType::SlideNotes
        ) && !block.text.trim().is_empty()
    });
    if !has_slide_text {
        issues.push(issue(
            "presentation_missing_slide_text",
            "presentation documents must include at least one slide_text or slide_notes block",
            None,
            None,
            None,
        ));
    }

    let has_slide_image = document
        .blocks
        .iter()
        .any(|block| matches!(block.block_type, BlockType::SlideImage));
    if !has_slide_image {
        issues.push(issue(
            "presentation_missing_slide_image",
            "presentation documents must include at least one slide_image block",
            None,
            None,
            None,
        ));
    }

    let mut pages = BTreeSet::new();
    for page in &document.pages {
        pages.insert(page.page_number);
    }
    for block in &document.blocks {
        if let Some(page) = block.page.or(block.source_locator.page) {
            pages.insert(page);
        }
    }
    for asset in &document.assets {
        if let Some(page) = asset.page {
            pages.insert(page);
        }
    }

    let asset_by_id = document
        .assets
        .iter()
        .map(|asset| (asset.asset_id.as_str(), asset))
        .collect::<BTreeMap<_, _>>();

    for page in pages {
        let slide_render_assets = document
            .assets
            .iter()
            .filter(|asset| asset.page == Some(page) && asset.asset_kind == AssetKind::SlideRender)
            .collect::<Vec<_>>();

        if slide_render_assets.len() != 1 {
            issues.push(issue(
                "presentation_slide_render_count_invalid",
                format!(
                    "presentation page {} must have exactly one slide_render asset",
                    page
                ),
                None,
                None,
                Some(page),
            ));
            continue;
        }

        let slide_render_asset = slide_render_assets[0];
        let referenced_by_slide_image = document.blocks.iter().any(|block| {
            matches!(block.block_type, BlockType::SlideImage)
                && block.page.or(block.source_locator.page) == Some(page)
                && block.asset_refs.iter().any(|asset_ref| {
                    asset_by_id
                        .get(asset_ref.as_str())
                        .is_some_and(|asset| asset.asset_id == slide_render_asset.asset_id)
                })
        });

        if !referenced_by_slide_image {
            issues.push(issue(
                "presentation_slide_image_missing_render_ref",
                format!(
                    "presentation page {} must have a slide_image block that references slide_render asset {}",
                    page, slide_render_asset.asset_id
                ),
                None,
                Some(slide_render_asset.asset_id.clone()),
                Some(page),
            ));
        }
    }
}

fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
    block_id: Option<String>,
    asset_id: Option<String>,
    page: Option<u32>,
) -> DocumentIrValidationIssue {
    DocumentIrValidationIssue {
        code: code.into(),
        message: message.into(),
        block_id,
        asset_id,
        page,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ir::{
        AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType,
        ParseBackend, SourceLocator,
    };

    fn base_document() -> DocumentIr {
        DocumentIr {
            document_id: "doc-1".to_string(),
            title: "doc".to_string(),
            doc_type: DocumentType::Pdf,
            primary_backend: ParseBackend::EdgeParsePdf,
            backend_version: None,
            language: None,
            metadata: BTreeMap::new(),
            pages: Vec::new(),
            blocks: vec![BlockIr {
                block_id: "block-1".to_string(),
                page: Some(1),
                block_type: BlockType::Paragraph,
                modality: BlockModality::TextOnly,
                text: "hello".to_string(),
                summary_text: None,
                asset_refs: Vec::new(),
                caption: None,
                section_path: Vec::new(),
                source_locator: SourceLocator {
                    page: Some(1),
                    ..SourceLocator::default()
                },
                parser_backend: ParseBackend::EdgeParsePdf,
                metadata: BTreeMap::new(),
            }],
            assets: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn validate_document_ir_rejects_duplicate_block_ids() {
        let mut document = base_document();
        document.blocks.push(document.blocks[0].clone());

        let error = validate_document_ir(&document).expect_err("should reject duplicate ids");
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.code == "duplicate_block_id")
        );
    }

    #[test]
    fn validate_document_ir_rejects_missing_asset_ref_targets() {
        let mut document = base_document();
        document.blocks[0].modality = BlockModality::ImageWithContext;
        document.blocks[0].block_type = BlockType::Figure;
        document.blocks[0]
            .asset_refs
            .push("missing-asset".to_string());

        let error = validate_document_ir(&document).expect_err("should reject unknown asset refs");
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.code == "unknown_asset_ref")
        );
    }

    #[test]
    fn sanitize_and_validate_document_ir_strips_nul_bytes() {
        let mut document = base_document();
        document.title = "bad\0title".to_string();
        document.blocks[0].text = "hello\0world".to_string();

        let report =
            sanitize_and_validate_document_ir(document, &DocumentIrValidationOptions::default())
                .expect("sanitized document should validate");

        assert_eq!(report.document.title, "badtitle");
        assert_eq!(report.document.blocks[0].text, "helloworld");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.code == "nul_bytes_stripped")
        );
    }

    #[test]
    fn validate_document_ir_accepts_resolved_image_blocks() {
        let mut document = base_document();
        document.blocks[0].modality = BlockModality::ImageWithContext;
        document.blocks[0].block_type = BlockType::Figure;
        document.blocks[0].summary_text = Some("summary".to_string());
        document.blocks[0].asset_refs.push("asset-1".to_string());
        document.assets.push(AssetIr {
            asset_id: "asset-1".to_string(),
            page: Some(1),
            asset_kind: AssetKind::Image,
            storage_path: "image.png".to_string(),
            mime_type: None,
            width: None,
            height: None,
            parser_backend: ParseBackend::EdgeParsePdf,
            metadata: BTreeMap::new(),
        });

        validate_document_ir(&document).expect("resolved image block should validate");
    }

    fn base_presentation_document() -> DocumentIr {
        DocumentIr {
            document_id: "deck-1".to_string(),
            title: "deck".to_string(),
            doc_type: DocumentType::Pptx,
            primary_backend: ParseBackend::PoiPptx,
            backend_version: None,
            language: None,
            metadata: BTreeMap::new(),
            pages: vec![crate::ir::PageIr {
                page_number: 1,
                width: None,
                height: None,
                backend: ParseBackend::PoiPptx,
                text_char_count: 10,
                image_count: 1,
                metadata: BTreeMap::new(),
            }],
            blocks: vec![
                BlockIr {
                    block_id: "slide-1-text".to_string(),
                    page: Some(1),
                    block_type: BlockType::SlideText,
                    modality: BlockModality::TextOnly,
                    text: "Agenda".to_string(),
                    summary_text: None,
                    asset_refs: Vec::new(),
                    caption: None,
                    section_path: Vec::new(),
                    source_locator: SourceLocator {
                        page: Some(1),
                        slide_index: Some(1),
                        ..SourceLocator::default()
                    },
                    parser_backend: ParseBackend::PoiPptx,
                    metadata: BTreeMap::new(),
                },
                BlockIr {
                    block_id: "slide-1-image".to_string(),
                    page: Some(1),
                    block_type: BlockType::SlideImage,
                    modality: BlockModality::ImageWithContext,
                    text: "Agenda slide".to_string(),
                    summary_text: Some("Agenda slide render".to_string()),
                    asset_refs: vec!["slide-render-1".to_string()],
                    caption: Some("Agenda slide".to_string()),
                    section_path: Vec::new(),
                    source_locator: SourceLocator {
                        page: Some(1),
                        slide_index: Some(1),
                        ..SourceLocator::default()
                    },
                    parser_backend: ParseBackend::PoiPptx,
                    metadata: BTreeMap::new(),
                },
            ],
            assets: vec![AssetIr {
                asset_id: "slide-render-1".to_string(),
                page: Some(1),
                asset_kind: AssetKind::SlideRender,
                storage_path: "temporary://slide-1.png".to_string(),
                mime_type: Some("image/png".to_string()),
                width: Some(1280),
                height: Some(720),
                parser_backend: ParseBackend::PoiPptx,
                metadata: BTreeMap::new(),
            }],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn validate_document_ir_rejects_presentation_without_slide_render_asset() {
        let mut document = base_presentation_document();
        document.assets.clear();

        let error = validate_document_ir(&document).expect_err("missing slide render should fail");
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.code == "presentation_slide_render_count_invalid")
        );
    }

    #[test]
    fn validate_document_ir_rejects_presentation_without_slide_image_block() {
        let mut document = base_presentation_document();
        document
            .blocks
            .retain(|block| block.block_type != BlockType::SlideImage);

        let error = validate_document_ir(&document).expect_err("missing slide image should fail");
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.code == "presentation_missing_slide_image")
        );
    }

    #[test]
    fn validate_document_ir_accepts_complete_presentation_contract() {
        let document = base_presentation_document();
        validate_document_ir(&document).expect("complete presentation contract should validate");
    }
}
