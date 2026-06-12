use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};
use anyhow::{Context, Result};
use lopdf::Document;

pub(crate) struct MineruV4FileUploadPayload<'a> {
    pub(crate) bytes: Vec<u8>,
    pub(crate) page_numbers: Option<&'a [u32]>,
}

pub(crate) struct MineruV4PageUploadFile {
    pub(crate) filename: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) page_number: u32,
}

pub(crate) fn prepare_v4_file_upload_payload<'a>(
    bytes: &[u8],
    filename: &str,
    page_numbers: Option<&'a [u32]>,
) -> Result<MineruV4FileUploadPayload<'a>> {
    if let Some(pages) = page_numbers
        && filename.to_ascii_lowercase().ends_with(".pdf")
    {
        return Ok(MineruV4FileUploadPayload {
            bytes: extract_pdf_pages(bytes, pages)?,
            page_numbers: None,
        });
    }

    Ok(MineruV4FileUploadPayload {
        bytes: bytes.to_vec(),
        page_numbers,
    })
}

pub(crate) fn prepare_v4_ocr_page_upload(
    bytes: &[u8],
    filename: &str,
    page_number: u32,
) -> Result<Option<MineruV4PageUploadFile>> {
    let page_bytes = extract_pdf_pages(bytes, &[page_number])?;
    if super::fallback::is_low_value_pdf_upload_page(&page_bytes)? {
        return Ok(None);
    }

    Ok(Some(MineruV4PageUploadFile {
        filename: v4_page_upload_filename(filename, page_number),
        bytes: page_bytes,
        page_number,
    }))
}

pub(crate) fn v4_page_upload_filename(filename: &str, page_number: u32) -> String {
    let path = PathBuf::from(filename);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "document".to_string());
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "pdf".to_string());
    format!("{stem}-page-{page_number:04}.{extension}")
}

pub(crate) fn extract_pdf_pages(bytes: &[u8], page_numbers: &[u32]) -> Result<Vec<u8>> {
    if page_numbers.is_empty() {
        anyhow::bail!("MinerU v4 PDF upload page filter must not be empty");
    }
    if let [page_number] = page_numbers
        && let Ok(split) = extract_single_pdf_page_with_pdfseparate(bytes, *page_number)
    {
        return Ok(split);
    }

    let mut document =
        Document::load_mem(bytes).context("Failed to load PDF for MinerU page upload")?;
    let selected = page_numbers
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let pages = document.get_pages();
    for page_number in &selected {
        if !pages.contains_key(page_number) {
            anyhow::bail!("PDF page {} is not present for MinerU upload", page_number);
        }
    }

    let pages_to_delete = pages
        .keys()
        .copied()
        .filter(|page_number| !selected.contains(page_number))
        .collect::<Vec<_>>();
    document.delete_pages(&pages_to_delete);
    document.prune_objects();
    document.renumber_objects();

    let mut output = Vec::new();
    document
        .save_to(&mut output)
        .context("Failed to serialize MinerU page-filtered PDF upload")?;
    Ok(output)
}

pub(crate) fn extract_single_pdf_page_with_pdfseparate(bytes: &[u8], page_number: u32) -> Result<Vec<u8>> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_dir = std::env::temp_dir().join(format!(
        "avrag-mineru-pdf-split-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&temp_dir).with_context(|| {
        format!(
            "Failed to create MinerU PDF split temp dir {}",
            temp_dir.display()
        )
    })?;

    let result = (|| -> Result<Vec<u8>> {
        let input_path = temp_dir.join("input.pdf");
        fs::write(&input_path, bytes).with_context(|| {
            format!(
                "Failed to write MinerU PDF split input {}",
                input_path.display()
            )
        })?;
        let output_pattern = temp_dir.join("page-%d.pdf");
        let output = Command::new("pdfseparate")
            .arg("-f")
            .arg(page_number.to_string())
            .arg("-l")
            .arg(page_number.to_string())
            .arg(&input_path)
            .arg(&output_pattern)
            .output()
            .context("Failed to run pdfseparate for MinerU PDF page upload")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("pdfseparate failed for page {page_number}: {stderr}");
        }

        let output_path = temp_dir.join(format!("page-{page_number}.pdf"));
        fs::read(&output_path).with_context(|| {
            format!(
                "Failed to read MinerU PDF split output {}",
                output_path.display()
            )
        })
    })();
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

pub(crate) fn should_use_remote_extract_v4(source_url: Option<&str>) -> bool {
    source_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(is_http_source_url)
}

pub(crate) fn is_http_source_url(source_url: &str) -> bool {
    source_url.starts_with("http://") || source_url.starts_with("https://")
}

pub(crate) fn require_remote_source_url(source_url: Option<&str>, filename: &str) -> Result<String> {
    let source_url = source_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| {
            format!("MinerU v4 parse for {filename} requires a source URL, but none was provided")
        })?;
    if !is_http_source_url(source_url) {
        anyhow::bail!(
            "MinerU v4 requires an HTTP(S) source URL; got {} for {}",
            source_url,
            filename
        );
    }
    Ok(source_url.to_string())
}
