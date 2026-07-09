//! Integration test verifying that `ChunkPolicy::overlap_chars` is actually
//! applied: adjacent text chunks produced by the chunker must share some
//! overlapping text at their boundary.
//!
//! See chunker.rs `token_chunk_config` for the implementation. Capacity is
//! fixed at `TARGET_CHUNK_TOKENS` (512) tokens, so we feed a document large
//! enough to span several chunks.

use std::collections::BTreeMap;

use ingestion::chunker::{ChunkPolicy, build_chunk_plan};
use ingestion::parser::{NormalizedDocument, ParsedUnit};

/// Build a `NormalizedDocument` consisting of a single text unit containing
/// `word_count` unique tokens, so it is guaranteed to be split into multiple
/// chunks (capacity is ~512 tokens ≈ 2048 words).
fn document_with_unique_words(word_count: usize) -> NormalizedDocument {
    let text = (0..word_count)
        .map(|i| format!("word{i:04}"))
        .collect::<Vec<_>>()
        .join(" ");
    NormalizedDocument {
        title: "overlap-test".to_string(),
        units: vec![ParsedUnit::new_text(
            1,
            text,
            "test-backend".to_string(),
        )],
        metadata: BTreeMap::new(),
    }
}

/// Return the set of whitespace-split tokens for a chunk's text.
fn token_set(text: &str) -> std::collections::HashSet<&str> {
    text.split_whitespace().collect()
}

#[test]
fn adjacent_chunks_share_overlapping_text_when_overlap_set() {
    // ~3000 unique words → comfortably more than one 512-token chunk.
    let doc = document_with_unique_words(3000);

    // Non-zero overlap (larger than the default 64 to make the shared region
    // easy to detect).
    let policy = ChunkPolicy {
        overlap_chars: 512,
        ..ChunkPolicy::default()
    };

    let plan = build_chunk_plan(&doc, "notes.txt", &policy);

    // We expect multiple chunks.
    assert!(
        plan.text_chunks.len() > 1,
        "expected more than one chunk, got {}",
        plan.text_chunks.len()
    );

    // For each pair of adjacent chunks, the tail tokens of chunk N must appear
    // among the leading tokens of chunk N+1 (this is exactly what overlap
    // guarantees: chunk N+1 starts partway into chunk N's content).
    let mut any_overlap = false;
    for window in plan.text_chunks.windows(2) {
        let prev = &window[0].text;
        let next = &window[1].text;

        let prev_tokens: Vec<&str> = prev.split_whitespace().collect();
        let next_lower = next.to_lowercase();
        let next_head: std::collections::HashSet<&str> = token_set(&next_lower);

        // Look at the last ~40 tokens of the previous chunk and require at
        // least one of them to show up at the very start of the next chunk.
        let tail = prev_tokens.len().saturating_sub(40);
        let tail_overlap: Vec<&&str> = prev_tokens[tail..]
            .iter()
            .filter(|t| next_head.contains(**t))
            .collect();

        assert!(
            !tail_overlap.is_empty(),
            "expected overlapping tokens between adjacent chunks, but the tail \
             of a chunk did not appear in the head of the next one.\n\
             --- prev tail ---\n{}\n--- next head ---\n{}",
            prev_tokens[tail..].join(" "),
            next.split_whitespace().take(40).collect::<Vec<_>>().join(" "),
        );
        any_overlap = true;
    }

    assert!(any_overlap, "no adjacent chunk pairs were checked for overlap");
}

#[test]
fn no_overlap_when_overlap_chars_is_zero() {
    let doc = document_with_unique_words(3000);

    let policy = ChunkPolicy {
        overlap_chars: 0,
        ..ChunkPolicy::default()
    };

    let plan = build_chunk_plan(&doc, "notes.txt", &policy);
    assert!(
        plan.text_chunks.len() > 1,
        "expected more than one chunk, got {}",
        plan.text_chunks.len()
    );

    // With overlap disabled, the final tokens of chunk N must NOT be repeated
    // at the start of chunk N+1. Because our tokens are globally unique
    // (word0000..word2999), any shared token between adjacent chunks would
    // prove unwanted overlap.
    for window in plan.text_chunks.windows(2) {
        let prev_tail: std::collections::HashSet<&str> = {
            let tokens: Vec<&str> = window[0].text.split_whitespace().collect();
            let start = tokens.len().saturating_sub(40);
            tokens[start..].iter().copied().collect()
        };
        let next_head: Vec<&str> =
            window[1].text.split_whitespace().take(40).collect();

        let shared: Vec<&&str> = next_head.iter().filter(|t| prev_tail.contains(**t)).collect();
        assert!(
            shared.is_empty(),
            "overlap was disabled but adjacent chunks still share tokens: {:?}",
            shared
        );
    }
}
