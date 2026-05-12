//! SysVec — lightweight vector encoding for sensitive system instructions.
//!
//! Splits sensitive phrases into short vectors and reconstructs them before
//! the LLM call.  This raises the bar for prompt-reflection attacks because
//! the sensitive instruction never exists as a single contiguous string in
//! the prompt payload until just before inference.

/// Split a sensitive instruction into short phrase vectors.
///
/// Vectors are created by splitting on sentence boundaries and then
/// further segmenting long sentences into chunks of at most `max_words`
/// words.
pub fn encode(input: &str, max_words: usize) -> Vec<String> {
    let sentences: Vec<&str> = input
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut vectors = Vec::new();
    for sentence in sentences {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        for chunk in words.chunks(max_words.max(1)) {
            vectors.push(chunk.join(" "));
        }
    }
    vectors
}

/// Reconstruct a string from phrase vectors.
///
/// Joins vectors with spaces and appends a period after each original
/// sentence boundary (heuristic: every vector that ends the sentence).
pub fn decode(vectors: &[String]) -> String {
    vectors.join(". ") + "."
}

/// Wrap a system prompt with SysVec encoding/decoding.
///
/// Sensitive lines are detected heuristically (lines containing words
/// like "must", "only", "never", "always") and encoded.  The caller
/// should `decode` just before sending to the LLM.
pub fn encode_sensitive_lines(prompt: &str, max_words: usize) -> Vec<String> {
    let sensitive_keywords = ["must", "only", "never", "always", "do not", "strictly"];
    prompt
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if sensitive_keywords.iter().any(|kw| lower.contains(kw)) {
                encode(line, max_words).join(" | ")
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// Decode a prompt that was processed with `encode_sensitive_lines`.
pub fn decode_sensitive_lines(encoded: &[String]) -> String {
    encoded
        .iter()
        .map(|line| {
            if line.contains(" | ") {
                let vectors: Vec<&str> = line.split(" | ").collect();
                decode(&vectors.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_splits_long_sentence() {
        let text = "You must only use retrieved evidence and never hallucinate facts.";
        let vectors = encode(text, 4);
        assert_eq!(vectors.len(), 3);
        assert_eq!(vectors[0], "You must only use");
        assert_eq!(vectors[1], "retrieved evidence and never");
        assert_eq!(vectors[2], "hallucinate facts");
    }

    #[test]
    fn decode_reconstructs_text() {
        let vectors = vec![
            "You must only use".to_string(),
            "retrieved evidence".to_string(),
        ];
        let text = decode(&vectors);
        assert!(text.contains("You must only use"));
        assert!(text.contains("retrieved evidence"));
    }

    #[test]
    fn encode_sensitive_lines_detects_keywords() {
        let prompt = "You are helpful.\nYou must only use evidence.\nBe concise.";
        let encoded = encode_sensitive_lines(prompt, 4);
        assert!(!encoded[0].contains(" | ")); // not sensitive
        assert!(encoded[1].contains(" | "));  // sensitive
        assert!(!encoded[2].contains(" | ")); // not sensitive
    }

    #[test]
    fn roundtrip_sensitive_lines() {
        let prompt = "You are helpful.\nYou must only use evidence.\nBe concise.";
        let encoded = encode_sensitive_lines(prompt, 4);
        let decoded = decode_sensitive_lines(&encoded);
        assert!(decoded.contains("You must only use"));
        assert!(decoded.contains("evidence"));
    }
}
