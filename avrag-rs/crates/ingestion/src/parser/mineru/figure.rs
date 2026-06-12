use serde::Deserialize;

use super::ParsedUnit;
use super::layout::looks_like_image_reference;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct ImageInfo {
    pub(crate) filename: String,
    pub(crate) url: String,
    pub(crate) page: u32,
    pub(crate) caption: Option<String>,
}

pub(crate) fn image_context(blocks: &[String], image: &ImageInfo) -> Option<String> {
    let caption = image.caption.as_deref().map(str::trim).unwrap_or("");
    let filename = image.filename.as_str();

    if let Some(index) = blocks.iter().position(|block| {
        block.contains(filename) || (!caption.is_empty() && block.contains(caption))
    }) {
        let start = index.saturating_sub(1);
        let end = (index + 2).min(blocks.len());
        let joined = blocks[start..end]
            .iter()
            .filter(|block| !looks_like_image_reference(block))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");
        return (!joined.trim().is_empty()).then_some(joined);
    }

    if !caption.is_empty() {
        return Some(caption.to_string());
    }

    blocks
        .iter()
        .find(|block| !looks_like_image_reference(block))
        .cloned()
}

pub(crate) fn figure_units_from_images(blocks: &[String], images: Vec<ImageInfo>) -> Vec<ParsedUnit> {
    let mut units = Vec::new();
    for image in images {
        let context = image_context(blocks, &image);
        let normalized_text = [
            image.caption.clone().unwrap_or_default(),
            context.clone().unwrap_or_default(),
            format!("[Image: {}]", image.filename),
        ]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

        units.push(ParsedUnit::new_image_with_context(
            image.page.max(1),
            normalized_text,
            image.url,
            image.caption,
            context,
            "mineru_precise".to_string(),
        ));
    }
    units
}

pub(crate) fn is_supported_image_file(path: &str) -> bool {
    path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".webp")
        || path.ends_with(".gif")
        || path.ends_with(".bmp")
}

pub(crate) fn infer_page_number_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let marker = lower.find("page")?;
    let suffix = &lower[marker + 4..];
    let digits = suffix
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}
