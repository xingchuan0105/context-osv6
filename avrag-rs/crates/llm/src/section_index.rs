use crate::client::{ChatMessage, LlmClient};
use anyhow::Context;
use serde::Deserialize;
use uuid::Uuid;

const DEFAULT_SECTION_INDEX_SYSTEM: &str =
    include_str!("../../../prompts/pipeline/section-index.system.v1.md");
const DEFAULT_SECTION_INDEX_USER: &str =
    include_str!("../../../prompts/templates/section-index-user.tmpl");

#[derive(Debug, Clone, Deserialize)]
pub struct SectionIndexOutput {
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
            llm: LlmClient::new(config),
            system_prompt: DEFAULT_SECTION_INDEX_SYSTEM.to_string(),
            user_template: DEFAULT_SECTION_INDEX_USER.to_string(),
        }
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
            return Ok(SectionIndexOutput { sections: vec![] });
        }

        let chunk_ids: Vec<String> = chunks.iter().map(|c| c.chunk_id.to_string()).collect();
        let mut chunks_map = serde_json::Map::new();
        for chunk in chunks {
            let preview = if chunk.text.len() > 1200 {
                format!("{}…", &chunk.text[..1200])
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
    fn parse_section_index_filters_invalid_chunk_ids() {
        let raw =
            r#"{"sections":[{"title":"Intro","heading_level":1,"rank":0,"chunk_ids":["a","b"]}]}"#;
        let out = parse_section_index_response(raw, &["a".to_string()]).unwrap();
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].chunk_ids, vec!["a"]);
    }
}
