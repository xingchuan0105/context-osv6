//! Playwright screenshot helper — Rust wrapper around Node CLI subprocess.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ViewportConfig {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum AspectRatio {
    Wide16_9,
    Standard4_3,
}

impl AspectRatio {
    pub fn viewport(&self) -> ViewportConfig {
        match self {
            Self::Wide16_9 => ViewportConfig {
                width: 1600,
                height: 900,
                device_scale_factor: 1.5,
            },
            Self::Standard4_3 => ViewportConfig {
                width: 1400,
                height: 1050,
                device_scale_factor: 1.5,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresentationRenderConfig {
    pub aspect_ratio: AspectRatio,
    pub slide_index: usize,
    pub device_scale_factor: f32,
    pub theme_override: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RenderDiagnostics {
    pub console_errors: Vec<String>,
    pub page_errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScreenshotArtifact {
    pub png_bytes: Vec<u8>,
    pub viewport: ViewportConfig,
    pub diagnostics: RenderDiagnostics,
}

#[derive(Debug)]
pub enum PlaywrightError {
    NodeNotAvailable,
    ScreenshotFailed(String),
    Io(std::io::Error),
}

impl From<std::io::Error> for PlaywrightError {
    fn from(e: std::io::Error) -> Self {
        PlaywrightError::Io(e)
    }
}

/// Check if Playwright (node + playwright package) is available.
pub async fn check_playwright_available() -> bool {
    let node_ok = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    if !node_ok {
        return false;
    }

    Command::new("npx")
        .arg("playwright")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

async fn run_screenshot(
    html_content: &str,
    viewport: ViewportConfig,
    clip_viewport: bool,
) -> Result<ScreenshotArtifact, PlaywrightError> {
    // Use temp dir without extra crate dependency
    let temp_name = format!("avrag-e2e-screenshot-{}", uuid::Uuid::new_v4());
    let temp_dir = std::env::temp_dir().join(&temp_name);
    tokio::fs::create_dir_all(&temp_dir).await?;

    let input_path = temp_dir.join("index.html");
    let output_path = temp_dir.join("screenshot.png");
    let diagnostics_path = temp_dir.join("diagnostics.json");

    tokio::fs::write(&input_path, html_content).await?;

    let script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/strategy_e2e/playwright/screenshot.js");

    let viewport_str = format!("{}x{}", viewport.width, viewport.height);

    // Find an available port for the HTTP server
    let port = find_available_port().await.unwrap_or(9876);

    let mut cmd = Command::new("node");
    // Set NODE_PATH so require('playwright') finds globally installed package
    if let Ok(npm_root) = std::process::Command::new("npm")
        .args(["root", "-g"])
        .output()
    {
        let global_modules = String::from_utf8_lossy(&npm_root.stdout).trim().to_string();
        if !global_modules.is_empty() {
            cmd.env("NODE_PATH", global_modules);
        }
    }
    cmd.arg(&script_path)
        .arg(format!("--input={}", input_path.display()))
        .arg(format!("--output={}", output_path.display()))
        .arg(format!("--viewport={}", viewport_str))
        .arg(format!("--diagnostics={}", diagnostics_path.display()))
        .arg(format!("--serve-port={}", port));

    if clip_viewport {
        cmd.arg("--clip-viewport");
    }

    let status = cmd.status().await?;

    let result = if !status.success() {
        let diag_text = tokio::fs::read_to_string(&diagnostics_path).await.unwrap_or_default();
        Err(PlaywrightError::ScreenshotFailed(format!(
            "Node script exited with failure. diagnostics: {}",
            diag_text
        )))
    } else {
        let png_bytes = tokio::fs::read(&output_path).await?;
        let diagnostics: RenderDiagnostics = {
            let text = tokio::fs::read_to_string(&diagnostics_path).await.unwrap_or_default();
            serde_json::from_str(&text).unwrap_or_default()
        };

        Ok(ScreenshotArtifact {
            png_bytes,
            viewport,
            diagnostics,
        })
    };

    // Best-effort cleanup
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    result
}

async fn find_available_port() -> Option<u16> {
    use tokio::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let addr = listener.local_addr().ok()?;
    // Drop listener to free the port
    drop(listener);
    Some(addr.port())
}

pub async fn screenshot_html(
    html_content: &str,
    viewport: ViewportConfig,
) -> Result<ScreenshotArtifact, PlaywrightError> {
    run_screenshot(html_content, viewport, false).await
}

pub async fn screenshot_presentation(
    html_content: &str,
    config: PresentationRenderConfig,
) -> Result<ScreenshotArtifact, PlaywrightError> {
    let viewport = config.aspect_ratio.viewport();
    run_screenshot(html_content, viewport, true).await
}

pub async fn screenshot_webpage(
    html_content: &str,
) -> Result<ScreenshotArtifact, PlaywrightError> {
    let viewport = ViewportConfig {
        width: 1280,
        height: 720,
        device_scale_factor: 1.0,
    };
    run_screenshot(html_content, viewport, false).await
}
