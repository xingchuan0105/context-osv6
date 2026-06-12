use std::path::{Path, PathBuf};

use common::AppError;
use ingestion::parser::{DocumentParser, HtmlParser};
use reqwest::{Client as HttpClient, Url, header::CONTENT_TYPE, redirect::Policy};
use tokio::{fs, time::Duration};

use crate::helpers::sanitize_filename;

const URL_IMPORT_MAX_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct UrlImportPayload {
    pub filename: String,
    pub mime_type: String,
    pub raw_bytes: Vec<u8>,
    pub extracted_content: String,
}

pub async fn fetch_url_import(raw_url: &str) -> Result<UrlImportPayload, AppError> {
    let url = Url::parse(raw_url)
        .map_err(|_| AppError::validation("invalid_url", "url must be a valid absolute URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::validation(
            "invalid_url_scheme",
            "url must start with http:// or https://",
        ));
    }

    let client = HttpClient::builder()
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(20))
        .user_agent("avrag-url-import/1.0")
        .build()
        .map_err(|error| AppError::internal(format!("failed to build url importer: {error}")))?;

    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| AppError::validation("url_fetch_failed", error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::validation(
            "url_fetch_failed",
            format!("url returned HTTP {status}"),
        ));
    }
    if response
        .content_length()
        .is_some_and(|len| len as usize > URL_IMPORT_MAX_BYTES)
    {
        return Err(AppError::validation(
            "url_too_large",
            format!(
                "url content exceeds {} MB",
                URL_IMPORT_MAX_BYTES / 1024 / 1024
            ),
        ));
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let raw_bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::validation("url_fetch_failed", error.to_string()))?
        .to_vec();
    if raw_bytes.is_empty() {
        return Err(AppError::validation(
            "url_empty_content",
            "the fetched url returned empty content",
        ));
    }
    if raw_bytes.len() > URL_IMPORT_MAX_BYTES {
        return Err(AppError::validation(
            "url_too_large",
            format!(
                "url content exceeds {} MB",
                URL_IMPORT_MAX_BYTES / 1024 / 1024
            ),
        ));
    }

    let mime_type = infer_url_import_mime_type(&content_type, &raw_bytes).to_string();
    let provisional_filename = build_url_source_filename(&final_url, &mime_type, None);
    let (extracted_content, title_hint) =
        extract_url_import_content(&raw_bytes, &mime_type, &provisional_filename).await?;
    let extracted_content = normalize_imported_text(&extracted_content);
    if extracted_content.is_empty() {
        return Err(AppError::validation(
            "url_empty_content",
            "the fetched url did not contain readable text",
        ));
    }

    Ok(UrlImportPayload {
        filename: build_url_source_filename(&final_url, &mime_type, title_hint.as_deref()),
        mime_type,
        raw_bytes,
        extracted_content,
    })
}

pub fn infer_url_import_mime_type(content_type: &str, bytes: &[u8]) -> &'static str {
    let normalized = content_type
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if normalized.contains("html") || looks_like_html(bytes) {
        "text/html"
    } else if normalized.contains("json") {
        "application/json"
    } else if normalized.contains("xml") {
        "application/xml"
    } else {
        "text/plain"
    }
}

pub async fn extract_url_import_content(
    bytes: &[u8],
    mime_type: &str,
    filename: &str,
) -> Result<(String, Option<String>), AppError> {
    if mime_type == "text/html" {
        let parsed = HtmlParser
            .parse(bytes, filename)
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let content = parsed
            .pages
            .iter()
            .map(|page| page.content.trim())
            .filter(|page| !page.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let title = (!parsed.title.trim().is_empty()).then_some(parsed.title);
        return Ok((content, title));
    }

    Ok((String::from_utf8_lossy(bytes).into_owned(), None))
}

pub fn looks_like_html(bytes: &[u8]) -> bool {
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    prefix.contains("<html")
        || prefix.contains("<body")
        || prefix.contains("<article")
        || prefix.contains("<!doctype html")
}

pub fn build_url_source_filename(
    url: &Url,
    mime_type: &str,
    title_hint: Option<&str>,
) -> String {
    let extension = match mime_type {
        "text/html" => "html",
        "application/json" => "json",
        "application/xml" => "xml",
        _ => "txt",
    };
    let base = title_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_filename)
        .or_else(|| {
            url.path_segments()
                .and_then(|segments| segments.rev().find(|segment| !segment.trim().is_empty()))
                .map(sanitize_filename)
                .filter(|value| !value.is_empty())
        })
        .filter(|value| value != "." && value != "..")
        .unwrap_or_else(|| {
            url.host_str()
                .map(sanitize_filename)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "url-source".to_string())
        });

    if base
        .rsplit('.')
        .next()
        .is_some_and(|value| value == extension)
    {
        base
    } else {
        format!("{base}.{extension}")
    }
}

pub fn normalize_imported_text(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn write_raw_object(
    object_root: &Path,
    object_path: &str,
    bytes: &[u8],
) -> std::io::Result<()> {
    let full_path = object_root.join(PathBuf::from(object_path));
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(full_path, bytes).await
}
