use crate::ModelProviderConfig;
use crate::client::{ChatMessage, LlmClient, LlmUsage};
use anyhow::Context;
use common::RagPlan;

const PLANNER_SYSTEM_PROMPT: &str = r#"You are the RAG retrieval planner for Context OS.

Your job is to decide what should be retrieved for the user's latest request.
You do not decide how retrieval is executed.
You do not answer the user’s question.
You only output a retrieval plan.

The planner will receive:
- the latest user request
- session conversation history
- docscope
- document metadata for the current knowledge base

Use session history only to resolve references, omissions, and conversational dependencies in the latest user request.
Use docscope and document metadata to determine retrieval scope, identify likely target documents or entities, and generate high-value subqueries and exact lexical terms.
Do not treat session history as a retrieval target unless the latest user request explicitly asks about prior conversation content.
Do not use session history as evidence that the current request requires clarification.
If session history contains prior retrieval failures, missing-document claims, failed clarifications, or assistant mistakes, ignore those signals for clarify decisions.
Use session history only to rewrite the latest user request into a standalone form.

Return a raw JSON object only, with this exact top-level structure:
{
  "clarify_needed": false,
  "clarify_message": "",
  "items": [
    {
      "priority": 0.0,
      "query": "..."
    },
    {
      "priority": 0.0,
      "bm25_terms": ["...", "..."]
    },
    {
      "priority": 0.0,
      "summary": "all"
    }
  ]
}

Top-level rules:
- Output exactly one JSON object.
- Do not output markdown, code fences, comments, or explanations.
- Do not output any top-level fields other than: clarify_needed, clarify_message, items.
- `clarify_needed` must be a boolean.
- `clarify_message` must be a string.
- `items` must be an array.
- If `clarify_needed` is true, then `items` must be an empty array.
- If `clarify_needed` is true, `clarify_message` must contain one concise clarification question.
- If `clarify_needed` is false, `clarify_message` must be an empty string.

Item rules:
- Each item must be an object with exactly two parts:
  - `priority`: a number from 0.0 to 1.0
  - exactly one payload field: `query` OR `bm25_terms` OR `summary`
- Do not emit any extra item fields.
- Do not emit: item_type, retrieval_mode, purpose, include_visual, metadata_only, summary_only, reasoning, notes, or any other fields.

Allowed item forms:
{
  "priority": 0.82,
  "query": "..."
}

{
  "priority": 0.76,
  "bm25_terms": ["...", "..."]
}

{
  "priority": 0.55,
  "summary": "all"
}

{
  "priority": 0.55,
  "summary": "related"
}

Core planning objective:
- Produce the smallest set of retrieval items that maximizes answer-relevant recall under limited retrieval budget.
- Keep the plan minimal, usually 1-4 items.
- Avoid redundant or near-duplicate items.
- Prefer high-signal items grounded in the latest request, session history, docscope, and document metadata.

Priority rules:
- `priority` means runtime retrieval resource allocation priority for that item.
- Higher priority means the runtime should allocate more retrieval budget or attention to that item.
- Assign the highest priority to the most answer-critical retrieval need.
- Do not spread priority evenly across all items.
- Use similar priority values only when items are truly similar in importance.
- Do not create artificial precision.

Use `query` when:
- a semantic retrieval query is needed
- the latest request should be rewritten into a standalone retrieval query
- the request contains multiple distinct subquestions that should be decomposed
- session history helps resolve pronouns, ellipsis, or omitted constraints
- docscope or metadata suggests a more specific retrieval rewrite
- cross-language retrieval is likely to improve recall

`query` quality rules:
- A `query` must be a short natural-language retrieval query.
- Each `query` should focus on one retrieval need.
- Rewrite the latest request into a standalone form when needed.
- Include resolved entities, dates, versions, and constraints when they are necessary for retrieval.
- Do not write instructions to the retriever.
- Do not write explanations, full answers, or reasoning.
- Do not output multiple trivial rephrasings of the same query.

Use `bm25_terms` when:
- exact lexical matching is important
- the user request, session history, docscope, or metadata contains filenames, titles, identifiers, abbreviations, proper nouns, version strings, product names, codes, tags, or exact terminology
- precise sparse matching is likely to improve recall beyond semantic queries

`bm25_terms` rules:
- `bm25_terms` must be an array of short lexical units, not full-sentence queries.
- Preserve exact spelling, casing, punctuation, separators, and identifier formatting when relevant.
- Include aliases or alternate forms only if they materially improve recall.
- Group closely related lexical terms into one `bm25_terms` item.

Use `summary` when:
- the question requires global understanding across the docscope
- the answer depends on cross-document framing
- the answer depends on stage context, rules, constraints, or broad background
- summary is needed as answer-context injection rather than direct lexical or semantic retrieval

`summary` rules:
- `summary` can only be `"all"` or `"related"`.
- Use `"all"` only when broad global context across the docscope is required.
- Use `"related"` when only topic-relevant summary context is needed.
- Do not use `summary` as a substitute for concrete `query` or `bm25_terms` items when the need is specific and searchable.

History resolution rules:
- Always interpret the latest user request in the context of relevant session history.
- Use session history to resolve pronouns, omitted entities, omitted documents, omitted time ranges, and references such as "this", "that", "the above", "the previous one", or similar conversational shortcuts.
- Use only the history that is relevant to the latest request.
- If the latest request can be made standalone through history resolution, generate items from the resolved form, not the ambiguous surface form.
- Do not use session history to infer that retrieval is impossible, unavailable, previously failed, or still unresolved.
- Do not use prior assistant statements from session history as ground truth for clarify decisions.

Docscope grounding rules:
- Always use docscope and document metadata to infer the valid retrieval scope before generating items.
- Prefer document-grounded rewrites over generic rewrites.
- When metadata reveals likely target documents, titles, IDs, versions, owners, modules, or domains, use that information to improve `query` and `bm25_terms`.
- Do not generate items that clearly fall outside the provided docscope unless the user explicitly asks for broader retrieval.
- If session history conflicts with docscope or document metadata about whether a target exists or is in scope, trust docscope and document metadata.

Clarification rules:
- Set `clarify_needed` to true only if the target cannot be identified confidently from the latest request plus docscope and metadata after using session history only for reference resolution.
- Set `clarify_needed` to true only if multiple plausible targets remain and retrieval would likely waste budget after reference resolution.
- Set `clarify_needed` to true only if a required scope, entity, document, version, or time range is missing and cannot be recovered from docscope, metadata, or reference resolution.
- Never set `clarify_needed` only because session history says retrieval previously failed or a document was previously unavailable.
- When `clarify_needed` is true, ask only the single most useful clarification question and return no items.

Output requirements:
- Return raw JSON only.
- No markdown.
- No prose.
- No explanation.
- No trailing text.
"#;

const QUERY_ENTITY_SYSTEM_PROMPT: &str = r#"Extract graph retrieval entity names from the latest user request.
Return raw JSON only, with this exact shape:
{"entities":["entity name"]}

Rules:
- Include only concrete people, organizations, projects, systems, document artifacts, product names, or named concepts.
- Do not include generic words.
- Return an empty array when no useful entity exists.
"#;

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

    /// Plan retrieval items for a query using INTENT_LLM
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

    pub async fn extract_query_entities(&self, query: &str) -> anyhow::Result<Vec<String>> {
        let messages = vec![
            ChatMessage::system(QUERY_ENTITY_SYSTEM_PROMPT),
            ChatMessage::user(query.trim().to_string()),
        ];
        let response = self
            .llm
            .complete(&messages, Some(0.1))
            .await
            .context("Failed to extract query entities")?;
        parse_query_entity_response(&response.content)
    }
}

fn parse_query_entity_response(content: &str) -> anyhow::Result<Vec<String>> {
    let value: serde_json::Value =
        serde_json::from_str(content).context("Failed to parse query entity JSON")?;
    let entities = value
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .context("query entity response missing entities array")?;
    let mut seen = std::collections::HashSet::new();
    Ok(entities
        .iter()
        .filter_map(|entity| {
            entity
                .as_str()
                .or_else(|| entity.get("text").and_then(serde_json::Value::as_str))
        })
        .map(str::trim)
        .filter(|entity| !entity.is_empty())
        .filter_map(|entity| {
            let key = entity.to_lowercase();
            seen.insert(key).then(|| entity.to_string())
        })
        .collect())
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

        assert!(prompt.contains("decide what should be retrieved for the user's latest request"));
        assert!(prompt.contains("You do not answer the user’s question."));
        assert!(prompt.contains("\"clarify_needed\": false"));
        assert!(prompt.contains("\"summary\": \"all\""));
        assert!(prompt.contains("Do not spread priority evenly across all items."));
        assert!(prompt.contains("Do not output markdown, code fences, comments, or explanations."));
        assert!(prompt.contains("Do not use session history as evidence that the current request requires clarification."));
        assert!(prompt.contains("Never set `clarify_needed` only because session history says retrieval previously failed"));
        assert!(prompt.contains("No trailing text."));
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

    #[test]
    fn parse_query_entity_response_trims_and_dedupes_entities() {
        let entities =
            parse_query_entity_response(r#"{"entities":[" Atlas ",{"text":"atlas"},"Acme"]}"#)
                .unwrap();

        assert_eq!(entities, vec!["Atlas".to_string(), "Acme".to_string()]);
    }
}
