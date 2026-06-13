//! Focus Mode — conditional chunk compression for RAG / WebSearch.
//!
//! Focus mode is the **optional** second stage of the Evidence Gate
//! pipeline: when the gate reports [`EvidenceGateOutcome::NeedsFocus`],
//! the caller invokes a `FocusMode` implementation to compress
//! recall before grounded answer.
//!
//! The default [`ScoreBasedFocusMode`] does two things:
//! 1. Keep only the top-N chunks by score.
//! 2. Optionally trim each chunk's text to a character budget
//!    (with sentence-level extraction when a query is provided).
//!
//! Focus mode is **off by default**. The Evidence Gate decides when
//! to invoke it; the strategy layer wires that decision through.

use contracts::AnswerContextChunk;
use std::fmt;

/// Errors emitted by focus-mode implementations.
#[derive(Debug)]
pub enum FocusError {
    /// Internal compression failure (e.g. invalid chunk payload).
    Internal(String),
}

impl fmt::Display for FocusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FocusError::Internal(msg) => write!(f, "focus compression failed: {msg}"),
        }
    }
}

impl std::error::Error for FocusError {}

/// Compressed chunk output by a FocusMode implementation.
#[derive(Debug, Clone)]
pub struct CompressedChunk {
    pub chunk: AnswerContextChunk,
    pub score: f32,
    /// When `true`, the chunk's text was trimmed or sentence-extracted.
    pub trimmed: bool,
}

/// Strategy-agnostic chunk compression trait.
///
/// `compress` receives the raw `(chunk, score)` pairs that the
/// Evidence Gate flagged as `NeedsFocus`. It returns a filtered and
/// possibly trimmed list. The contract is:
/// - Output count <= `target_count`
/// - Output preserves `(chunk, score)` pairing so citations stay
///   consistent downstream.
pub trait FocusMode: Send + Sync {
    fn compress(
        &self,
        items: &[(AnswerContextChunk, f32)],
        query: &str,
        target_count: usize,
    ) -> Result<Vec<CompressedChunk>, FocusError>;
}

/// Default, score-based focus-mode implementation.
#[derive(Debug, Clone)]
pub struct ScoreBasedFocusMode {
    /// Maximum chunks to keep.
    pub keep_top_n: usize,
    /// Per-chunk text character budget after trimming.
    pub trim_to_chars: usize,
    /// When `true`, run a sentence-level extraction that prefers
    /// sentences containing the query's keywords.
    pub extract_relevant_sentences: bool,
}

impl Default for ScoreBasedFocusMode {
    fn default() -> Self {
        Self {
            keep_top_n: 10,
            trim_to_chars: 500,
            extract_relevant_sentences: true,
        }
    }
}

impl FocusMode for ScoreBasedFocusMode {
    fn compress(
        &self,
        items: &[(AnswerContextChunk, f32)],
        query: &str,
        target_count: usize,
    ) -> Result<Vec<CompressedChunk>, FocusError> {
        let target = target_count.min(self.keep_top_n).max(1);

        // 1. Sort by score desc, keep top N.
        let mut sorted: Vec<&(AnswerContextChunk, f32)> = items.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let kept: Vec<&(AnswerContextChunk, f32)> = sorted.into_iter().take(target).collect();

        // 2. Optionally trim each chunk.
        let query_keywords = extract_keywords(query);

        let mut out = Vec::with_capacity(kept.len());
        for (chunk, score) in kept {
            let (new_text, trimmed) = if chunk.text.chars().count() > self.trim_to_chars {
                let trimmed = if self.extract_relevant_sentences && !query_keywords.is_empty() {
                    sentence_extract(&chunk.text, &query_keywords, self.trim_to_chars)
                } else {
                    truncate_chars(&chunk.text, self.trim_to_chars)
                };
                (trimmed, true)
            } else {
                (chunk.text.clone(), false)
            };

            let mut new_chunk = chunk.clone();
            new_chunk.text = new_text;
            out.push(CompressedChunk {
                chunk: new_chunk,
                score: *score,
                trimmed,
            });
        }
        Ok(out)
    }
}

/// Extracts lowercase keyword tokens from a query (length >= 3).
fn extract_keywords(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|s| {
            s.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|s| s.len() >= 3)
        .collect()
}

/// Truncate a string to `max_chars` characters, ending at a word
/// boundary if possible.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    if let Some(last_space) = out.rfind(char::is_whitespace) {
        out.truncate(last_space);
    }
    out.push('…');
    out
}

/// Pick sentences from `text` that contain at least one query
/// keyword, preserving order, until `max_chars` is reached.
fn sentence_extract(text: &str, keywords: &[String], max_chars: usize) -> String {
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }
    let trimmed_tail = current.trim().to_string();
    if !trimmed_tail.is_empty() {
        sentences.push(trimmed_tail);
    }

    let mut buf = String::new();
    for s in sentences {
        let lower = s.to_lowercase();
        if keywords.iter().any(|k| lower.contains(k.as_str())) {
            if buf.len() + s.len() + 1 > max_chars {
                break;
            }
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(&s);
        }
    }
    if buf.is_empty() {
        truncate_chars(text, max_chars)
    } else if buf.chars().count() > max_chars {
        truncate_chars(&buf, max_chars)
    } else {
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(text: &str) -> (AnswerContextChunk, f32) {
        (
            AnswerContextChunk {
                chunk_id: uuid::Uuid::new_v4().to_string(),
                doc_id: Some("doc-1".to_string()),
                chunk_type: "text".to_string(),
                page: None,
                text: text.to_string(),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
            },
            0.0,
        )
    }

    #[test]
    fn keeps_top_n_by_score() {
        let mut items = Vec::new();
        items.push((chunk("a").0, 0.1_f32));
        items.push((chunk("b").0, 0.9));
        items.push((chunk("c").0, 0.5));
        items.push((chunk("d").0, 0.7));
        let focus = ScoreBasedFocusMode {
            keep_top_n: 10,
            trim_to_chars: 1000,
            extract_relevant_sentences: false,
        };
        let out = focus.compress(&items, "irrelevant", 2).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].chunk.text, "b");
        assert_eq!(out[1].chunk.text, "d");
    }

    #[test]
    fn trims_oversized_chunks() {
        let long = "a".repeat(2000);
        let items = vec![(chunk(&long).0, 0.5_f32)];
        let focus = ScoreBasedFocusMode {
            keep_top_n: 10,
            trim_to_chars: 100,
            extract_relevant_sentences: false,
        };
        let out = focus.compress(&items, "x", 5).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].trimmed);
        assert!(out[0].chunk.text.chars().count() <= 101); // 100 + ellipsis
    }

    #[test]
    fn sentence_extract_prefers_relevant() {
        // Pad the text so it exceeds the trim budget.
        let text = "Quantum entanglement is a phenomenon. Bread is tasty. \
                    Einstein studied quantum entanglement closely. Cookies are sweet. \
                    Lorem ipsum dolor sit amet consectetur adipiscing elit \
                    sed do eiusmod tempor incididunt ut labore et dolore magna.";
        let items = vec![(chunk(text).0, 0.5_f32)];
        let focus = ScoreBasedFocusMode {
            keep_top_n: 10,
            trim_to_chars: 120,
            extract_relevant_sentences: true,
        };
        let out = focus.compress(&items, "quantum entanglement", 5).unwrap();
        let combined = &out[0].chunk.text;
        assert!(combined.contains("Quantum entanglement is a phenomenon"));
        assert!(combined.contains("Einstein studied quantum entanglement"));
        assert!(!combined.contains("Bread"));
        assert!(!combined.contains("Cookies"));
    }

    #[test]
    fn no_trim_when_within_budget() {
        let items = vec![(chunk("short chunk").0, 0.5_f32)];
        let focus = ScoreBasedFocusMode::default();
        let out = focus.compress(&items, "x", 5).unwrap();
        assert!(!out[0].trimmed);
    }
}
