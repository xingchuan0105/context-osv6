//! `MaterialPack` — the background appendix assembled from `ResearchOutcome` and
//! incremental in-loop research, re-injected into every refine-loop user prompt.
//!
//! See `docs/plans/2026-07-07-write-refine-agent-loop.md` §5.1.

use contracts::chat::Citation;
use heavytail::skeleton::MaterialCard;

/// Research materials fed into MaterialPack (decoupled from app-chat invoker).
#[derive(Debug, Clone, Default)]
pub struct ResearchMaterials {
    pub cards: Vec<MaterialCard>,
    pub citations: Vec<Citation>,
    pub reservoir: Vec<String>,
}


/// Compact view of a single `MaterialCard` for prompt rendering.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MaterialCardView {
    pub id: String,
    pub kind: String,
    /// ≤80 字摘要。
    pub content: String,
    /// 「用户文档：…」/ 「网络：…」
    pub source_label: String,
    pub rare_terms: Vec<String>,
    /// Whether the card's content words already appear in the live draft.
    pub used_in_draft: bool,
}

/// The background appendix attached to every refine-loop user prompt.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MaterialPack {
    pub rag_cards: Vec<MaterialCardView>,
    pub web_cards: Vec<MaterialCardView>,
    pub reservoir: Vec<String>,
    pub citation_index: Vec<String>,
}

impl MaterialPack {
    /// Reservoir-only pack for experiment harnesses without research cards.
    pub fn with_reservoir(reservoir: Vec<String>) -> Self {
        Self {
            reservoir,
            ..Default::default()
        }
    }

    /// Build the initial pack from research materials (cards + citations + reservoir).
    pub fn from_research(materials: &ResearchMaterials, workspace_text: &str) -> Self {
        let mut rag_cards = Vec::new();
        let mut web_cards = Vec::new();
        for card in &materials.cards {
            let view = card_to_view(card, workspace_text);
            // Heuristic split: cards whose source is a web citation go to
            // `web_cards`; everything else is treated as RAG. The orchestrator
            // does not tag cards by channel today, so we bucket by source kind
            // when available, defaulting to RAG.
            if is_web_card(card) {
                web_cards.push(view);
            } else {
                rag_cards.push(view);
            }
        }
        Self {
            rag_cards,
            web_cards,
            reservoir: materials.reservoir.clone(),
            citation_index: materials
                .citations
                .iter()
                .map(|c| {
                    if c.doc_name.is_empty() {
                        format!("[{}] {}", c.citation_id, c.doc_id)
                    } else {
                        format!("[{}] {}", c.citation_id, c.doc_name)
                    }
                })
                .collect(),
        }
    }

    /// Merge up to `limit` new cards produced by an in-loop `write_refine_research` call.
    ///
    /// Returns the views that were **actually inserted** (deduped against the
    /// existing pack), so the caller can report precisely which cards this
    /// research call contributed — not a tail-slice of the whole pack.
    pub fn merge_new_cards(
        &mut self,
        new_cards: Vec<MaterialCard>,
        workspace_text: &str,
        limit: usize,
    ) -> Vec<MaterialCardView> {
        let mut inserted = Vec::new();
        for card in new_cards.into_iter().take(limit) {
            let view = card_to_view(&card, workspace_text);
            let pushed = if is_web_card(&card) {
                if !self.web_cards.iter().any(|c| c.id == view.id) {
                    self.web_cards.push(view.clone());
                    true
                } else {
                    false
                }
            } else if !self.rag_cards.iter().any(|c| c.id == view.id) {
                self.rag_cards.push(view.clone());
                true
            } else {
                false
            };
            if pushed {
                inserted.push(view);
            }
            for term in &card.rare_terms {
                if !self.reservoir.iter().any(|r| r == term) {
                    self.reservoir.push(term.clone());
                }
            }
        }
        inserted
    }

    /// Render the appendix as a Chinese-language prompt section.
    pub fn render_appendix_zh(&self) -> String {
        let mut out = String::from("## 背景资料附录\n\n");
        if !self.rag_cards.is_empty() {
            out.push_str("### 用户文档卡片\n");
            for c in &self.rag_cards {
                out.push_str(&format!(
                    "- **{id}** [{kind}] {content}（{label}）{used}\n",
                    id = c.id,
                    kind = c.kind,
                    content = c.content,
                    label = c.source_label,
                    used = if c.used_in_draft {
                        "（已用于正文）"
                    } else {
                        ""
                    }
                ));
                if !c.rare_terms.is_empty() {
                    out.push_str(&format!("  - 术语：{}\n", c.rare_terms.join("、")));
                }
            }
            out.push('\n');
        }
        if !self.web_cards.is_empty() {
            out.push_str("### 网络证据卡片\n");
            for c in &self.web_cards {
                out.push_str(&format!(
                    "- **{id}** [{kind}] {content}（{label}）{used}\n",
                    id = c.id,
                    kind = c.kind,
                    content = c.content,
                    label = c.source_label,
                    used = if c.used_in_draft {
                        "（已用于正文）"
                    } else {
                        ""
                    }
                ));
            }
            out.push('\n');
        }
        if !self.reservoir.is_empty() {
            out.push_str("### 可复用素材词\n");
            out.push_str(&self.reservoir.join("、"));
            out.push_str("\n\n");
        }
        out
    }
}

fn card_to_view(card: &MaterialCard, workspace_text: &str) -> MaterialCardView {
    let content = truncate_chars(&card.content, 80);
    let used_in_draft = !card.rare_terms.is_empty()
        && card
            .rare_terms
            .iter()
            .filter(|t| t.chars().count() >= 2)
            .any(|t| workspace_text.contains(t.as_str()));
    MaterialCardView {
        id: card.id.clone(),
        kind: format!("{:?}", card.kind).to_lowercase(),
        content,
        source_label: source_label(card),
        rare_terms: card.rare_terms.clone(),
        used_in_draft,
    }
}

fn source_label(card: &MaterialCard) -> String {
    if card.source.is_null() {
        return "未知来源".to_string();
    }
    if let Some(label) = card.source.get("label").and_then(|v| v.as_str()) {
        return format!("来源：{label}");
    }
    if let Some(url) = card.source.get("url").and_then(|v| v.as_str()) {
        return format!("网络：{url}");
    }
    if let Some(title) = card.source.get("title").and_then(|v| v.as_str()) {
        return format!("文档：{title}");
    }
    "未知来源".to_string()
}

fn is_web_card(card: &MaterialCard) -> bool {
    card.source.get("url").is_some()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::chat::Citation;
    use heavytail::skeleton::{MaterialCard, MaterialKind};

    fn make_card(id: &str, content: &str, terms: Vec<&str>, url: Option<&str>) -> MaterialCard {
        let source = if let Some(u) = url {
            serde_json::json!({ "url": u, "label": u })
        } else {
            serde_json::json!({ "title": "doc.pdf", "label": "doc.pdf" })
        };
        MaterialCard {
            id: id.to_string(),
            kind: MaterialKind::Fact,
            content: content.to_string(),
            source,
            section_hint: None,
            rare_terms: terms.into_iter().map(String::from).collect(),
        }
    }

    fn empty_materials(cards: Vec<MaterialCard>, reservoir: Vec<&str>) -> ResearchMaterials {
        ResearchMaterials {
            cards,
            citations: vec![Citation {
                citation_id: 1,
                doc_id: "doc-1".to_string(),
                doc_name: "doc.pdf".to_string(),
                score: 0.9,
                chunk_id: None,
                page: None,
                preview: None,
                content: None,
                layer: None,
                chunk_type: None,
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }],
            reservoir: reservoir.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn from_research_buckets_rag_and_web() {
        let rag = make_card("m01", "RAG fact about Rust ownership", vec!["Rust"], None);
        let web = make_card(
            "m02",
            "Web news about Rust 1.75",
            vec!["Rust"],
            Some("https://example.com/news"),
        );
        let outcome = empty_materials(vec![rag, web], vec!["Rust"]);
        let pack = MaterialPack::from_research(&outcome, "Rust is great");
        assert_eq!(pack.rag_cards.len(), 1);
        assert_eq!(pack.web_cards.len(), 1);
        assert_eq!(pack.reservoir, vec!["Rust".to_string()]);
        assert_eq!(pack.citation_index.len(), 1);
    }

    #[test]
    fn render_appendix_has_sections() {
        let rag = make_card("m01", "RAG fact", vec!["Rust"], None);
        let outcome = empty_materials(vec![rag], vec!["Rust"]);
        let pack = MaterialPack::from_research(&outcome, "Rust is great");
        let rendered = pack.render_appendix_zh();
        assert!(rendered.contains("## 背景资料附录"));
        assert!(rendered.contains("### 用户文档卡片"));
        assert!(rendered.contains("**m01**"));
        assert!(rendered.contains("### 可复用素材词"));
        assert!(pack.rag_cards[0].used_in_draft);
    }

    #[test]
    fn merge_new_cards_dedupes_and_extends_reservoir() {
        let rag = make_card("m01", "RAG fact", vec!["Rust"], None);
        let outcome = empty_materials(vec![rag], vec!["Rust"]);
        let mut pack = MaterialPack::from_research(&outcome, "Rust is great");
        let new = make_card("m02", "Second fact", vec!["Tokio"], None);
        let inserted = pack.merge_new_cards(vec![new], "Tokio runtime", 3);
        assert_eq!(pack.rag_cards.len(), 2);
        assert!(pack.reservoir.contains(&"Tokio".to_string()));
        // P1.2: merge returns exactly the newly inserted views.
        assert_eq!(inserted.len(), 1);
        assert_eq!(inserted[0].id, "m02");
        let dup = make_card("m01", "RAG fact", vec!["Rust"], None);
        let inserted_dup = pack.merge_new_cards(vec![dup], "Rust is great", 3);
        assert_eq!(pack.rag_cards.len(), 2);
        // A duplicate contributes no new view.
        assert!(inserted_dup.is_empty());
    }

    #[test]
    fn merge_respects_limit() {
        let outcome = empty_materials(vec![], vec![]);
        let mut pack = MaterialPack::from_research(&outcome, "");
        let new: Vec<MaterialCard> = (0..5)
            .map(|i| make_card(&format!("m{i:02}"), "fact", vec!["term"], None))
            .collect();
        pack.merge_new_cards(new, "", 3);
        assert_eq!(pack.rag_cards.len(), 3);
    }
}