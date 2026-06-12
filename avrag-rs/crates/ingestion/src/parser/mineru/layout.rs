use super::ParsedUnit;

pub(crate) fn markdown_blocks(markdown: &str) -> Vec<String> {
    markdown
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn looks_like_image_reference(block: &str) -> bool {
    let lower = block.to_lowercase();
    lower.starts_with("![")
        || lower.contains("<img")
        || lower.contains(".png")
        || lower.contains(".jpg")
}

pub(crate) fn title_from_blocks(blocks: &[String], filename: &str) -> String {
    blocks
        .iter()
        .find_map(|block| block.strip_prefix("# ").map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| filename.to_string())
}

pub(crate) fn text_units_from_blocks(blocks: &[String]) -> Vec<ParsedUnit> {
    blocks
        .iter()
        .filter(|block| !looks_like_image_reference(block))
        .map(|block| ParsedUnit::new_text(1, block.clone(), "mineru_precise".to_string()))
        .collect()
}
