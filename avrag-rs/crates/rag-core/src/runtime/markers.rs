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

/// `[block n]` code-execution observation line (error path) — single producer
/// for the `Execution failed:` failure form parsed by [`parse_block`] (P3-1;
/// the failure form used to be a handwritten `format!` in iteration_codegen).
pub fn format_block_failure(idx: usize, error: &str) -> String {
    format!("[block {idx}] Execution failed: {error}")
}

/// A parsed `[block n]` code-execution observation segment (the text after the
/// opening `[block ` marker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    pub idx: usize,
    /// `stdout:` payload, trimmed.
    pub stdout: Option<&'a str>,
    /// `stderr:` payload (success form; `None` when the line is truncated).
    pub stderr: Option<&'a str>,
    /// `Execution failed:` payload (error form; `None` on success).
    pub failure: Option<&'a str>,
}

/// Parse one `[block n]` observation segment: `{n}] stdout: <…>\nstderr: <…>`
/// (success) or `{n}] Execution failed: <…>` (error). Returns `None` for
/// non-block segments (no numeric index, or neither `stdout:` nor
/// `Execution failed:` present). This is the single parse-side implementation
/// behind the former hand-written scanner in exit_policy (P1-3).
pub fn parse_block(segment: &str) -> Option<Block<'_>> {
    let (idx_part, body) = segment.split_once(']')?;
    let idx = idx_part.trim().parse::<usize>().ok()?;
    let body = body.trim_start();
    if let Some(failure) = body.strip_prefix("Execution failed:") {
        return Some(Block {
            idx,
            stdout: None,
            stderr: None,
            failure: Some(failure.trim()),
        });
    }
    let stdout = body.strip_prefix("stdout:")?;
    let (stdout, stderr) = match stdout.split_once("stderr:") {
        Some((stdout, stderr)) => (stdout.trim(), Some(stderr.trim())),
        None => (stdout.trim(), None),
    };
    Some(Block {
        idx,
        stdout: Some(stdout),
        stderr,
        failure: None,
    })
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

    #[test]
    fn parse_block_roundtrips_format_block() {
        let line = format_block(3, "chunk-a chunk-b text", "traceback");
        let parsed = parse_block(&line["[block ".len()..]).expect("parse success block");
        assert_eq!(parsed.idx, 3);
        assert_eq!(parsed.stdout, Some("chunk-a chunk-b text"));
        assert_eq!(parsed.stderr, Some("traceback"));
        assert_eq!(parsed.failure, None);
    }

    #[test]
    fn parse_block_reads_error_form() {
        let parsed = parse_block("1] Execution failed: NameError: x").expect("parse error block");
        assert_eq!(parsed.idx, 1);
        assert_eq!(parsed.failure, Some("NameError: x"));
        assert_eq!(parsed.stdout, None);
    }

    #[test]
    fn format_block_failure_matches_producer_contract() {
        assert_eq!(
            format_block_failure(2, "interpreter task panicked: boom"),
            "[block 2] Execution failed: interpreter task panicked: boom"
        );
    }

    #[test]
    fn parse_block_roundtrips_format_block_failure() {
        let line = format_block_failure(0, "NameError: name 'x' is not defined");
        let parsed = parse_block(&line["[block ".len()..]).expect("parse failure block");
        assert_eq!(parsed.idx, 0);
        assert_eq!(parsed.failure, Some("NameError: name 'x' is not defined"));
        assert_eq!(parsed.stdout, None);
        assert_eq!(parsed.stderr, None);
    }

    #[test]
    fn parse_block_error_form_wins_over_embedded_stdout_marker() {
        // The failure form is matched before `stdout:`; an error message that
        // merely mentions stdout stays a failure payload.
        let parsed = parse_block("3] Execution failed: write to stdout: broken pipe")
            .expect("failure form takes precedence");
        assert_eq!(parsed.failure, Some("write to stdout: broken pipe"));
        assert_eq!(parsed.stdout, None);
    }

    #[test]
    fn parse_block_error_form_truncates_to_empty_payload() {
        // A truncated failure line still parses as the error form (empty payload).
        let parsed = parse_block("1] Execution failed:").expect("truncated failure");
        assert_eq!(parsed.idx, 1);
        assert_eq!(parsed.failure, Some(""));
    }

    #[test]
    fn parse_block_skips_non_block_segments() {
        // Preamble / non-block text, missing index, or no stdout marker.
        assert_eq!(parse_block("\n<code_execution_result>\n"), None);
        assert_eq!(parse_block("abc] stdout: x"), None);
        assert_eq!(parse_block("0] stderr only"), None);
    }

    #[test]
    fn parse_block_handles_truncated_stdout_without_stderr() {
        let parsed = parse_block("0] stdout: partial").expect("truncated block");
        assert_eq!(parsed.stdout, Some("partial"));
        assert_eq!(parsed.stderr, None);
    }
}
