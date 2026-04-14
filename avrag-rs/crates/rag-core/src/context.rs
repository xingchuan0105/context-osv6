use common::ChatMessage;
use serde::{Deserialize, Serialize};
use tiktoken_rs::CoreBPE;
use uuid::Uuid;

use crate::retrieval::ScoredChunk;

/// Session context loaded from storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    /// Recent chat messages in the session
    pub messages: Vec<ChatMessage>,
    /// Optional summary of the conversation
    pub summary: Option<String>,
}

/// Citation validation result
#[derive(Debug, Clone)]
pub struct CitationValidation {
    pub citation_id: String,
    pub valid: bool,
    pub chunk_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EvidenceIndex {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub chunk_type: String,
    pub page: Option<i64>,
    pub retrieval_channel: String,
    pub asset_id: Option<Uuid>,
    pub caption: Option<String>,
    pub image_path: Option<String>,
    pub context_excerpt: Option<String>,
}

fn tokenizer() -> CoreBPE {
    tiktoken_rs::cl100k_base().expect("failed to load cl100k_base tokenizer")
}

pub fn count_tokens(text: &str) -> usize {
    let bpe = tokenizer();
    bpe.encode_ordinary(text).len()
}

pub fn assemble_llm_context_layered(
    summaries: &[(Uuid, String)],
    retrieval_chunks: &[ScoredChunk],
    summary_budget: usize,
    retrieval_budget: usize,
) -> (String, usize, usize, Vec<EvidenceIndex>) {
    let bpe = tokenizer();
    let mut context = String::new();
    let mut summary_tokens_used = 0;
    let mut retrieval_tokens_used = 0;
    let mut evidence_index = Vec::new();

    if !summaries.is_empty() {
        context.push_str("[文档摘要]\n");
        for (_doc_id, content) in summaries {
            let prefixed = format!("{}\n", content);
            let tokens = bpe.encode_ordinary(&prefixed).len();
            if summary_tokens_used + tokens > summary_budget {
                break;
            }
            context.push_str(&prefixed);
            summary_tokens_used += tokens;
        }
        context.push('\n');
    }

    if !retrieval_chunks.is_empty() {
        if !context.is_empty() {
            context.push_str("\n[检索结果]\n");
        }
        for chunk in retrieval_chunks {
            let chunk_type = chunk.chunk_type.as_str();

            let entry = format!(
                "[{}|{}] {}\n",
                chunk
                    .chunk_id
                    .to_string()
                    .chars()
                    .take(8)
                    .collect::<String>(),
                chunk_type,
                chunk.content
            );
            let tokens = bpe.encode_ordinary(&entry).len();
            if retrieval_tokens_used + tokens > retrieval_budget {
                break;
            }
            context.push_str(&entry);
            retrieval_tokens_used += tokens;

            evidence_index.push(EvidenceIndex {
                chunk_id: chunk.chunk_id,
                doc_id: chunk.doc_id,
                chunk_type: chunk_type.to_string(),
                page: chunk.page,
                retrieval_channel: chunk.source.clone(),
                asset_id: chunk.asset_id,
                caption: chunk.caption.clone(),
                image_path: chunk.image_path.clone(),
                context_excerpt: Some(chunk.content.chars().take(100).collect()),
            });
        }
    }

    (
        context,
        summary_tokens_used,
        retrieval_tokens_used,
        evidence_index,
    )
}

pub fn assemble_llm_context(chunks: Vec<ScoredChunk>, max_tokens: usize) -> String {
    let (ctx, _, _, _) = assemble_llm_context_layered(&[], &chunks, 0, max_tokens);
    ctx
}
