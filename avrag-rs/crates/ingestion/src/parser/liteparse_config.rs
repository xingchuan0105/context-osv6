use std::env;

use serde::{Deserialize, Serialize};

/// Runtime configuration for LiteParse ingestion (§12.1 + §5.3 of architecture doc).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiteParseConfig {
    pub ocr_enabled: bool,
    pub ocr_server_url: Option<String>,
    pub ocr_language: String,
    pub scanned_page_threshold: usize,
    pub table_garble_threshold: f32,
    pub table_heavy_threshold: usize,
    pub fig_ratio_threshold: f32,
    pub fig_count_threshold: usize,
    pub text_qual_threshold: f32,
    pub decorative_max_area: f32,
}

impl Default for LiteParseConfig {
    fn default() -> Self {
        Self {
            ocr_enabled: false,
            ocr_server_url: None,
            ocr_language: "zh".to_string(),
            scanned_page_threshold: 100,
            table_garble_threshold: 0.30,
            table_heavy_threshold: 10,
            fig_ratio_threshold: 0.15,
            fig_count_threshold: 2,
            text_qual_threshold: 0.5,
            decorative_max_area: 0.03,
        }
    }
}

impl LiteParseConfig {
    pub fn from_env() -> Self {
        Self {
            ocr_enabled: env_flag("LITEPARSE_OCR_ENABLED"),
            ocr_server_url: env::var("LITEPARSE_OCR_SERVER_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            ocr_language: env::var("LITEPARSE_OCR_LANGUAGE").unwrap_or_else(|_| "zh".to_string()),
            scanned_page_threshold: env_usize("LITEPARSE_SCANNED_PAGE_THRESHOLD", 100),
            table_garble_threshold: env_f32("LITEPARSE_TABLE_GARBLE_THRESHOLD", 0.30),
            table_heavy_threshold: env_usize("LITEPARSE_TABLE_HEAVY_THRESHOLD", 10),
            fig_ratio_threshold: env_f32("LITEPARSE_FIG_RATIO_THRESHOLD", 0.15),
            fig_count_threshold: env_usize("LITEPARSE_FIG_COUNT_THRESHOLD", 2),
            text_qual_threshold: env_f32("LITEPARSE_TEXT_QUAL_THRESHOLD", 0.5),
            decorative_max_area: env_f32("LITEPARSE_DECORATIVE_MAX_AREA", 0.03),
        }
    }
}

fn env_flag(key: &str) -> bool {
    env::var(key)
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f32(key: &str, default: f32) -> f32 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_sane_thresholds() {
        let cfg = LiteParseConfig::default();
        assert_eq!(cfg.scanned_page_threshold, 100);
        assert!(!cfg.ocr_enabled);
    }
}
