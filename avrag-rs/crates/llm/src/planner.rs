use crate::ModelProviderConfig;
use crate::client::{ChatMessage, LlmClient, LlmUsage};
use anyhow::Context;
use common::RagPlan;
// Prompts are externalized to avrag-rs/prompts/ for version control and tuning.
const PLANNER_SYSTEM_PROMPT: &str = include_str!("../../../prompts/rag_planner_system.txt");

pub struct RetrievalPlanner {
    llm: LlmClient,
}

fn build_planner_system_prompt() -> String {
    PLANNER_SYSTEM_PROMPT.to_string()
}

fn build_planner_user_prompt(
    query: &str,
    session_context: Option<&str>,
    docscope: Option<&common::DocScopeMetadata>,
) -> String {
    let mut prompt = String::new();

    if let Some(ds) = docscope {
        prompt.push_str("Docscope and document metadata:\n");
        prompt.push_str("DocScope Profile:\n");
        prompt.push_str(&format!("- Languages: {:?}\n", ds.profile.languages));
        prompt.push_str(&format!("- Domains: {:?}\n", ds.profile.domains));
        prompt.push_str(&format!("- Genres: {:?}\n", ds.profile.genres));
        prompt.push_str(&format!("- Eras: {:?}\n", ds.profile.eras));
        prompt.push_str("\nDocuments in Scope:\n");
        for doc in &ds.documents {
            prompt.push_str(&format!(
                "- {} (ID: {}, File: {}, Lang: {}, Domain: {}, Genre: {}, Era: {})\n",
                doc.docname, doc.doc_id, doc.filename, doc.language, doc.domain, doc.genre, doc.era
            ));
        }
        prompt.push('\n');
    }

    if let Some(ctx) = session_context {
        prompt.push_str("Session conversation history:\n");
        prompt.push_str(ctx);
        prompt.push_str("\n\n");
    }

    prompt.push_str("Latest user request:\n");
    prompt.push_str(query);
    prompt
}

impl RetrievalPlanner {
    pub fn new(intent_config: ModelProviderConfig) -> Self {
        Self {
            llm: LlmClient::new(intent_config),
        }
    }

    /// Plan retrieval items for a query using AGENT_LLM
    pub async fn plan(
        &self,
        query: &str,
        session_context: Option<&str>,
        docscope: Option<&common::DocScopeMetadata>,
    ) -> anyhow::Result<RagPlan> {
        let (plan, _) = self
            .plan_with_usage(query, session_context, docscope)
            .await?;
        Ok(plan)
    }

    pub async fn plan_with_usage(
        &self,
        query: &str,
        session_context: Option<&str>,
        docscope: Option<&common::DocScopeMetadata>,
    ) -> anyhow::Result<(RagPlan, LlmUsage)> {
        let mut messages = vec![ChatMessage::system(build_planner_system_prompt())];
        messages.push(ChatMessage::user(build_planner_user_prompt(
            query,
            session_context,
            docscope,
        )));

        let response = self
            .llm
            .complete(&messages, Some(0.3))
            .await
            .context("Failed to get planner response")?;

        let plan: RagPlan = serde_json::from_str(&response.content).with_context(|| {
            format!(
                "Failed to parse RagPlan from LLM response: {}",
                response.content
            )
        })?;

        Ok((plan, response.usage))
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_docscope() -> common::DocScopeMetadata {
        common::DocScopeMetadata {
            documents: vec![common::SummaryMetadata {
                doc_id: "doc-1".to_string(),
                filename: "atlas-plan.md".to_string(),
                docname: "Atlas Plan".to_string(),
                language: "zh".to_string(),
                domain: "technology".to_string(),
                genre: "manual".to_string(),
                era: "contemporary".to_string(),
            }],
            profile: common::DocScopeProfile {
                languages: vec!["zh".to_string()],
                domains: vec!["technology".to_string()],
                genres: vec!["manual".to_string()],
                eras: vec!["contemporary".to_string()],
            },
        }
    }

    #[test]
    fn planner_system_prompt_keeps_new_schema_constraints() {
        let prompt = build_planner_system_prompt();
        assert!(prompt.contains("RAG retrieval planner"));
    }

    #[test]
    fn planner_user_prompt_injects_docscope_metadata_index() {
        let prompt = build_planner_user_prompt("定位 Atlas", None, Some(&sample_docscope()));

        assert!(prompt.contains("Docscope and document metadata"));
        assert!(prompt.contains("- Languages: [\"zh\"]"));
        assert!(prompt.contains("Atlas Plan"));
        assert!(prompt.contains("atlas-plan.md"));
        assert!(prompt.contains("Domain: technology"));
    }

    #[test]
    fn planner_user_prompt_includes_session_context_when_present() {
        let prompt = build_planner_user_prompt(
            "how to roll back?",
            Some("Conversation summary"),
            Some(&sample_docscope()),
        );

        assert!(prompt.contains("Session conversation history:\nConversation summary"));
        assert!(prompt.contains("Latest user request:\nhow to roll back?"));
        assert!(prompt.contains("Docscope and document metadata"));
    }

    #[test]
    fn planner_user_prompt_omits_session_header_when_absent() {
        let prompt = build_planner_user_prompt("how to roll back?", None, None);

        assert!(!prompt.contains("Session conversation history:"));
        assert_eq!(prompt, "Latest user request:\nhow to roll back?");
    }

}
