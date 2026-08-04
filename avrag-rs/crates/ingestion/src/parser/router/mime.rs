pub(super) fn normalize_extension(filename: &str) -> Option<String> {
    let (_, extension) = filename.rsplit_once('.')?;
    let normalized = extension.trim().to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

pub(super) fn normalize_mime_type(mime_type: &str) -> String {
    mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

pub(super) fn is_supported_extension(extension: &str) -> bool {
    matches!(
        extension,
        // text/code long-tail (markitdown)
        "txt"
            | "md"
            | "rst"
            | "tsv"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "html"
            | "htm"
            // PDF / images
            | "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "gif"
            | "bmp"
            // anydoc — Word
            | "doc"
            | "docx"
            | "docm"
            // anydoc — Excel
            | "xls"
            | "xlsx"
            | "xlsm"
            | "xlsb"
            // anydoc — PowerPoint
            | "ppt"
            | "pps"
            | "pot"
            | "pptx"
            | "pptm"
            | "ppsx"
            | "ppsm"
            // anydoc — ODF / RTF / EPUB / CSV
            | "odt"
            | "ods"
            | "odp"
            | "rtf"
            | "epub"
            | "csv"
    ) || is_code_extension(extension)
}

pub(super) fn is_code_extension(extension: &str) -> bool {
    matches!(
        extension,
        "rs" | "py"
            | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "kt"
            | "scala"
            | "r"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "ps1"
            | "sql"
            | "xml"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "vue"
            | "svelte"
            | "graphql"
            | "proto"
            | "gradle"
            | "cmake"
            | "makefile"
            | "dockerfile"
            | "tf"
            | "hcl"
    )
}

pub(super) fn mime_matches_extension(extension: &str, mime_type: &str) -> bool {
    match extension {
        "pdf" => mime_type == "application/pdf",
        "png" => mime_type == "image/png",
        "jpg" | "jpeg" => mime_type == "image/jpeg",
        "webp" => mime_type == "image/webp",
        "gif" => mime_type == "image/gif",
        "bmp" => mime_type == "image/bmp",
        "doc" => mime_type == "application/msword",
        "docx" => {
            mime_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        "docm" => {
            mime_type == "application/vnd.ms-word.document.macroenabled.12"
                || mime_type
                    == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        "xls" => mime_type == "application/vnd.ms-excel",
        "xlsx" => mime_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsm" => {
            mime_type == "application/vnd.ms-excel.sheet.macroenabled.12"
                || mime_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        "xlsb" => mime_type == "application/vnd.ms-excel.sheet.binary.macroenabled.12",
        "ppt" | "pps" | "pot" => mime_type == "application/vnd.ms-powerpoint",
        "pptx" => {
            mime_type == "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        "pptm" => {
            mime_type == "application/vnd.ms-powerpoint.presentation.macroenabled.12"
                || mime_type
                    == "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        "ppsx" => {
            mime_type == "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
        }
        "ppsm" => {
            mime_type == "application/vnd.ms-powerpoint.slideshow.macroenabled.12"
                || mime_type
                    == "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
        }
        "odt" => {
            mime_type == "application/vnd.oasis.opendocument.text"
                || mime_type == "application/x-vnd.oasis.opendocument.text"
        }
        "ods" => {
            mime_type == "application/vnd.oasis.opendocument.spreadsheet"
                || mime_type == "application/x-vnd.oasis.opendocument.spreadsheet"
        }
        "odp" => {
            mime_type == "application/vnd.oasis.opendocument.presentation"
                || mime_type == "application/x-vnd.oasis.opendocument.presentation"
        }
        "rtf" => mime_type == "application/rtf" || mime_type == "text/rtf",
        "epub" => mime_type == "application/epub+zip",
        "html" | "htm" => {
            mime_type == "application/xhtml+xml"
                || mime_type == "application/xml"
                || is_generic_text_mime(mime_type)
        }
        "json" => {
            mime_type == "application/json"
                || mime_type == "text/json"
                || mime_type == "application/ld+json"
                || mime_type == "text/plain"
        }
        "csv" => {
            mime_type == "text/csv" || mime_type == "application/csv" || mime_type == "text/plain"
        }
        "tsv" => mime_type == "text/tab-separated-values" || mime_type == "text/plain",
        "md" => mime_type == "text/markdown" || mime_type == "text/plain",
        "toml" => mime_type == "application/toml" || mime_type == "text/plain",
        "yaml" | "yml" => matches!(
            mime_type,
            "application/yaml" | "application/x-yaml" | "text/yaml" | "text/x-yaml" | "text/plain"
        ),
        "txt" | "rst" => mime_type == "text/plain",
        _ if is_code_extension(extension) => {
            is_generic_text_mime(mime_type)
                || matches!(
                    mime_type,
                    "application/javascript"
                        | "application/typescript"
                        | "application/xml"
                        | "application/sql"
                )
        }
        _ => false,
    }
}

fn is_generic_text_mime(mime_type: &str) -> bool {
    mime_type.starts_with("text/")
}
