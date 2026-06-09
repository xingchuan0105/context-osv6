use crate::client::{ChatMessage, LlmClient};
use anyhow::Context;
use common::{Domain, Era, Genre, SummaryMetadata, SummaryOutput};
use serde::Deserialize;
use text_splitter::{ChunkConfig, CodeSplitter, MarkdownSplitter, TextSplitter};
use tiktoken_rs::{CoreBPE, cl100k_base};
const MAX_SUMMARY_CONTEXT_TOKENS: usize = 900_000;
const RESERVED_PROMPT_TOKENS: usize = 4_000;
const MAX_BATCH_CONTEXT_TOKENS: usize = MAX_SUMMARY_CONTEXT_TOKENS - RESERVED_PROMPT_TOKENS;
const DEFAULT_SUMMARY_SYSTEM_PROMPT: &str =
    include_str!("../../../prompts/pipeline/summary-generation.system.v1.md");
const DEFAULT_SUMMARY_FINALIZE_SYSTEM_PROMPT: &str =
    include_str!("../../../prompts/pipeline/summary-generation-finalize.system.v1.md");
const DEFAULT_SUMMARY_USER_TEMPLATE: &str =
    include_str!("../../../prompts/templates/summary-user.tmpl");
const DEFAULT_SUMMARY_FINALIZE_USER_TEMPLATE: &str =
    include_str!("../../../prompts/templates/summary-finalize-user.tmpl");
const SUMMARY_BLOCK_LABELS: &[&str] = &["summary_text", "markdown", "md", "text"];

#[derive(Debug, Clone)]
struct SummaryBatch {
    batch_index: usize,
    batch_count: usize,
    token_count: usize,
    content: String,
}

#[derive(Debug, Clone, Copy)]
enum SummarySplitMode {
    Text,
    Markdown,
    Code(CodeLanguage),
}

#[derive(Debug, Clone, Copy)]
enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    Java,
}

#[derive(Debug, Default, Deserialize)]
struct ModelSummaryMetadata {
    language: Option<String>,
    domain: Option<String>,
    genre: Option<String>,
    era: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelSummaryEnvelope {
    summary_text: String,
    #[serde(default)]
    summary_metadata: ModelSummaryMetadata,
}

/// SummaryGenerator creates structured summaries for documents based on their content type.
pub struct SummaryGenerator {
    llm: LlmClient,
    prompt_template: Option<String>,
    finalize_prompt_template: Option<String>,
}

impl SummaryGenerator {
    pub fn new(config: crate::ModelProviderConfig) -> Self {
        Self {
            llm: LlmClient::new(config),
            prompt_template: None,
            finalize_prompt_template: None,
        }
    }

    pub fn with_prompt_template(mut self, template: impl Into<String>) -> Self {
        let template = template.into();
        self.prompt_template = (!template.trim().is_empty()).then_some(template);
        self
    }

    pub fn with_finalize_prompt_template(mut self, template: impl Into<String>) -> Self {
        let template = template.into();
        self.finalize_prompt_template = (!template.trim().is_empty()).then_some(template);
        self
    }

    pub async fn synthesize(
        &self,
        doc_id: &str,
        title: &str,
        filename: &str,
        content: &str,
    ) -> anyhow::Result<(SummaryOutput, crate::LlmUsage)> {
        let batches = build_summary_batches(filename, content)?;
        self.summarize_batches(doc_id, title, filename, &batches)
            .await
    }

    async fn summarize_batches(
        &self,
        doc_id: &str,
        title: &str,
        filename: &str,
        batches: &[SummaryBatch],
    ) -> anyhow::Result<(SummaryOutput, crate::LlmUsage)> {
        if batches.is_empty() {
            anyhow::bail!("no summary batches available");
        }

        let mut total_usage = crate::LlmUsage::zeroed();
        let mut partial_summaries = Vec::with_capacity(batches.len());
        let system_prompt = build_summary_system_prompt(self.prompt_template.as_deref());

        for batch in batches {
            let user_prompt = build_summary_user_prompt(title, filename, batch);
            let messages = vec![
                ChatMessage::system(system_prompt.clone()),
                ChatMessage::user(user_prompt),
            ];
            let response = self
                .llm
                .complete(&messages, Some(0.3))
                .await
                .context("Failed to get summary response")?;
            total_usage.accumulate(&response.usage);
            partial_summaries.push(response.content);
        }

        let final_text = if partial_summaries.len() == 1 {
            partial_summaries.remove(0)
        } else {
            let finalize_messages = vec![
                ChatMessage::system(build_finalize_system_prompt(
                    self.finalize_prompt_template.as_deref(),
                )),
                ChatMessage::user(build_finalize_user_prompt(
                    title,
                    filename,
                    &partial_summaries,
                )),
            ];
            let response = self
                .llm
                .complete(&finalize_messages, Some(0.2))
                .await
                .context("Failed to get final summary response")?;
            total_usage.accumulate(&response.usage);
            response.content
        };

        // Extract metadata and clean up text
        let (summary_text, metadata) =
            parse_summary_and_metadata(doc_id, title, filename, &final_text);

        Ok((
            SummaryOutput {
                summary_text,
                summary_metadata: metadata,
            },
            total_usage,
        ))
    }
}

fn parse_summary_and_metadata(
    doc_id: &str,
    title: &str,
    filename: &str,
    raw_output: &str,
) -> (String, SummaryMetadata) {
    let trimmed_output = raw_output.trim();

    if let Some((summary_text, metadata)) =
        parse_block_contract(doc_id, title, filename, trimmed_output)
    {
        return (summary_text, metadata);
    }

    if let Some((summary_text, metadata)) =
        parse_json_envelope(doc_id, title, filename, trimmed_output)
    {
        return (summary_text, metadata);
    }

    if let Some((summary_text, metadata)) =
        parse_legacy_text_plus_metadata(doc_id, title, filename, trimmed_output)
    {
        return (summary_text, metadata);
    }

    (
        trimmed_output.to_string(),
        default_summary_metadata(doc_id, title, filename),
    )
}

fn parse_block_contract(
    doc_id: &str,
    title: &str,
    filename: &str,
    raw_output: &str,
) -> Option<(String, SummaryMetadata)> {
    let summary_text = extract_first_code_block(raw_output, SUMMARY_BLOCK_LABELS)?;
    let metadata_json = extract_last_code_block(raw_output, "json")?;
    let metadata = serde_json::from_str::<ModelSummaryMetadata>(&metadata_json).ok();
    Some((
        summary_text,
        build_summary_metadata(doc_id, title, filename, metadata.unwrap_or_default()),
    ))
}

fn parse_json_envelope(
    doc_id: &str,
    title: &str,
    filename: &str,
    raw_output: &str,
) -> Option<(String, SummaryMetadata)> {
    let envelope = serde_json::from_str::<ModelSummaryEnvelope>(raw_output)
        .ok()
        .or_else(|| {
            extract_last_code_block(raw_output, "json")
                .and_then(|json| serde_json::from_str::<ModelSummaryEnvelope>(&json).ok())
        })?;

    Some((
        envelope.summary_text.trim().to_string(),
        build_summary_metadata(doc_id, title, filename, envelope.summary_metadata),
    ))
}

fn parse_legacy_text_plus_metadata(
    doc_id: &str,
    title: &str,
    filename: &str,
    raw_output: &str,
) -> Option<(String, SummaryMetadata)> {
    let json_start = raw_output.rfind("```json")?;
    let summary_text = raw_output[..json_start].trim();
    let metadata_json = extract_last_code_block(raw_output, "json")?;
    let metadata = serde_json::from_str::<ModelSummaryMetadata>(&metadata_json).ok();
    Some((
        summary_text.to_string(),
        build_summary_metadata(doc_id, title, filename, metadata.unwrap_or_default()),
    ))
}

fn default_summary_metadata(doc_id: &str, title: &str, filename: &str) -> SummaryMetadata {
    build_summary_metadata(doc_id, title, filename, ModelSummaryMetadata::default())
}

fn build_summary_metadata(
    doc_id: &str,
    title: &str,
    filename: &str,
    metadata: ModelSummaryMetadata,
) -> SummaryMetadata {
    SummaryMetadata {
        doc_id: doc_id.to_string(),
        filename: filename.to_string(),
        docname: title.to_string(),
        language: metadata
            .language
            .map(|value| normalize_metadata_field(&value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
        domain: metadata
            .domain
            .map(|value| normalize_metadata_field(&value))
            .filter(|value| !value.is_empty())
            .map(|value| Domain::from(value.as_str()))
            .unwrap_or(Domain::Unknown),
        genre: metadata
            .genre
            .map(|value| normalize_metadata_field(&value))
            .filter(|value| !value.is_empty())
            .map(|value| Genre::from(value.as_str()))
            .unwrap_or(Genre::Unknown),
        era: metadata
            .era
            .map(|value| normalize_metadata_field(&value))
            .filter(|value| !value.is_empty())
            .map(|value| Era::from(value.as_str()))
            .unwrap_or(Era::Unknown),
    }
}

fn normalize_metadata_field(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

fn extract_first_code_block(raw_output: &str, labels: &[&str]) -> Option<String> {
    labels
        .iter()
        .find_map(|label| extract_code_block(raw_output, label, false))
}

fn extract_last_code_block(raw_output: &str, label: &str) -> Option<String> {
    extract_code_block(raw_output, label, true)
}

fn extract_code_block(raw_output: &str, label: &str, from_end: bool) -> Option<String> {
    let fence = format!("```{label}");
    let start = if from_end {
        raw_output.rfind(&fence)?
    } else {
        raw_output.find(&fence)?
    };
    let content_start = start + fence.len();
    let after_fence = raw_output[content_start..].trim_start_matches(['\r', '\n', ' ', '\t']);
    let content_end = after_fence.find("```")?;
    Some(after_fence[..content_end].trim().to_string())
}

fn build_summary_system_prompt(prompt_template: Option<&str>) -> String {
    prompt_template
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_SUMMARY_SYSTEM_PROMPT.trim().to_string())
}

fn build_summary_user_prompt(title: &str, filename: &str, batch: &SummaryBatch) -> String {
    DEFAULT_SUMMARY_USER_TEMPLATE
        .replace("{batch_index}", &batch.batch_index.to_string())
        .replace("{batch_count}", &batch.batch_count.to_string())
        .replace("{token_count}", &batch.token_count.to_string())
        .replace("{title}", title)
        .replace("{filename}", filename)
        .replace("{content}", &batch.content)
}
fn build_finalize_system_prompt(prompt_template: Option<&str>) -> String {
    prompt_template
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_SUMMARY_FINALIZE_SYSTEM_PROMPT.trim().to_string())
}

fn build_finalize_user_prompt(title: &str, filename: &str, partial_summaries: &[String]) -> String {
    let partial_text = partial_summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| format!("[partial_summary_{}]\n{}", index + 1, summary.trim()))
        .collect::<Vec<_>>()
        .join("\n\n");
    DEFAULT_SUMMARY_FINALIZE_USER_TEMPLATE
        .replace("{title}", title)
        .replace("{filename}", filename)
        .replace("{partial_count}", &partial_summaries.len().to_string())
        .replace("{partial_text}", &partial_text)
}
fn build_summary_batches(filename: &str, content: &str) -> anyhow::Result<Vec<SummaryBatch>> {
    build_summary_batches_for_limit(filename, content, MAX_BATCH_CONTEXT_TOKENS)
}

fn build_summary_batches_for_limit(
    filename: &str,
    content: &str,
    max_batch_context_tokens: usize,
) -> anyhow::Result<Vec<SummaryBatch>> {
    let content = content.trim();
    if content.is_empty() {
        anyhow::bail!("summary content is empty");
    }

    let tokenizer = cl100k_base().context("failed to initialize summary tokenizer")?;
    let total_tokens = tokenizer.encode_ordinary(content).len();
    let batch_count = total_tokens.div_ceil(max_batch_context_tokens).max(1);
    let target_tokens = total_tokens.div_ceil(batch_count).max(1);
    let segments = split_content_for_summary(filename, content, target_tokens)?;
    let batches = segments
        .into_iter()
        .enumerate()
        .map(|(index, segment)| SummaryBatch {
            batch_index: index + 1,
            batch_count,
            token_count: tokenizer.encode_ordinary(&segment).len(),
            content: segment,
        })
        .collect::<Vec<_>>();
    Ok(batches)
}

fn split_content_for_summary(
    filename: &str,
    content: &str,
    target_tokens: usize,
) -> anyhow::Result<Vec<String>> {
    let config = token_chunk_config(target_tokens);
    let mode = summary_split_mode(filename);
    let segments: Vec<String> = match mode {
        SummarySplitMode::Markdown => MarkdownSplitter::new(config)
            .chunks(content)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        SummarySplitMode::Code(language) => match code_splitter(language, config) {
            Some(splitter) => splitter
                .chunks(content)
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            None => TextSplitter::new(token_chunk_config(target_tokens))
                .chunks(content)
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        },
        SummarySplitMode::Text => TextSplitter::new(config)
            .chunks(content)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    };

    if segments.is_empty() {
        Ok(vec![content.to_string()])
    } else {
        Ok(segments)
    }
}

fn token_chunk_config(target_tokens: usize) -> ChunkConfig<CoreBPE> {
    let tokenizer = cl100k_base().expect("cl100k tokenizer should load");
    ChunkConfig::new(target_tokens.max(1)).with_sizer(tokenizer)
}

fn summary_split_mode(filename: &str) -> SummarySplitMode {
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" => SummarySplitMode::Markdown,
        "rs" => SummarySplitMode::Code(CodeLanguage::Rust),
        "py" => SummarySplitMode::Code(CodeLanguage::Python),
        "js" | "jsx" => SummarySplitMode::Code(CodeLanguage::JavaScript),
        "ts" => SummarySplitMode::Code(CodeLanguage::TypeScript),
        "tsx" => SummarySplitMode::Code(CodeLanguage::Tsx),
        "go" => SummarySplitMode::Code(CodeLanguage::Go),
        "java" => SummarySplitMode::Code(CodeLanguage::Java),
        _ => SummarySplitMode::Text,
    }
}

fn code_splitter(
    language: CodeLanguage,
    config: ChunkConfig<CoreBPE>,
) -> Option<CodeSplitter<CoreBPE>> {
    match language {
        CodeLanguage::Rust => CodeSplitter::new(tree_sitter_rust::LANGUAGE, config).ok(),
        CodeLanguage::Python => CodeSplitter::new(tree_sitter_python::LANGUAGE, config).ok(),
        CodeLanguage::JavaScript => {
            CodeSplitter::new(tree_sitter_javascript::LANGUAGE, config).ok()
        }
        CodeLanguage::TypeScript => {
            CodeSplitter::new(tree_sitter_typescript::LANGUAGE_TYPESCRIPT, config).ok()
        }
        CodeLanguage::Tsx => CodeSplitter::new(tree_sitter_typescript::LANGUAGE_TSX, config).ok(),
        CodeLanguage::Go => CodeSplitter::new(tree_sitter_go::LANGUAGE, config).ok(),
        CodeLanguage::Java => CodeSplitter::new(tree_sitter_java::LANGUAGE, config).ok(),
    }
}

#[cfg(test)]
mod tests;
