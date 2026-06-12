use contracts::chat::{AnswerBlock, Citation};

pub fn plain_text_answer_blocks(text: &str) -> Vec<AnswerBlock> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![AnswerBlock::Text {
            text: trimmed.to_string(),
            citations: Vec::new(),
        }]
    }
}

pub fn answer_blocks_to_markup(answer_blocks: &[AnswerBlock]) -> String {
    let mut segments = Vec::new();

    for block in answer_blocks {
        match block {
            AnswerBlock::Text { text, citations } => {
                let trimmed = text.trim();
                if trimmed.is_empty() && citations.is_empty() {
                    continue;
                }
                if citations.is_empty() {
                    segments.push(trimmed.to_string());
                } else {
                    let inline = citations
                        .iter()
                        .map(|chunk_id| format!("[[cite:{chunk_id}]]"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    segments.push(format!("{trimmed} {inline}").trim().to_string());
                }
            }
            AnswerBlock::Image { chunk_id } => {
                let trimmed = chunk_id.trim();
                if !trimmed.is_empty() {
                    segments.push(format!("[[image:{trimmed}]]"));
                }
            }
        }
    }

    segments.join("\n\n").trim().to_string()
}

fn citation_chunk_id_by_display_id(citations: &[Citation], display_id: u64) -> Option<String> {
    citations.iter().enumerate().find_map(|(index, citation)| {
        let citation_id = if citation.citation_id > 0 {
            citation.citation_id as u64
        } else {
            (index + 1) as u64
        };
        (citation_id == display_id)
            .then(|| citation.chunk_id.clone())
            .flatten()
    })
}

fn citation_chunk_id_from_token(token: &str, citations: &[Citation]) -> Option<String> {
    if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
        return (!chunk_id.is_empty()).then(|| chunk_id.to_string());
    }
    token
        .parse::<u64>()
        .ok()
        .and_then(|display_id| citation_chunk_id_by_display_id(citations, display_id))
}

fn image_chunk_id_from_token(token: &str, citations: &[Citation]) -> Option<String> {
    if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim) {
        if !chunk_id.is_empty() && !chunk_id.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(chunk_id.to_string());
        }
        return chunk_id
            .parse::<u64>()
            .ok()
            .and_then(|display_id| citation_chunk_id_by_display_id(citations, display_id));
    }
    None
}

fn normalize_block_text(text: &str) -> String {
    let mut normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for (from, to) in [
        (" .", "."),
        (" ,", ","),
        (" ;", ";"),
        (" :", ":"),
        (" !", "!"),
        (" ?", "?"),
        ("( ", "("),
        (" )", ")"),
    ] {
        normalized = normalized.replace(from, to);
    }
    normalized.trim().to_string()
}

pub fn answer_blocks_from_rendered_answer(
    answer: &str,
    citations: &[Citation],
) -> Vec<AnswerBlock> {
    let mut blocks = Vec::new();

    for raw_block in answer.split("\n\n") {
        let trimmed = raw_block.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(chunk_id) = trimmed
            .strip_prefix("[[image:")
            .and_then(|rest| rest.strip_suffix("]]"))
            .map(str::trim)
            .and_then(|token| image_chunk_id_from_token(&format!("image:{token}"), citations))
        {
            blocks.push(AnswerBlock::Image { chunk_id });
            continue;
        }

        let mut remaining = trimmed;
        let mut text = String::new();
        let mut chunk_ids = Vec::new();

        while let Some(start) = remaining.find("[[") {
            text.push_str(&remaining[..start]);
            let after_start = &remaining[start + 2..];
            let Some(end) = after_start.find("]]") else {
                text.push_str(&remaining[start..]);
                remaining = "";
                break;
            };
            let token = after_start[..end].trim();
            if let Some(chunk_id) = citation_chunk_id_from_token(token, citations) {
                if !chunk_ids.iter().any(|existing| existing == &chunk_id) {
                    chunk_ids.push(chunk_id);
                }
            } else if image_chunk_id_from_token(token, citations).is_none() {
                text.push_str(&remaining[start..start + 2 + end + 2]);
            }
            remaining = &after_start[end + 2..];
        }
        text.push_str(remaining);

        let normalized = normalize_block_text(&text);
        if !normalized.is_empty() || !chunk_ids.is_empty() {
            blocks.push(AnswerBlock::Text {
                text: normalized,
                citations: chunk_ids,
            });
        }
    }

    if blocks.is_empty() {
        return plain_text_answer_blocks(answer);
    }

    blocks
}
