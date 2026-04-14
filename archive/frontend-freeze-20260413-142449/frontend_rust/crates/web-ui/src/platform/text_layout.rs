use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyProfile {
    pub font_css: String,
    pub line_height_px: f64,
    pub horizontal_padding_px: f64,
    pub vertical_padding_px: f64,
    pub block_gap_px: f64,
    pub reserved_width_px: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextHeightPrediction {
    pub text_height_px: f64,
    pub line_count: usize,
}

pub fn width_bucket(width_px: f64) -> u32 {
    ((width_px.max(32.0) / 32.0).floor() as u32) * 32
}

pub fn estimate_shell_height(
    text_height_px: f64,
    profile: &TypographyProfile,
    block_count: usize,
) -> f64 {
    let gaps = if block_count > 0 {
        (block_count.saturating_sub(1)) as f64 * profile.block_gap_px
    } else {
        0.0
    };
    text_height_px + profile.vertical_padding_px * 2.0 + gaps
}

#[cfg(target_arch = "wasm32")]
pub async fn predict_text_height(
    text: &str,
    profile: &TypographyProfile,
    _locale: &str,
    available_width_px: f64,
) -> anyhow::Result<TextHeightPrediction> {
    let usable_width_px =
        (available_width_px - profile.horizontal_padding_px - profile.reserved_width_px).max(32.0);
    let line_count = fallback_line_count(text, usable_width_px);
    Ok(TextHeightPrediction {
        text_height_px: line_count as f64 * profile.line_height_px,
        line_count,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn predict_text_height(
    text: &str,
    profile: &TypographyProfile,
    _locale: &str,
    available_width_px: f64,
) -> anyhow::Result<TextHeightPrediction> {
    let usable_width_px =
        (available_width_px - profile.horizontal_padding_px - profile.reserved_width_px).max(32.0);
    let line_count = fallback_line_count(text, usable_width_px);
    Ok(TextHeightPrediction {
        text_height_px: line_count as f64 * profile.line_height_px,
        line_count,
    })
}

fn fallback_line_count(text: &str, usable_width_px: f64) -> usize {
    if text.is_empty() {
        return 0;
    }

    let chars_per_line = (usable_width_px / 8.0).floor().max(1.0) as usize;
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                1
            } else {
                (line.chars().count().max(1) + chars_per_line - 1) / chars_per_line
            }
        })
        .sum()
}
