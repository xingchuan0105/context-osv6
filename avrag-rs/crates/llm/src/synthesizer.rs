use crate::ModelProviderConfig;
use crate::client::{ChatMessage, LlmClient};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeSet, HashSet};
use tokio_util::sync::CancellationToken;

const SYNTHESIZER_SYSTEM_PROMPT: &str = include_str!("../../../prompts/synthesis/rag-answer.md");
const DEFAULT_SYNTHESIZER_USER_TEMPLATE: &str =
    include_str!("../../../prompts/templates/synthesizer-user.tmpl");

#[derive(Debug, Clone)]
pub struct SynthesisOutput {
    pub answer_text: String,
    pub answer_blocks: Vec<common::AnswerBlock>,
    pub cited_chunk_ids: Vec<String>,
    pub llm_usage: Option<crate::LlmUsage>,
}

pub struct SynthesizeStreamParams<'a> {
    pub query: &'a str,
    pub context_chunks: &'a [common::AnswerContextChunk],
    pub rag_plan: &'a Option<common::RagPlan>,
    pub item_traces: &'a [common::RagTraceItem],
    pub history: Option<&'a [ChatMessage]>,
    pub token: CancellationToken,
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
    DEFAULT_SYNTHESIZER_USER_TEMPLATE
        .replace("{query}", query)
        .replace("{index_section}", index_section)
        .replace("{context_section}", context_section)
}

// ---------------------------------------------------------------------------
// Tool-result → synthesis context helpers (new tool-call paradigm)
// ---------------------------------------------------------------------------

fn tool_status_str(status: common::ToolStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extract `AnswerContextChunk`s from a slice of `ToolResult`s.
/// Only `Ok` results with array `data` are considered.
/// Deduplicates by `chunk_id`; first occurrence wins.
pub fn tool_results_to_context_chunks(
    tool_results: &[common::ToolResult],
) -> Vec<common::AnswerContextChunk> {
    let mut chunks = Vec::new();
    let mut seen = HashSet::new();

    for result in tool_results {
        if result.status != common::ToolStatus::Ok {
            continue;
        }
        let Some(data) = result.data.as_ref().and_then(|d| d.as_array()) else {
            continue;
        };
        for item in data {
            let chunk_id = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if chunk_id.is_empty() || !seen.insert(chunk_id.clone()) {
                continue;
            }
            chunks.push(common::AnswerContextChunk {
                chunk_id,
                doc_id: item
                    .get("doc_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                chunk_type: "text".to_string(),
                page: item.get("page").and_then(|v| v.as_i64()),
                text: item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
            });
        }
    }
    chunks
}

/// Build a retrieval-index JSON from `ToolResult`s.
/// Each tool result becomes a "path" with its name, status, trace, and chunk count.
pub fn build_tool_result_index(
    original_query: &str,
    tool_results: &[common::ToolResult],
    context_chunk_count: usize,
) -> String {
    let recalled_chunk_ids: BTreeSet<String> = tool_results
        .iter()
        .filter(|r| r.status == common::ToolStatus::Ok)
        .flat_map(|r| {
            r.data
                .as_ref()
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            item.get("chunk_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();

    let tool_paths: Vec<_> = tool_results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let chunk_count = result
                .data
                .as_ref()
                .and_then(|d| d.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            let trace = result.trace.as_ref();
            json!({
                "path_id": index + 1,
                "tool": result.tool,
                "version": result.version,
                "status": tool_status_str(result.status),
                "chunk_count": chunk_count,
                "elapsed_ms": trace.and_then(|t| t.elapsed_ms),
                "raw_hit_count": trace.and_then(|t| t.raw_hit_count),
                "degrade_reason": trace.and_then(|t| t.degrade_reason.clone()),
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "original_query": original_query,
        "grounding": {
            "tool_call_count": tool_results.len(),
            "recalled_chunk_count": recalled_chunk_ids.len(),
            "context_chunk_count": context_chunk_count,
            "zero_recall": recalled_chunk_ids.is_empty(),
        },
        "tool_paths": tool_paths,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Build a context-section JSON from `ToolResult`s.
/// Each chunk object is annotated with a `tool_source` field so the
/// synthesizer knows which tool produced it.
pub fn build_tool_result_context_section(tool_results: &[common::ToolResult]) -> String {
    let mut all_chunks = Vec::new();
    let mut seen = HashSet::new();

    for result in tool_results {
        if result.status != common::ToolStatus::Ok {
            continue;
        }
        let Some(data) = result.data.as_ref().and_then(|d| d.as_array()) else {
            continue;
        };
        for item in data {
            let chunk_id = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if chunk_id.is_empty() || !seen.insert(chunk_id.clone()) {
                continue;
            }
            let mut chunk = item.clone();
            chunk["tool_source"] = serde_json::Value::String(result.tool.clone());
            all_chunks.push(chunk);
        }
    }

    serde_json::to_string_pretty(&all_chunks).unwrap_or_else(|_| "[]".to_string())
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
    system_prompt: String,
}

impl AnswerSynthesizer {
    pub fn new(answer_config: ModelProviderConfig) -> Self {
        Self {
            llm: LlmClient::new(answer_config),
            system_prompt: SYNTHESIZER_SYSTEM_PROMPT.to_string(),
        }
    }

    pub fn from_llm_client(llm: LlmClient) -> Self {
        Self {
            llm,
            system_prompt: SYNTHESIZER_SYSTEM_PROMPT.to_string(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    pub async fn synthesize(
        &self,
        query: &str,
        context_chunks: &[common::AnswerContextChunk],
        rag_plan: &Option<common::RagPlan>,
        item_traces: &[common::RagTraceItem],
        history: Option<&[ChatMessage]>,
    ) -> anyhow::Result<SynthesisOutput> {
        let mut messages = vec![ChatMessage::system(&self.system_prompt)];

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
        params: SynthesizeStreamParams<'_>,
        on_delta: impl FnMut(&str),
    ) -> anyhow::Result<crate::LlmResponse> {
        let mut messages = vec![ChatMessage::system(&self.system_prompt)];

        if let Some(hist) = params.history {
            messages.extend(hist.iter().cloned());
        }

        let index_section = build_retrieval_index(
            params.query,
            params.rag_plan,
            params.item_traces,
            params.context_chunks.len(),
        );
        let context_section = build_context_section(params.context_chunks);
        messages.push(ChatMessage::user(build_synthesis_request(
            params.query,
            &index_section,
            &context_section,
        )));

        self.llm
            .complete_stream(&messages, Some(0.7), params.token, on_delta, |_| {})
            .await
            .context("Failed to stream synthesizer response")
    }

    /// Synthesize an answer from `Vec<ToolResult>` (tool-call paradigm).
    ///
    /// This is the synthesizer entry-point for the new runtime architecture where
    /// the planner emits `ToolCall`s and the runtime returns `ToolResult`s.
    /// Chunks are extracted from successful tool results, deduplicated, and
    /// annotated with their `tool_source` so the LLM knows which evidence came
    /// from which retrieval pipeline.
    pub async fn synthesize_from_tool_results(
        &self,
        query: &str,
        tool_results: &[common::ToolResult],
        history: Option<&[ChatMessage]>,
    ) -> anyhow::Result<SynthesisOutput> {
        let mut messages = vec![ChatMessage::system(&self.system_prompt)];

        if let Some(hist) = history {
            messages.extend(hist.iter().cloned());
        }

        let context_chunks = tool_results_to_context_chunks(tool_results);
        let index_section = build_tool_result_index(query, tool_results, context_chunks.len());
        let context_section = build_tool_result_context_section(tool_results);
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

    /// Stream an answer from `Vec<ToolResult>`.
    pub async fn synthesize_stream_text_from_tool_results(
        &self,
        query: &str,
        tool_results: &[common::ToolResult],
        history: Option<&[ChatMessage]>,
        token: CancellationToken,
        on_delta: impl FnMut(&str),
    ) -> anyhow::Result<crate::LlmResponse> {
        let mut messages = vec![ChatMessage::system(&self.system_prompt)];

        if let Some(hist) = history {
            messages.extend(hist.iter().cloned());
        }

        let context_chunks = tool_results_to_context_chunks(tool_results);
        let index_section = build_tool_result_index(query, tool_results, context_chunks.len());
        let context_section = build_tool_result_context_section(tool_results);
        messages.push(ChatMessage::user(build_synthesis_request(
            query,
            &index_section,
            &context_section,
        )));

        self.llm
            .complete_stream(&messages, Some(0.7), token, on_delta, |_| {})
            .await
            .context("Failed to stream synthesizer response")
    }
}

#[cfg(test)]
mod tests;
