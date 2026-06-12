use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SynthesisOutput {
    pub answer_text: String,
    pub answer_blocks: Vec<contracts::chat::AnswerBlock>,
    pub cited_chunk_ids: Vec<String>,
    pub llm_usage: Option<crate::LlmUsage>,
}

#[derive(Debug, Deserialize)]
struct RawSynthesisOutput {
    answer_text: String,
    #[serde(default)]
    cited_chunk_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BlockSynthesisOutput {
    #[serde(default)]
    answer_blocks: Vec<RawAnswerBlock>,
    #[serde(default)]
    cited_chunk_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawAnswerBlock {
    Text {
        text: String,
        #[serde(default)]
        citations: Vec<String>,
    },
    Image {
        chunk_id: String,
    },
}

fn append_unique_chunk_ids(
    ids: &mut Vec<String>,
    seen: &mut HashSet<String>,
    new_ids: impl IntoIterator<Item = String>,
) {
    for chunk_id in new_ids {
        let trimmed = chunk_id.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.clone()) {
            ids.push(trimmed);
        }
    }
}

fn build_answer_text_from_blocks(blocks: &[RawAnswerBlock]) -> (String, Vec<String>) {
    let mut segments = Vec::new();
    let mut cited_chunk_ids: Vec<String> = Vec::new();
    let mut seen = HashSet::new();

    for block in blocks {
        match block {
            RawAnswerBlock::Text { citations, .. } => {
                for chunk_id in citations {
                    let trimmed = chunk_id.trim().to_string();
                    if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
                        cited_chunk_ids.push(trimmed);
                    }
                }
            }
            RawAnswerBlock::Image { chunk_id } => {
                let trimmed = chunk_id.trim().to_string();
                if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
                    cited_chunk_ids.push(trimmed);
                }
            }
        }
    }

    let chunk_to_idx: std::collections::HashMap<String, usize> = cited_chunk_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i + 1))
        .collect();

    for block in blocks {
        match block {
            RawAnswerBlock::Text { text, citations } => {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let valid_citations = citations
                    .iter()
                    .map(|chunk_id| chunk_id.trim().to_string())
                    .filter(|chunk_id| !chunk_id.is_empty())
                    .collect::<Vec<_>>();

                if valid_citations.is_empty() {
                    segments.push(text.to_string());
                } else {
                    let inline = valid_citations
                        .iter()
                        .filter_map(|chunk_id| chunk_to_idx.get(chunk_id))
                        .map(|idx| format!("[[{idx}]]"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    segments.push(format!("{text} {inline}"));
                }
            }
            RawAnswerBlock::Image { chunk_id } => {
                let chunk_id = chunk_id.trim();
                if chunk_id.is_empty() {
                    continue;
                }
                segments.push(format!("[[image:{chunk_id}]]"));
            }
        }
    }

    (segments.join("\n\n").trim().to_string(), cited_chunk_ids)
}

pub fn parse_synthesis_output(raw_output: &str) -> SynthesisOutput {
    let trimmed = raw_output.trim();

    let block_parsed = serde_json::from_str::<BlockSynthesisOutput>(trimmed)
        .ok()
        .or_else(|| {
            extract_json_code_block(trimmed)
                .and_then(|json| serde_json::from_str::<BlockSynthesisOutput>(&json).ok())
        });

    if let Some(parsed) = block_parsed
        && !parsed.answer_blocks.is_empty()
    {
        let (answer_text, mut cited_chunk_ids) =
            build_answer_text_from_blocks(&parsed.answer_blocks);
        let mut seen = cited_chunk_ids.iter().cloned().collect::<HashSet<_>>();
        append_unique_chunk_ids(&mut cited_chunk_ids, &mut seen, parsed.cited_chunk_ids);
        return SynthesisOutput {
            answer_text,
            answer_blocks: parsed
                .answer_blocks
                .iter()
                .map(|block| match block {
                    RawAnswerBlock::Text { text, citations } => contracts::chat::AnswerBlock::Text {
                        text: text.trim().to_string(),
                        citations: citations
                            .iter()
                            .map(|chunk_id| chunk_id.trim().to_string())
                            .filter(|chunk_id| !chunk_id.is_empty())
                            .collect(),
                    },
                    RawAnswerBlock::Image { chunk_id } => contracts::chat::AnswerBlock::Image {
                        chunk_id: chunk_id.trim().to_string(),
                    },
                })
                .collect(),
            cited_chunk_ids,
            llm_usage: None,
        };
    }

    let parsed = serde_json::from_str::<RawSynthesisOutput>(trimmed)
        .ok()
        .or_else(|| {
            extract_json_code_block(trimmed)
                .and_then(|json| serde_json::from_str::<RawSynthesisOutput>(&json).ok())
        });

    if let Some(parsed) = parsed {
        return SynthesisOutput {
            answer_text: parsed.answer_text.trim().to_string(),
            answer_blocks: common::plain_text_answer_blocks(&parsed.answer_text),
            cited_chunk_ids: parsed
                .cited_chunk_ids
                .into_iter()
                .map(|chunk_id| chunk_id.trim().to_string())
                .filter(|chunk_id| !chunk_id.is_empty())
                .collect(),
            llm_usage: None,
        };
    }

    SynthesisOutput {
        answer_text: trimmed.to_string(),
        answer_blocks: common::plain_text_answer_blocks(trimmed),
        cited_chunk_ids: Vec::new(),
        llm_usage: None,
    }
}

fn extract_json_code_block(raw_output: &str) -> Option<String> {
    let start = raw_output.find("```json")?;
    let after_fence = raw_output[start + "```json".len()..].trim_start();
    let end = after_fence.find("```")?;
    Some(after_fence[..end].trim().to_string())
}

#[cfg(test)]
mod tests;
