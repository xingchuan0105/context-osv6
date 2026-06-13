use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ir::DocumentIr;

/// Per-page parse outcome written into document IR metadata (`page_status` JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageParseStatus {
    Ok,
    Partial,
    OcrFail,
    Missing,
}

impl PageParseStatus {
    pub fn from_metadata_str(s: &str) -> Self {
        match s {
            "ok" => Self::Ok,
            "partial" => Self::Partial,
            "ocr_fail" => Self::OcrFail,
            "missing" => Self::Missing,
            _ => Self::Missing,
        }
    }

    pub fn as_metadata_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Partial => "partial",
            Self::OcrFail => "ocr_fail",
            Self::Missing => "missing",
        }
    }

    pub fn is_ocr_fail(self) -> bool {
        matches!(self, Self::OcrFail)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageStatusEntry {
    pub page_no: u32,
    pub status: PageParseStatus,
    pub route: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Parse `page_status` metadata JSON from a document IR into a page-number map.
pub fn parse_page_status_from_ir(document_ir: &DocumentIr) -> HashMap<u32, PageParseStatus> {
    let mut map = HashMap::new();
    let Some(raw) = document_ir.metadata.get("page_status") else {
        return map;
    };
    if raw.is_empty() {
        return map;
    }
    if let Ok(entries) = serde_json::from_str::<Vec<PageStatusEntry>>(raw) {
        for entry in entries {
            map.insert(entry.page_no, entry.status);
        }
        return map;
    }
    // Backward compatibility: tolerate loosely-typed JSON arrays.
    if let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(raw) {
        for entry in entries {
            let page_no = entry.get("page_no").and_then(|v| v.as_u64()).map(|n| n as u32);
            let status = entry
                .get("status")
                .and_then(|v| v.as_str())
                .map(PageParseStatus::from_metadata_str);
            if let (Some(p), Some(s)) = (page_no, status) {
                map.insert(p, s);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DocumentIr, DocumentType, ParseBackend};

    #[test]
    fn page_parse_status_roundtrip_serde() {
        let entry = PageStatusEntry {
            page_no: 2,
            status: PageParseStatus::OcrFail,
            route: "C".to_string(),
            duration_ms: Some(120),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("ocr_fail"));
        let parsed: PageStatusEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.status, PageParseStatus::OcrFail);
    }

    #[test]
    fn parse_page_status_from_ir_reads_metadata() {
        let entries = vec![
            PageStatusEntry {
                page_no: 1,
                status: PageParseStatus::Ok,
                route: "A".to_string(),
                duration_ms: None,
            },
            PageStatusEntry {
                page_no: 3,
                status: PageParseStatus::OcrFail,
                route: "C".to_string(),
                duration_ms: Some(50),
            },
        ];
        let mut document = DocumentIr::new(
            "doc-1",
            "title",
            DocumentType::Pdf,
            ParseBackend::EdgeParsePdf, // historical IR wire name
        );
        document.metadata.insert(
            "page_status".to_string(),
            serde_json::to_string(&entries).expect("serialize entries"),
        );
        let map = parse_page_status_from_ir(&document);
        assert_eq!(map.get(&1), Some(&PageParseStatus::Ok));
        assert_eq!(map.get(&3), Some(&PageParseStatus::OcrFail));
    }
}
