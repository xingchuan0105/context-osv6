use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn precompress_site(site_root: &Path) {
    for dir in ["pkg", "style"] {
        precompress_dir(&site_root.join(dir));
    }
}

fn precompress_dir(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            precompress_dir(&path);
            continue;
        }
        if !should_precompress(&path) {
            continue;
        }
        write_gzip(&path);
        write_brotli(&path);
    }
}

fn should_precompress(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".gz") || name.ends_with(".br") || name == "hash.txt" {
        return false;
    }
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("js" | "css" | "wasm" | "html" | "svg" | "json" | "mjs")
    )
}

fn write_gzip(path: &Path) {
    let dest = sidecar(path, "gz");
    if dest.exists() {
        return;
    }
    let Ok(data) = fs::read(path) else {
        return;
    };
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    if encoder.write_all(&data).is_err() {
        return;
    }
    if let Ok(buf) = encoder.finish() {
        let _ = fs::write(dest, buf);
    }
}

fn write_brotli(path: &Path) {
    let dest = sidecar(path, "br");
    if dest.exists() {
        return;
    }
    let Ok(data) = fs::read(path) else {
        return;
    };
    let mut out = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
        if encoder.write_all(&data).is_err() {
            return;
        }
        if encoder.flush().is_err() {
            return;
        }
    }
    let _ = fs::write(dest, out);
}

fn sidecar(path: &Path, ext: &str) -> PathBuf {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    path.with_file_name(format!("{name}.{ext}"))
}

#[cfg(test)]
mod tests {
    use super::{should_precompress, sidecar};
    use std::path::Path;

    #[test]
    fn wasm_and_js_are_precompressed_sidecars() {
        assert!(should_precompress(Path::new("pkg/web_ui.js")));
        assert!(should_precompress(Path::new("pkg/web_ui.wasm")));
        assert!(!should_precompress(Path::new("pkg/web_ui.js.gz")));
        assert!(!should_precompress(Path::new("pkg/hash.txt")));
        assert_eq!(
            sidecar(Path::new("pkg/web_ui.wasm"), "br"),
            Path::new("pkg/web_ui.wasm.br")
        );
    }
}
