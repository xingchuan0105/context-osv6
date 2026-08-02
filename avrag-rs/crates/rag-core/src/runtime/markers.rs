//! Unified citation-marker grammar — the single implementation for the
//! `[[cite:id]]` / `[[image:id]]` / `[[web:n]]` / legacy bare `[[n]]` markers
//! and the `[block n]` code-execution observation line.
//!
//! Previously each crate carried its own hand-written scanner:
//! - `agent-loop/src/cite_extract.rs` + `app-chat/src/prompts/citations.rs`
//!   (byte-identical `extract_referenced_chunk_ids`, cite + image),
//! - `rag-core/src/runtime/response_utils.rs` (third copy),
//! - `agent-loop/src/react_loop/answer_contract.rs` `extract_cite_chunk_ids`
//!   (cite only — the `image:` drift, decision 3) and `extract_web_marker_indices`.
//!
//! All of them delegate here now; behavior is locked by the tests in this module.

/// Marker kind for a `[[…]]` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    /// `[[cite:id]]` — a document-chunk reference.
    Cite,
    /// `[[image:id]]` — an inline-image chunk reference (decision 3: must not
    /// be dropped by the doc-citation extractor).
    Image,
    /// `[[web:n]]` — a web-marker index.
    Web,
    /// Legacy bare `[[n]]` — web-index alias, parsed as `u32` by
    /// [`extract_web_indices`].
    Bare,
}

/// A single parsed `[[…]]` marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub kind: MarkerKind,
    /// Inner token without the `kind:` prefix for Cite/Image/Web; the raw inner
    /// for Bare.
    pub value: String,
}

/// Parse every `[[…]]` marker in `text`, in order. Unclosed brackets, empty
/// tokens and whitespace-only inners are skipped.
pub fn extract_markers(text: &str) -> Vec<Marker> {
    let mut markers = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let token = after[..end].trim();
        if let Some(value) = token.strip_prefix("cite:").map(str::trim) {
            if !value.is_empty() {
                markers.push(Marker {
                    kind: MarkerKind::Cite,
                    value: value.to_string(),
                });
            }
        } else if let Some(value) = token.strip_prefix("image:").map(str::trim) {
            if !value.is_empty() {
                markers.push(Marker {
                    kind: MarkerKind::Image,
                    value: value.to_string(),
                });
            }
        } else if let Some(value) = token.strip_prefix("web:").map(str::trim) {
            if !value.is_empty() {
                markers.push(Marker {
                    kind: MarkerKind::Web,
                    value: value.to_string(),
                });
            }
        } else if !token.is_empty() {
            markers.push(Marker {
                kind: MarkerKind::Bare,
                value: token.to_string(),
            });
        }
        rest = &after[end + 2..];
    }
    markers
}

/// Chunk-reference ids from `[[cite:…]]` and `[[image:…]]`, first-seen order,
/// deduped. This is the single implementation behind the former per-crate
/// `extract_referenced_chunk_ids` / `extract_cite_chunk_ids` copies (decision 3:
/// the image: branch is recognized here, so no extractor drops inline images).
pub fn extract_chunk_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for marker in extract_markers(text) {
        if matches!(marker.kind, MarkerKind::Cite | MarkerKind::Image)
            && !ids.contains(&marker.value)
        {
            ids.push(marker.value);
        }
    }
    ids
}

/// Web marker indices from `[[web:n]]` and legacy bare `[[n]]`, deduped.
/// Replaces `answer_contract::extract_web_marker_indices`.
///
/// Ordering preserves the former two-pass behavior: all `[[web:n]]` indices
/// (in text order) first, then all legacy bare `[[n]]` indices (in text order).
pub fn extract_web_indices(text: &str) -> Vec<u32> {
    let markers = extract_markers(text);
    let mut indices = Vec::new();
    for kind in [MarkerKind::Web, MarkerKind::Bare] {
        for marker in &markers {
            if marker.kind == kind {
                if let Ok(n) = marker.value.parse::<u32>() {
                    if !indices.contains(&n) {
                        indices.push(n);
                    }
                }
            }
        }
    }
    indices
}

/// `[block n]` code-execution observation line (success path) — single producer
/// for the `<code_execution_result>` body used by iteration_codegen.
pub fn format_block(idx: usize, stdout: &str, stderr: &str) -> String {
    format!("[block {}] stdout: {}\nstderr: {}", idx, stdout, stderr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_markers_classifies_all_kinds_in_order() {
        let text = "A [[cite:c1]] then [[image:img1]] then [[web:2]] and [[3]] bare.";
        let markers = extract_markers(text);
        assert_eq!(
            markers,
            vec![
                Marker { kind: MarkerKind::Cite, value: "c1".into() },
                Marker { kind: MarkerKind::Image, value: "img1".into() },
                Marker { kind: MarkerKind::Web, value: "2".into() },
                Marker { kind: MarkerKind::Bare, value: "3".into() },
            ]
        );
    }

    #[test]
    fn extract_markers_trims_whitespace_and_skips_empty() {
        assert_eq!(
            extract_markers("x [[cite: c-1 ]] y [[image: ]] z [[ ]] w"),
            vec![Marker { kind: MarkerKind::Cite, value: "c-1".into() }]
        );
    }

    #[test]
    fn extract_markers_stops_at_unclosed_bracket() {
        assert_eq!(extract_markers("[[cite:unclosed"), Vec::<Marker>::new());
        assert_eq!(extract_markers("ok [[cite:a]] tail [[image:b"), vec![Marker {
            kind: MarkerKind::Cite,
            value: "a".into(),
        }]);
    }

    #[test]
    fn extract_chunk_ids_returns_cite_and_image_deduped_in_order() {
        // image: must NOT be dropped (decision 3 — former answer_contract
        // extractor only read [[cite:).
        assert_eq!(
            extract_chunk_ids("a [[cite:c1]] b [[image:i1]] c [[cite:c1]] d [[image:i1]]"),
            vec!["c1".to_string(), "i1".to_string()]
        );
        // web / bare markers are not chunk ids.
        assert_eq!(extract_chunk_ids("[[web:1]] [[2]]"), Vec::<String>::new());
    }

    #[test]
    fn extract_web_indices_reads_web_and_legacy_bare() {
        assert_eq!(extract_web_indices("[[web:1]] [[2]] [[web:1]] [[3]] [[cite:c]]"), vec![1, 2, 3]);
        assert_eq!(extract_web_indices("no markers"), Vec::<u32>::new());
    }

    #[test]
    fn format_block_matches_producer_contract() {
        assert_eq!(
            format_block(0, "42", "err"),
            "[block 0] stdout: 42\nstderr: err"
        );
    }
}
