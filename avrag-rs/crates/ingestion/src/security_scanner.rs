//! Malware and ZIP-bomb detection for uploaded documents.
//!
//! # ClamAV integration (optional)
//!
//! Set `CLAMAV_HOST` (and optional `CLAMAV_PORT`, default `3310`) to enable.
//! Unset host → skip the malware scan (local/dev without clamd).
//! Host set but daemon unreachable or response unexpected → **error**
//! (callers fail-closed unless they opt into `SECURITY_SCAN_FAIL_OPEN`).
//!
//! # ZIP-bomb detection
//!
//! For ZIP archives we compute the **aggregate compression ratio**
//! (`sum(uncompressed_size) / sum(compressed_size)`).  If the ratio exceeds
//! `ZIP_BOMB_MAX_RATIO` (default: 100) the file is rejected.

use std::io::{Read, Write};
use tracing::{info, warn};

/// Default ClamAV TCP port.
const CLAMAV_DEFAULT_PORT: u16 = 3310;

/// Default maximum compression ratio before a ZIP is considered a bomb.
const ZIP_BOMB_MAX_RATIO: f64 = 100.0;

/// Default maximum uncompressed size for a single ZIP entry (1 GB).
const ZIP_BOMB_MAX_ENTRY_BYTES: u64 = 1024 * 1024 * 1024;

/// Scan result.
#[derive(Debug, Clone, PartialEq)]
pub enum ScanResult {
    Clean,
    ThreatDetected { threat_name: String },
    ZipBomb { ratio: f64 },
}

/// Scan `data` for known malware (via ClamAV) and ZIP bombs.
///
/// ZIP-bomb detection always runs. ClamAV runs only when `CLAMAV_HOST` is set;
/// a configured but unreachable daemon is an error, not a clean bill.
pub async fn scan_upload(data: &[u8], filename: &str) -> anyhow::Result<ScanResult> {
    // 1. ZIP-bomb check (local, always runs).
    if looks_like_zip(data) || filename.to_ascii_lowercase().ends_with(".zip") {
        if let Some(ratio) = zip_compression_ratio(data)? {
            if ratio > ZIP_BOMB_MAX_RATIO {
                info!(
                    filename = %filename,
                    ratio = %ratio,
                    "ZIP bomb detected: compression ratio exceeds threshold"
                );
                return Ok(ScanResult::ZipBomb { ratio });
            }
        }
    }

    let Some(addr) = clamav_endpoint() else {
        return Ok(ScanResult::Clean);
    };

    match clamav_scan(&addr, data).await? {
        ScanResult::ThreatDetected { threat_name } => {
            info!(filename = %filename, threat = %threat_name, "ClamAV detected malware");
            Ok(ScanResult::ThreatDetected { threat_name })
        }
        other => Ok(other),
    }
}

/// `None` when ClamAV is not configured (do not default to localhost).
fn clamav_endpoint() -> Option<String> {
    clamav_endpoint_from(
        std::env::var("CLAMAV_HOST").ok().as_deref(),
        std::env::var("CLAMAV_PORT").ok().as_deref(),
    )
}

fn clamav_endpoint_from(host: Option<&str>, port: Option<&str>) -> Option<String> {
    let host = host?.trim();
    if host.is_empty() {
        return None;
    }
    let port = port
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(CLAMAV_DEFAULT_PORT);
    Some(format!("{host}:{port}"))
}

fn looks_like_zip(data: &[u8]) -> bool {
    data.starts_with(b"PK\x03\x04")
        || data.starts_with(b"PK\x05\x06")
        || data.starts_with(b"PK\x07\x08")
}

fn zip_compression_ratio(data: &[u8]) -> anyhow::Result<Option<f64>> {
    let reader = std::io::Cursor::new(data);
    let mut archive = match zip::ZipArchive::new(reader) {
        Ok(a) => a,
        Err(error) => {
            // Not a valid ZIP — skip the check.
            warn!(error = %error, "failed to parse as ZIP, skipping ZIP-bomb check");
            return Ok(None);
        }
    };

    let mut total_compressed: u64 = 0;
    let mut total_uncompressed: u64 = 0;

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let compressed = file.compressed_size();
        let uncompressed = file.size();

        // Reject individual entries that claim enormous uncompressed sizes.
        if uncompressed > ZIP_BOMB_MAX_ENTRY_BYTES {
            return Ok(Some(f64::MAX));
        }

        total_compressed = total_compressed.saturating_add(compressed);
        total_uncompressed = total_uncompressed.saturating_add(uncompressed);
    }

    if total_compressed == 0 {
        return Ok(None);
    }

    let ratio = total_uncompressed as f64 / total_compressed as f64;
    Ok(Some(ratio))
}

async fn clamav_scan(addr: &str, data: &[u8]) -> anyhow::Result<ScanResult> {
    let addr = addr.to_string();
    let data = data.to_vec();
    let response = tokio::task::spawn_blocking(move || clamav_scan_sync(&addr, &data))
        .await
        .map_err(|e| anyhow::anyhow!("clamav blocking task panicked: {e}"))??;

    parse_clamav_response(&response)
}

fn clamav_scan_sync(addr: &str, data: &[u8]) -> anyhow::Result<String> {
    let mut stream = std::net::TcpStream::connect(addr)
        .map_err(|e| anyhow::anyhow!("cannot connect to clamd at {addr}: {e}"))?;

    stream
        .write_all(b"zINSTREAM\n")
        .map_err(|e| anyhow::anyhow!("failed to write clamd command: {e}"))?;

    const CHUNK_SIZE: usize = 8192;
    for chunk in data.chunks(CHUNK_SIZE) {
        let size = chunk.len() as u32;
        stream
            .write_all(&size.to_be_bytes())
            .map_err(|e| anyhow::anyhow!("failed to write chunk size: {e}"))?;
        stream
            .write_all(chunk)
            .map_err(|e| anyhow::anyhow!("failed to write chunk data: {e}"))?;
    }

    // Terminate stream with zero-length chunk.
    stream
        .write_all(&[0u8; 4])
        .map_err(|e| anyhow::anyhow!("failed to write terminator: {e}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| anyhow::anyhow!("failed to read clamd response: {e}"))?;

    Ok(response)
}

fn parse_clamav_response(response: &str) -> anyhow::Result<ScanResult> {
    let trimmed = response.trim();
    if trimmed.ends_with("OK") {
        return Ok(ScanResult::Clean);
    }
    // Response format: stream: <virus-name> FOUND
    if let Some(pos) = trimmed.find("FOUND") {
        let prefix = &trimmed[..pos];
        let threat = prefix
            .split(':')
            .nth(1)
            .unwrap_or(prefix)
            .trim()
            .to_string();
        return Ok(ScanResult::ThreatDetected {
            threat_name: threat,
        });
    }
    if trimmed.contains("ERROR") {
        return Err(anyhow::anyhow!("clamd returned error: {trimmed}"));
    }
    Err(anyhow::anyhow!("unexpected clamd response: {trimmed}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_zip_detects_magic_bytes() {
        assert!(looks_like_zip(b"PK\x03\x04"));
        assert!(looks_like_zip(b"PK\x05\x06"));
        assert!(!looks_like_zip(b"hello world"));
    }

    #[test]
    fn parse_clamav_response_clean() {
        assert_eq!(
            parse_clamav_response("stream: OK").unwrap(),
            ScanResult::Clean
        );
        assert_eq!(
            parse_clamav_response("stream: OK\n").unwrap(),
            ScanResult::Clean
        );
    }

    #[test]
    fn parse_clamav_response_found() {
        let result = parse_clamav_response("stream: Eicar-Test-Signature FOUND").unwrap();
        assert_eq!(
            result,
            ScanResult::ThreatDetected {
                threat_name: "Eicar-Test-Signature".to_string()
            }
        );
    }

    #[test]
    fn parse_clamav_response_error() {
        assert!(parse_clamav_response("stream: ERROR").is_err());
    }

    #[test]
    fn clamav_endpoint_is_opt_in() {
        assert_eq!(clamav_endpoint_from(None, None), None);
        assert_eq!(clamav_endpoint_from(Some(""), None), None);
        assert_eq!(clamav_endpoint_from(Some("  "), Some("3310")), None);
        assert_eq!(
            clamav_endpoint_from(Some("clamd.internal"), None).as_deref(),
            Some("clamd.internal:3310")
        );
        assert_eq!(
            clamav_endpoint_from(Some("clamd.internal"), Some("3311")).as_deref(),
            Some("clamd.internal:3311")
        );
    }
}
