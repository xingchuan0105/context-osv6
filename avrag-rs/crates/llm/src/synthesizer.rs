use crate::ModelProviderConfig;
use crate::client::{ChatMessage, LlmClient};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeSet, HashSet};

const SYNTHESIZER_SYSTEM_PROMPT: &str = r#"You are a grounded answer agent.

Your job is to answer the user's latest question using the provided retrieval evidence.
Do not answer from unsupported prior knowledge when grounding is missing, weak, or partial.
Do not explain your internal process.
Return raw JSON only.

Evidence priority:
1. Read the Retrieval Index first to understand retrieval attempts, query rewrites, bm25 terms, summary policy, recall status, and retrieval coverage.
2. Use Context Chunks as the primary grounding evidence.
3. If Context Chunks support only part of the question, answer only the supported part and explicitly state which part is not grounded.
4. If retrieval clearly provides no grounded support or insufficient grounded support, say that the current retrieval did not return enough grounded material to answer reliably, and ask the user to rephrase, add more specific keywords, or narrow the scope.
5. Reply in the same language as the user's question unless the conversation strongly indicates another language.

Grounding rules:
- Treat Context Chunks as structured evidence, not loose prose.
- Use only information grounded in the provided chunk fields.
- Do not invent facts, details, implications, or visual content not supported by the chunks.
- Do not present unsupported inferences as certain.
- If grounded context exists, do not claim that there were no results.
- Prefer grounded evidence over generic world knowledge.

Composition rules:
- Build the answer as a sequence of short answer blocks.
- Prefer one factual sentence per text block.
- Keep each text block tightly scoped to one supported claim or one short connected claim set.
- Do not merge many unrelated claims into one long block.
- If the answer has multiple points, split them into separate text blocks.

Citation rules:
- Every text block containing grounded factual content must include a non-empty `citations` array.
- `citations` must list the `chunk_id` values that directly support that block.
- Use the smallest set of chunk ids that materially supports the block.
- Do not attach citations to blocks that are purely conversational fillers.
- Do not include unsupported chunk ids.
- `cited_chunk_ids` must be the deduplicated union of all chunk ids used in all blocks.

Image rules:
- If a chunk has `chunk_type = "image_with_context"`, treat `text`, `caption`, and `image_url` as the complete allowed visual grounding package.
- Do not infer objects, attributes, layout, or relationships beyond those fields.
- Only emit an image block when the image materially helps the answer.
- An image block must reference a valid image chunk by `chunk_id`.
- If an image is included, the immediately preceding text block should introduce why the image is relevant.

Output schema:
{
  "answer_blocks": [
    {
      "type": "text",
      "text": "human-facing answer sentence",
      "citations": ["chunk-id-1"]
    },
    {
      "type": "text",
      "text": "another grounded sentence",
      "citations": ["chunk-id-2", "chunk-id-3"]
    },
    {
      "type": "image",
      "chunk_id": "chunk-id-9"
    }
  ],
  "cited_chunk_ids": ["chunk-id-1", "chunk-id-2", "chunk-id-3", "chunk-id-9"]
}

Schema rules:
- Return exactly one raw JSON object.
- Do not output markdown, code fences, comments, or explanations.
- Do not output any top-level fields other than `answer_blocks` and `cited_chunk_ids`.
- `answer_blocks` must be an array.
- `cited_chunk_ids` must be an array.
- Each block must have a `type` field.
- Allowed block types are only `text` and `image`.

Text block rules:
- A text block must contain exactly:
  - `type`: `"text"`
  - `text`: string
  - `citations`: array of valid `chunk_id` values from Context Chunks
- Do not add extra fields.
- `text` must be plain user-facing answer text.
- `citations` may be empty only when the block contains no grounded factual claim, such as a brief uncertainty notice or a clarification-style transition.

Image block rules:
- An image block must contain exactly:
  - `type`: `"image"`
  - `chunk_id`: valid image chunk id from Context Chunks
- Do not add extra fields.
- The image block's `chunk_id` must also appear in `cited_chunk_ids`.

No-support behavior:
- If there is no grounded support, return one or more text blocks explaining that the current retrieval did not return enough grounded material to answer reliably and suggesting the user rephrase, add specific keywords, or narrow the scope.
- In that case, `cited_chunk_ids` should be an empty array unless a block explicitly cites a chunk to explain the retrieval state.

General output rules:
- Keep the answer concise but complete.
- Prefer short, high-confidence blocks over long synthesized paragraphs.
- Do not emit control tokens or placeholder markers like `[INSUFFICIENT_EVIDENCE]`.
"#;

const SYNTHESIZER_STREAM_SYSTEM_PROMPT: &str = r#"You are a grounded answer agent.

Answer the user's latest question using only the provided retrieval evidence.
Do not mention internal planning, tool calls, or hidden reasoning.
Do not output JSON.
Do not output markdown code fences.
Do not include inline citation markers, chunk ids, or source ids.
Reply in the same language as the user's question unless the conversation strongly indicates another language.
If the evidence is partial, answer only the grounded portion and clearly note what remains uncertain.
If the evidence is insufficient, say so plainly and suggest how the user can refine the request.
"#;

#[derive(Debug, Clone)]
pub struct SynthesisOutput {
    pub answer_text: String,
    pub answer_blocks: Vec<common::AnswerBlock>,
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

fn payload_kind(
    plan_item: Option<&common::RagPlanItem>,
    trace_item: Option<&common::RagTraceItem>,
) -> String {
    if let Some(item) = plan_item {
        if item.summary.is_some() {
            return "summary".to_string();
        }
        if item
            .bm25_terms
            .as_ref()
            .is_some_and(|terms| !terms.is_empty())
        {
            return "bm25_terms".to_string();
        }
        if item
            .query
            .as_ref()
            .is_some_and(|query| !query.trim().is_empty())
        {
            return "query".to_string();
        }
    }
    trace_item
        .map(|item| item.payload_kind.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_retrieval_index(
    original_query: &str,
    rag_plan: &Option<common::RagPlan>,
    item_traces: &[common::RagTraceItem],
    context_chunk_count: usize,
) -> String {
    let total_paths = rag_plan
        .as_ref()
        .map(|plan| plan.items.len())
        .unwrap_or_default()
        .max(item_traces.len());
    let recalled_chunk_ids = item_traces
        .iter()
        .flat_map(|item| item.source_ids.iter().cloned())
        .collect::<BTreeSet<_>>();

    let retrieval_paths: Vec<_> = (0..total_paths)
        .map(|index| {
            let plan_item = rag_plan.as_ref().and_then(|plan| plan.items.get(index));
            let trace_item = item_traces.get(index);
            let payload_kind = payload_kind(plan_item, trace_item);
            let query = plan_item
                .and_then(|item| item.query.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| trace_item.and_then(|item| item.query.clone()));
            let bm25_terms = plan_item
                .and_then(|item| item.bm25_terms.clone())
                .unwrap_or_else(|| trace_item.map(|item| item.bm25_terms.clone()).unwrap_or_default());
            let summary = plan_item
                .and_then(|item| item.summary.clone())
                .or_else(|| trace_item.and_then(|item| item.summary.clone()));

            json!({
                "path_id": index + 1,
                "priority": plan_item.map(|item| item.priority).or_else(|| trace_item.map(|item| item.priority)).unwrap_or(0.0),
                "payload_kind": payload_kind,
                "query": query,
                "bm25_terms": bm25_terms,
                "summary": summary,
                "recall": {
                    "source_count": trace_item.map(|item| item.source_count).unwrap_or_default(),
                    "chunk_ids": trace_item.map(|item| item.source_ids.clone()).unwrap_or_default(),
                    "budget": {
                        "recall_budget": trace_item.map(|item| item.recall_budget),
                        "bm25_k": trace_item.map(|item| item.bm25_k),
                        "dense_k": trace_item.map(|item| item.dense_k),
                        "rerank_budget": trace_item.map(|item| item.rerank_budget),
                    }
                }
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "original_query": original_query,
        "grounding": {
            "retrieval_path_count": total_paths,
            "recalled_chunk_count": recalled_chunk_ids.len(),
            "context_chunk_count": context_chunk_count,
            "zero_recall": recalled_chunk_ids.is_empty(),
        },
        "plan_version": rag_plan.as_ref().map(|plan| plan.plan_version.clone()).unwrap_or_else(|| "none".to_string()),
        "plan_confidence": rag_plan.as_ref().map(|plan| plan.plan_confidence),
        "clarify_needed": rag_plan.as_ref().map(|plan| plan.clarify_needed).unwrap_or(false),
        "path_count": total_paths,
        "retrieval_paths": retrieval_paths,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn build_context_section(context_chunks: &[common::AnswerContextChunk]) -> String {
    if context_chunks.is_empty() {
        return "[]".to_string();
    }

    serde_json::to_string_pretty(context_chunks).unwrap_or_else(|_| "[]".to_string())
}

fn build_synthesis_request(query: &str, index_section: &str, context_section: &str) -> String {
    format!(
        "User Question:\n{}\n\nRetrieval Index (JSON):\n{}\n\nContext Chunks (JSON array of objects with fields: chunk_id, doc_id, chunk_type, page, text, asset_id, caption, image_url, parser_backend, source_locator):\n{}",
        query, index_section, context_section
    )
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

    // First pass to collect all unique cited_chunk_ids in order of appearance
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

    // Map chunk_id to its 1-based index in cited_chunk_ids
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

fn parse_synthesis_output(raw_output: &str) -> SynthesisOutput {
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
                    RawAnswerBlock::Text { text, citations } => common::AnswerBlock::Text {
                        text: text.trim().to_string(),
                        citations: citations
                            .iter()
                            .map(|chunk_id| chunk_id.trim().to_string())
                            .filter(|chunk_id| !chunk_id.is_empty())
                            .collect(),
                    },
                    RawAnswerBlock::Image { chunk_id } => common::AnswerBlock::Image {
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

pub struct AnswerSynthesizer {
    llm: LlmClient,
}

impl AnswerSynthesizer {
    pub fn new(answer_config: ModelProviderConfig) -> Self {
        Self {
            llm: LlmClient::new(answer_config),
        }
    }

    pub async fn synthesize(
        &self,
        query: &str,
        context_chunks: &[common::AnswerContextChunk],
        rag_plan: &Option<common::RagPlan>,
        item_traces: &[common::RagTraceItem],
        history: Option<&[ChatMessage]>,
    ) -> anyhow::Result<SynthesisOutput> {
        let mut messages = vec![ChatMessage::system(SYNTHESIZER_SYSTEM_PROMPT)];

        if let Some(hist) = history {
            messages.extend(hist.iter().cloned());
        }

        let index_section =
            build_retrieval_index(query, rag_plan, item_traces, context_chunks.len());
        let context_section = build_context_section(context_chunks);
        messages.push(ChatMessage::user(build_synthesis_request(
            query,
            &index_section,
            &context_section,
        )));

        let response = self
            .llm
            .complete(&messages, Some(0.7))
            .await
            .context("Failed to get synthesizer response")?;

        let mut output = parse_synthesis_output(&response.content);
        output.llm_usage = Some(response.usage);
        Ok(output)
    }

    pub async fn synthesize_stream_text(
        &self,
        query: &str,
        context_chunks: &[common::AnswerContextChunk],
        rag_plan: &Option<common::RagPlan>,
        item_traces: &[common::RagTraceItem],
        history: Option<&[ChatMessage]>,
        on_delta: impl FnMut(&str),
    ) -> anyhow::Result<crate::LlmResponse> {
        let mut messages = vec![ChatMessage::system(SYNTHESIZER_STREAM_SYSTEM_PROMPT)];

        if let Some(hist) = history {
            messages.extend(hist.iter().cloned());
        }

        let index_section =
            build_retrieval_index(query, rag_plan, item_traces, context_chunks.len());
        let context_section = build_context_section(context_chunks);
        messages.push(ChatMessage::user(build_synthesis_request(
            query,
            &index_section,
            &context_section,
        )));

        self.llm
            .complete_stream(&messages, Some(0.7), on_delta)
            .await
            .context("Failed to stream synthesizer response")
    }
}

#[cfg(test)]
mod tests;
