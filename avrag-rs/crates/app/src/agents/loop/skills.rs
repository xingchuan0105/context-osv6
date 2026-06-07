use crate::agents::capability::{CapabilityRegistry, SkillMetadata};
use avrag_llm::ChatMessage;

pub struct SkillDisclosure;

impl SkillDisclosure {
    pub fn progressive_disclose(
        &self,
        config: &super::config::DisclosureConfig,
        catalog: &[String],
        registry: &CapabilityRegistry,
        conversation: &[ChatMessage],
        iteration: u8,
        already_disclosed: &[SkillMetadata],
    ) -> Vec<SkillMetadata> {
        let round = config
            .rounds
            .iter()
            .find(|r| r.round_idx == iteration)
            .or_else(|| config.rounds.last());

        let load = match round {
            Some(r) => r.load.clone(),
            None => return vec![],
        };

        let new_skills: Vec<SkillMetadata> = match load {
            super::config::DisclosureLoad::Index => catalog
                .iter()
                .filter_map(|id| registry.skill(id))
                .cloned()
                .collect(),
            super::config::DisclosureLoad::Skills(ids) => ids
                .iter()
                .filter_map(|id| registry.skill(id))
                .cloned()
                .collect(),
            super::config::DisclosureLoad::Auto => {
                let last_user_msg = conversation
                    .iter()
                    .rev()
                    .find(|m| m.role == "user")
                    .map(|m| m.content.as_str())
                    .unwrap_or("");

                let mut matched = Vec::new();
                let lower = last_user_msg.to_lowercase();
                let catalog_set: std::collections::HashSet<&str> = catalog.iter().map(|s| s.as_str()).collect();

                let mut push_if_in_catalog = |id: &str| {
                    if catalog_set.contains(id) {
                        matched.extend(registry.skill(id).cloned());
                    }
                };

                if contains_chat_keywords(&lower) {
                    push_if_in_catalog("chat");
                }
                if contains_pronouns(&lower) {
                    push_if_in_catalog("anaphora-resolution");
                }
                if last_user_msg.len() > 50 || asks_writing_help(&lower) {
                    push_if_in_catalog("tone-guidance");
                }
                if lower.contains("search")
                    || lower.contains("retrieve")
                    || lower.contains("find")
                    || lower.contains("look up")
                {
                    push_if_in_catalog("rag-codegen-guide");
                }
                if lower.contains("search")
                    || lower.contains("retrieve")
                    || lower.contains("dense")
                    || lower.contains("lexical")
                {
                    push_if_in_catalog("rag-retrieval-strategy");
                }
                if lower.contains("cite") || lower.contains("引用") || lower.contains("source") {
                    push_if_in_catalog("rag-citation-format");
                }
                if lower.contains("remember") || lower.contains("recall") || lower.contains("previous")
                {
                    push_if_in_catalog("rag-memory-mgmt");
                }
                if lower.contains("summary")
                    || lower.contains("summarize")
                    || lower.contains("总结")
                {
                    push_if_in_catalog("rag-doc-summary-guide");
                }
                if lower.contains("search")
                    || lower.contains("web")
                    || lower.contains("news")
                    || lower.contains("find")
                {
                    push_if_in_catalog("search-strategy");
                }
                if lower.contains("verify")
                    || lower.contains("validate")
                    || lower.contains("check")
                {
                    push_if_in_catalog("result-validation");
                }
                if lower.contains("cite")
                    || lower.contains("url")
                    || lower.contains("source")
                    || lower.contains("link")
                {
                    push_if_in_catalog("url-citation-format");
                }

                matched
            }
        };

        let already_ids: std::collections::HashSet<_> =
            already_disclosed.iter().map(|s| s.id.as_str()).collect();

        new_skills
            .into_iter()
            .filter(|s| !already_ids.contains(s.id.as_str()))
            .collect()
    }
}

fn contains_chat_keywords(text: &str) -> bool {
    let keywords = ["hello", "hi", "hey", "how are you", "what's up", "help"];
    keywords.iter().any(|&kw| text.contains(kw))
}

fn contains_pronouns(text: &str) -> bool {
    let pronouns = ["it", "that", "this", "he", "she", "they", "them"];
    pronouns.iter().any(|&p| {
        text.split_whitespace()
            .any(|word| word.trim_matches(|c: char| !c.is_alphabetic()) == p)
    })
}

fn asks_writing_help(text: &str) -> bool {
    let phrases = ["write", "draft", "compose", "essay", "letter", "email", "article"];
    phrases.iter().any(|&p| text.contains(p))
}
