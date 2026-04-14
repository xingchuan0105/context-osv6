use chrono::Utc;
use uuid::Uuid;

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn estimate_token_count(text: &str) -> i64 {
    (((text.chars().count() as f64) / 4.0).ceil() as i64).max(1)
}

pub fn is_remote_url(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("data:image/")
}

pub fn infer_image_extension(path: &str) -> Option<&'static str> {
    let lower = path.trim().to_ascii_lowercase();
    if lower.ends_with(".png") {
        Some("png")
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("jpg")
    } else if lower.ends_with(".webp") {
        Some("webp")
    } else if lower.ends_with(".gif") {
        Some("gif")
    } else if lower.ends_with(".bmp") {
        Some("bmp")
    } else if lower.ends_with(".svg") {
        Some("svg")
    } else {
        None
    }
}

pub fn infer_mime_type(path: &str) -> Option<&'static str> {
    let extension = path.rsplit('.').next()?.to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        _ => return None,
    };
    Some(mime)
}
