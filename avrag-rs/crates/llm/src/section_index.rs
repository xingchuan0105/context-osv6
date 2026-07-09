use crate::client::{ChatMessage, LlmClient};
use anyhow::Context;
use common::{Domain, Era, Genre, SummaryMetadata};
use serde::Deserialize;
use uuid::Uuid;

const DEFAULT_SECTION_INDEX_SYSTEM: &str =
    include_str!("../../../prompts/pipeline/section-index.system.v1.md");
const DEFAULT_SECTION_INDEX_USER: &str =
    include_str!("../../../prompts/templates/section-index-user.tmpl");

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DocumentProfileMetadata {
    pub language: Option<String>,
    pub domain: Option<String>,
    pub genre: Option<String>,
    pub era: Option<String>,
    pub author: Option<String>,
    pub publication_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SectionIndexOutput {
    #[serde(default)]
    pub document_metadata: DocumentProfileMetadata,
    pub sections: Vec<SectionIndexSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SectionIndexSection {
    pub title: String,
    pub heading_level: i32,
    #[serde(default)]
    pub page: Option<i32>,
    #[serde(default)]
    pub rank: i32,
    pub chunk_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SectionIndexChunk {
    pub chunk_id: Uuid,
    pub text: String,
}

pub struct SectionIndexGenerator {
    llm: LlmClient,
    system_prompt: String,
    user_template: String,
}

impl SectionIndexGenerator {
    pub fn new(config: crate::ModelProviderConfig) -> Self {
        Self {
            llm: LlmClient::new(config).with_feature("section_index"),
            system_prompt: DEFAULT_SECTION_INDEX_SYSTEM.to_string(),
            user_template: DEFAULT_SECTION_INDEX_USER.to_string(),
        }
    }

    /// Wrap a preconfigured client (observer / feature already applied).
    pub fn from_client(llm: LlmClient) -> Self {
        Self {
            llm: llm.with_feature("section_index"),
            system_prompt: DEFAULT_SECTION_INDEX_SYSTEM.to_string(),
            user_template: DEFAULT_SECTION_INDEX_USER.to_string(),
        }
    }

    pub fn with_observer(
        mut self,
        observer: std::sync::Arc<dyn crate::UsageObserver>,
        tenant: crate::TenantContext,
    ) -> Self {
        self.llm = self.llm.with_observer(observer, tenant);
        self
    }

    pub fn with_prompts(
        mut self,
        system: impl Into<String>,
        user_template: impl Into<String>,
    ) -> Self {
        self.system_prompt = system.into();
        self.user_template = user_template.into();
        self
    }

    pub async fn generate(
        &self,
        title: &str,
        filename: &str,
        chunks: &[SectionIndexChunk],
    ) -> anyhow::Result<SectionIndexOutput> {
        if chunks.is_empty() {
            return Ok(SectionIndexOutput {
                document_metadata: DocumentProfileMetadata::default(),
                sections: vec![],
            });
        }

        let chunk_ids: Vec<String> = chunks.iter().map(|c| c.chunk_id.to_string()).collect();
        let mut chunks_map = serde_json::Map::new();
        for chunk in chunks {
            let preview = if chunk.text.len() > 1200 {
                let end = chunk.text.floor_char_boundary(1200);
                format!("{}…", &chunk.text[..end])
            } else {
                chunk.text.clone()
            };
            chunks_map.insert(
                chunk.chunk_id.to_string(),
                serde_json::Value::String(preview),
            );
        }
        let chunks_json = serde_json::to_string_pretty(&chunks_map)
            .context("serialize chunks for section index")?;

        let user = self
            .user_template
            .replace("{title}", title)
            .replace("{filename}", filename)
            .replace("{chunk_ids}", &chunk_ids.join(", "))
            .replace("{chunks_json}", &chunks_json);

        let messages = vec![
            ChatMessage::system(&self.system_prompt),
            ChatMessage::user(user),
        ];
        let response = self.llm.complete(&messages, Some(0.1)).await?;
        parse_section_index_response(&response.content, &chunk_ids)
    }
}

pub fn build_profile_metadata(
    doc_id: &str,
    title: &str,
    filename: &str,
    metadata: &DocumentProfileMetadata,
) -> SummaryMetadata {
    SummaryMetadata {
        doc_id: doc_id.to_string(),
        filename: filename.to_string(),
        docname: title.to_string(),
        language: metadata
            .language
            .as_deref()
            .map(normalize_metadata_field)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
        domain: metadata
            .domain
            .as_deref()
            .map(normalize_metadata_field)
            .filter(|value| !value.is_empty())
            .map(|value| Domain::from(value.as_str()))
            .unwrap_or(Domain::Unknown),
        genre: metadata
            .genre
            .as_deref()
            .map(normalize_metadata_field)
            .filter(|value| !value.is_empty())
            .map(|value| Genre::from(value.as_str()))
            .unwrap_or(Genre::Unknown),
        era: metadata
            .era
            .as_deref()
            .map(normalize_metadata_field)
            .filter(|value| !value.is_empty())
            .map(|value| Era::from(value.as_str()))
            .unwrap_or(Era::Unknown),
        author: metadata
            .author
            .as_deref()
            .map(normalize_metadata_field)
            .filter(|value| !value.is_empty()),
        publication_date: metadata
            .publication_date
            .as_deref()
            .map(normalize_metadata_field)
            .filter(|value| !value.is_empty()),
    }
}

fn normalize_metadata_field(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

pub fn parse_section_index_response(
    content: &str,
    valid_chunk_ids: &[String],
) -> anyhow::Result<SectionIndexOutput> {
    let trimmed = content.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .with_context(|| format!("section index response is not valid JSON: {trimmed}"))?;
    let mut output: SectionIndexOutput =
        serde_json::from_value(value).context("section index JSON does not match schema")?;

    let valid: std::collections::HashSet<&str> =
        valid_chunk_ids.iter().map(String::as_str).collect();
    for section in &mut output.sections {
        section.chunk_ids.retain(|id| valid.contains(id.as_str()));
    }
    output
        .sections
        .retain(|s| !s.chunk_ids.is_empty() && !s.title.trim().is_empty());
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_preview_truncation_respects_utf8_boundaries() {
        let em_dash = "—";
        let text = format!("{}{}", "x".repeat(1198), em_dash);
        assert!(text.len() > 1200);
        let end = text.floor_char_boundary(1200);
        let _preview = format!("{}…", &text[..end]);
        assert!(end <= 1200);
        assert!(text.is_char_boundary(end));
    }

    #[test]
    fn parse_section_index_filters_invalid_chunk_ids() {
        let raw = r#"{"document_metadata":{"language":"zh"},"sections":[{"title":"Intro","heading_level":1,"rank":0,"chunk_ids":["a","b"]}]}"#;
        let out = parse_section_index_response(raw, &["a".to_string()]).unwrap();
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].chunk_ids, vec!["a"]);
        assert_eq!(out.document_metadata.language.as_deref(), Some("zh"));
    }

    #[test]
    fn build_profile_metadata_maps_enums() {
        let metadata = DocumentProfileMetadata {
            language: Some("zh".to_string()),
            domain: Some("technology".to_string()),
            genre: Some("thesis".to_string()),
            era: Some("contemporary".to_string()),
            author: Some("Alice".to_string()),
            publication_date: Some("2021".to_string()),
        };
        let profile = build_profile_metadata("doc-1", "Title", "file.txt", &metadata);
        assert_eq!(profile.doc_id, "doc-1");
        assert_eq!(profile.language, "zh");
        assert_eq!(profile.domain, Domain::Technology);
        assert_eq!(profile.genre, Genre::Thesis);
        assert_eq!(profile.author.as_deref(), Some("Alice"));
    }
}
