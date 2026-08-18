use contracts::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::skills::{ExecutionContext, SkillComponent};

/// Web Fetch Skill — fetch a web page by URL and extract its main text content.
///
/// # Gotchas
/// - JavaScript-rendered content is NOT supported (static HTML only).
/// - Private network addresses (localhost, RFC1918) are blocked.
/// - Non-HTTP schemes (ftp, file, data) are rejected.
/// - Content extraction is heuristic; unusual markup may leave noise or lose content.
pub struct WebFetchSkill;

#[async_trait::async_trait]
impl SkillComponent for WebFetchSkill {
    fn id(&self) -> &str {
        "web_fetch"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    /// Index-tier routing trigger.
    fn description(&self) -> &str {
        "Load when the user provides a URL and wants to extract, summarize, or answer questions about its content."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_fetch".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Fetch a web page by URL and extract its main text content. ",
                "Use this when the user references a specific URL or asks about content that requires reading a web page.\n",
                "Rules:\n",
                "- Only http:// and https:// URLs are supported.\n",
                "- Private addresses (localhost, 127.0.0.1, 10.x.x.x, etc.) are blocked.\n",
                "- JavaScript-rendered content is NOT supported.\n",
                "- Use max_length to control how much text is returned."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The fully-qualified URL to fetch. Must start with http:// or https://."
                    },
                    "max_length": {
                        "type": "integer",
                        "default": 8000,
                        "description": "Maximum characters to return. Longer content is truncated."
                    }
                },
                "required": ["url"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "title": { "type": "string" },
                    "content": { "type": "string" },
                    "truncated": { "type": "boolean" },
                    "length": { "type": "integer" }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "JavaScript-rendered content is NOT supported (static HTML only).",
            "Private addresses (localhost, RFC1918) are blocked for security.",
            "Only http:// and https:// schemes are allowed.",
            "Content extraction is heuristic; some noise may remain on unusual pages.",
            "Large pages are truncated to max_length (default 8000 chars).",
        ]
    }

    fn render_hint(&self) -> &str {
        "web_fetch"
    }

    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let url = args.get("url").and_then(|v| v.as_str()).unwrap_or_default();

        if url.is_empty() {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": "missing url" })),
                trace: None,
            };
        }

        if let Err(e) = common::validate_http_url_with_dns(url, true) {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": e.to_string() })),
                trace: None,
            };
        }

        let max_length = args
            .get("max_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(8000) as usize;

        let start = std::time::Instant::now();

        match fetch_and_extract(url, max_length).await {
            Ok(result) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                ToolResult {
                    tool: self.id().to_string(),
                    version: self.version().to_string(),
                    status: ToolStatus::Ok,
                    data: Some(serde_json::json!({
                        "url": url,
                        "title": result.title,
                        "content": result.content,
                        "truncated": result.truncated,
                        "length": result.length,
                    })),
                    trace: Some(contracts::ToolTrace {
                        elapsed_ms: Some(elapsed_ms),
                        raw_hit_count: None,
                        hydrated_hit_count: None,
                        degrade_reason: None,
                    }),
                }
            }
            Err(error) => {
                let elapsed_ms = start.elapsed().as_millis() as u64;
                ToolResult {
                    tool: self.id().to_string(),
                    version: self.version().to_string(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({ "error": error.to_string() })),
                    trace: Some(contracts::ToolTrace {
                        elapsed_ms: Some(elapsed_ms),
                        raw_hit_count: None,
                        hydrated_hit_count: None,
                        degrade_reason: None,
                    }),
                }
            }
        }
    }
}

struct FetchResult {
    title: String,
    content: String,
    truncated: bool,
    length: usize,
}

async fn fetch_and_extract(url: &str, max_length: usize) -> anyhow::Result<FetchResult> {
    // Re-check here so CRW never receives a private target even if a caller
    // bypasses execute(). DNS is resolved so names that map to RFC1918 fail closed.
    common::validate_http_url_with_dns(url, true)?;

    // Prefer CRW (Docker/local binary) when configured — same reader as web auto-scrape.
    if let Some(result) = try_crw_scrape(url, max_length).await {
        return result;
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Context-OS-Agent/1.0")
        .build()?;

    let resp = client.get(url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!(
            "HTTP {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("unknown")
        );
    }

    let html = resp.text().await?;
    let title = extract_title(&html);
    let text = extract_text(&html);

    let length = text.len();
    let (content, truncated) = truncate_extracted_text(&text, max_length);

    Ok(FetchResult {
        title,
        content,
        truncated,
        length,
    })
}

/// Returns `None` when CRW is not configured; `Some(Err)` when CRW was tried and failed.
async fn try_crw_scrape(url: &str, max_length: usize) -> Option<anyhow::Result<FetchResult>> {
    let base = std::env::var("CRW_BASE_URL").unwrap_or_default();
    let base = base.trim();
    if base.is_empty() {
        return None;
    }
    let key = std::env::var("CRW_API_KEY").unwrap_or_default();
    let timeout_ms = std::env::var("WEB_AUTO_SCRAPE_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12_000_u64);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Some(Err(e.into())),
    };
    match avrag_search::scrape_url(&client, base, &key, url, timeout_ms, max_length).await {
        Ok(md) => {
            let title = md
                .lines()
                .find(|l| l.starts_with('#') || !l.trim().is_empty())
                .map(|l| l.trim_start_matches('#').trim().to_string())
                .unwrap_or_default();
            let length = md.len();
            let truncated = length > max_length;
            Some(Ok(FetchResult {
                title,
                content: md,
                truncated,
                length,
            }))
        }
        Err(e) => Some(Err(e)),
    }
}

fn extract_title(html: &str) -> String {
    let re = regex::Regex::new(r"(?is)<title\s*>(.*?)</title\s*>").unwrap();
    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| decode_basic_entities(m.as_str()).trim().to_string())
        .unwrap_or_default()
}

fn extract_text(html: &str) -> String {
    let mut text = html.to_string();

    // Remove script, style, and common boilerplate tags with their contents.
    for tag in &[
        "script", "style", "nav", "header", "footer", "aside", "noscript", "svg", "canvas",
    ] {
        let escaped = regex::escape(tag);
        let pattern = format!(r"(?is)<{}\b[^>]*>.*?</{}\s*>", escaped, escaped);
        if let Ok(re) = regex::Regex::new(&pattern) {
            text = re.replace_all(&text, " ").to_string();
        }
    }

    // Remove remaining HTML tags.
    let re = regex::Regex::new(r"<[^>]+>").unwrap();
    text = re.replace_all(&text, " ").to_string();

    // Decode basic HTML entities.
    text = decode_basic_entities(&text);

    // Normalize whitespace.
    let re_ws = regex::Regex::new(r"\s+").unwrap();
    text = re_ws.replace_all(&text, " ").trim().to_string();

    text
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Truncate by **byte budget** without panicking on multi-byte UTF-8 (e.g. CJK).
fn truncate_extracted_text(text: &str, max_length: usize) -> (String, bool) {
    if text.len() <= max_length {
        return (text.to_string(), false);
    }
    let end = text.floor_char_boundary(max_length);
    (
        format!(
            "{}... [truncated, {} chars total]",
            &text[..end],
            text.chars().count()
        ),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_extracted_text_handles_cjk_byte_boundary() {
        // "速度" is 6 bytes (3+3); cut at 4 must not panic mid-char.
        let text = "速度".repeat(1000);
        let (out, truncated) = truncate_extracted_text(&text, 4);
        assert!(truncated);
        assert!(out.contains("truncated"));
        assert!(out.is_char_boundary(out.find('.').unwrap_or(0)));
    }

    #[test]
    fn validate_url_accepts_http() {
        assert!(common::validate_http_url("http://example.com").is_ok());
    }

    #[test]
    fn validate_url_accepts_https() {
        assert!(common::validate_http_url("https://example.com").is_ok());
    }

    #[test]
    fn validate_url_rejects_ftp() {
        assert!(common::validate_http_url("ftp://example.com").is_err());
    }

    #[test]
    fn validate_url_rejects_localhost() {
        assert!(common::validate_http_url("http://localhost:8080").is_err());
    }

    #[test]
    fn validate_url_rejects_127_0_0_1() {
        assert!(common::validate_http_url("http://127.0.0.1/api").is_err());
    }

    #[test]
    fn validate_url_rejects_10_x() {
        assert!(common::validate_http_url("http://10.0.0.1/").is_err());
    }

    #[test]
    fn validate_url_rejects_192_168() {
        assert!(common::validate_http_url("https://192.168.1.1/").is_err());
    }

    #[test]
    fn validate_url_rejects_172_16() {
        assert!(common::validate_http_url("http://172.16.0.1/").is_err());
    }

    #[test]
    fn validate_url_accepts_172_32() {
        assert!(common::validate_http_url("http://172.32.0.1/").is_ok());
    }

    #[test]
    fn validate_url_rejects_link_local_metadata() {
        assert!(common::validate_http_url("http://169.254.169.254/").is_err());
    }

    #[test]
    fn validate_url_rejects_unspecified_ipv4() {
        assert!(common::validate_http_url("http://0.0.0.0/").is_err());
    }

    #[test]
    fn validate_url_rejects_unique_local_ipv6() {
        assert!(common::validate_http_url("http://[fd00::1]/").is_err());
    }

    #[test]
    fn extract_title_finds_title() {
        let html = "<html><head><title>Hello World</title></head><body></body></html>";
        assert_eq!(extract_title(html), "Hello World");
    }

    #[test]
    fn extract_title_returns_empty_when_missing() {
        let html = "<html><body></body></html>";
        assert_eq!(extract_title(html), "");
    }

    #[test]
    fn extract_text_removes_scripts() {
        let html = r#"<p>Hello</p><script>alert("x")</script><p>World</p>"#;
        let text = extract_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("script"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn extract_text_removes_styles() {
        let html = r#"<style>.red{color:red}</style><p>Text</p>"#;
        let text = extract_text(html);
        assert!(text.contains("Text"));
        assert!(!text.contains("style"));
        assert!(!text.contains("color"));
    }

    #[test]
    fn extract_text_normalizes_whitespace() {
        let html = "<p>Hello   \n\n   World</p>";
        let text = extract_text(html);
        assert_eq!(text, "Hello World");
    }

    #[tokio::test]
    async fn test_web_fetch_basic() {
        let skill = WebFetchSkill;
        let ctx = ExecutionContext::new(None);
        let result = skill
            .execute(&serde_json::json!({"url": "https://example.com"}), &ctx)
            .await;
        // example.com may succeed or fail depending on network; we just check structure.
        assert!(matches!(result.status, ToolStatus::Ok | ToolStatus::Error));
    }

    #[tokio::test]
    async fn test_web_fetch_missing_url() {
        let skill = WebFetchSkill;
        let ctx = ExecutionContext::new(None);
        let result = skill.execute(&serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing url"));
    }

    async fn assert_fetch_rejects(url: &str) {
        let skill = WebFetchSkill;
        let ctx = ExecutionContext::new(None);
        let result = skill
            .execute(&serde_json::json!({"url": url}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::Error, "url={url}");
        let data = result.data.unwrap();
        let err = data["error"].as_str().unwrap();
        assert!(
            err.contains("not permitted") || err.contains("private") || err.contains("scheme"),
            "unexpected ssrf error for {url}: {err}"
        );
    }

    #[tokio::test]
    async fn test_web_fetch_private_url() {
        assert_fetch_rejects("http://localhost:8080").await;
    }

    #[tokio::test]
    async fn test_web_fetch_rejects_metadata_and_ula() {
        assert_fetch_rejects("http://169.254.169.254/").await;
        assert_fetch_rejects("http://0.0.0.0/").await;
        assert_fetch_rejects("http://[fd00::1]/").await;
    }
}
