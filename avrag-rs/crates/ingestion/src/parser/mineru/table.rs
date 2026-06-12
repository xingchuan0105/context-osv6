use std::io::{Cursor, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use zip::ZipArchive;

use super::figure::ImageInfo;

pub(crate) fn build_file_upload_batch_payload_v4(
    filename: &str,
    page_numbers: Option<&[u32]>,
    is_ocr: bool,
) -> serde_json::Value {
    let mut file = serde_json::json!({ "name": filename });
    if let Some(page_numbers) = page_numbers {
        file["page_ranges"] = serde_json::json!(format_page_ranges(page_numbers));
    }
    if is_ocr {
        file["is_ocr"] = serde_json::json!(true);
    }

    serde_json::json!({
        "files": [file],
        "model_version": "vlm",
    })
}

pub(crate) fn build_file_upload_batch_payload_v4_files(
    filenames: &[String],
    is_ocr: bool,
) -> serde_json::Value {
    let files = filenames
        .iter()
        .map(|filename| {
            let mut file = serde_json::json!({ "name": filename });
            if is_ocr {
                file["is_ocr"] = serde_json::json!(true);
            }
            file
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "files": files,
        "model_version": "vlm",
    })
}

pub(crate) fn format_page_ranges(page_numbers: &[u32]) -> String {
    if page_numbers.is_empty() {
        return String::new();
    }

    let mut numbers = page_numbers.to_vec();
    numbers.sort_unstable();
    numbers.dedup();

    let mut ranges = Vec::new();
    let mut range_start = numbers[0];
    let mut previous = numbers[0];

    for current in numbers.into_iter().skip(1) {
        if current == previous + 1 {
            previous = current;
            continue;
        }
        if range_start == previous {
            ranges.push(range_start.to_string());
        } else {
            ranges.push(format!("{range_start}-{previous}"));
        }
        range_start = current;
        previous = current;
    }

    if range_start == previous {
        ranges.push(range_start.to_string());
    } else {
        ranges.push(format!("{range_start}-{previous}"));
    }

    ranges.join(",")
}

pub(crate) fn extract_markdown_and_images_from_zip(
    bytes: &[u8],
    task_id: &str,
) -> Result<(String, Vec<ImageInfo>)> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        ZipArchive::new(cursor).context("Failed to open MinerU v4 zip payload archive")?;

    let mut markdown: Option<String> = None;
    let mut images = Vec::new();
    let image_dir = std::env::temp_dir()
        .join("mineru-v4")
        .join(format!("task-{task_id}"));
    std::fs::create_dir_all(&image_dir).with_context(|| {
        format!(
            "Failed to create MinerU temporary image directory {}",
            image_dir.display()
        )
    })?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("Failed to read zip entry #{index}"))?;
        if file.is_dir() {
            continue;
        }

        let entry_name = file.name().to_string();
        let lower = entry_name.to_ascii_lowercase();
        if lower.ends_with(".md") {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .context("Failed to read markdown from MinerU v4 zip")?;
            if markdown.is_none() || !content.trim().is_empty() {
                markdown = Some(content);
            }
            continue;
        }

        if !super::figure::is_supported_image_file(&lower) {
            continue;
        }

        let file_name = PathBuf::from(&entry_name)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("image-{index}.bin"));
        let file_path = image_dir.join(&file_name);

        let mut image_bytes = Vec::new();
        file.read_to_end(&mut image_bytes)
            .context("Failed to read image bytes from MinerU v4 zip")?;
        std::fs::write(&file_path, &image_bytes).with_context(|| {
            format!(
                "Failed to write temporary MinerU image {}",
                file_path.display()
            )
        })?;

        images.push(ImageInfo {
            filename: file_name.clone(),
            url: format!("temporary://{}", file_path.to_string_lossy()),
            page: super::figure::infer_page_number_from_name(&entry_name).unwrap_or(1),
            caption: None,
        });
    }

    let markdown = markdown.context("MinerU v4 zip payload does not contain markdown output")?;
    Ok((markdown, images))
}
