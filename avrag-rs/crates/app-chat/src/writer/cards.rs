//! Material card extraction from research worker results (spec §6.3, §7).

use std::collections::BTreeSet;

use avrag_guardrails::GuardPipeline;
use contracts::chat::{Citation, DegradeReason, DegradeTraceItem};
use heavytail::segment::char_len;
use heavytail::skeleton::{MaterialCard, MaterialKind};
use heavytail::tokenize::{is_content_word, tokens};

use crate::agents::untrusted_input::{SanitizedContent, UntrustedInputProcessor};
use crate::agents::AgentKind;

const MAX_CARD_CONTENT_CHARS: usize = 80;
const INJECTION_THRESHOLD: f64 = 0.8;

pub struct CardExtraction {
    pub cards: Vec<MaterialCard>,
    pub degrade_trace: Vec<DegradeTraceItem>,
}

/// Rule-based extraction: one card per citation (MVP spec §7 step 3).
pub fn extract_material_cards(
    result: &crate::agents::runtime::AgentRunResult,
    kind: AgentKind,
    guard: Option<&GuardPipeline>,
    trace_id: Option<&str>,
) -> CardExtraction {
    let mut cards = Vec::new();
    let mut degrade_trace = Vec::new();
    let mut next_id = 1usize;

    for citation in &result.citations {
        let raw_content = citation
            .content
            .as_deref()
            .or(citation.preview.as_deref())
            .unwrap_or("")
            .trim();
        if raw_content.is_empty() {
            continue;
        }

        let is_web = kind == AgentKind::Search || citation.layer.as_deref() == Some("search");
        let content = if is_web {
            match sanitize_web_card_content(raw_content, guard, trace_id) {
                Ok(text) => text,
                Err(reason) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "write:material_cards".into(),
                        reason: DegradeReason::ContentGuard,
                        impact: reason,
                    });
                    continue;
                }
            }
        } else {
            truncate_card_content(raw_content)
        };

        if content.is_empty() {
            continue;
        }

        let rare_terms = extract_rare_terms(&content);
        let card = MaterialCard {
            id: format!("m{next_id:02}"),
            kind: classify_kind(kind, citation),
            content,
            source: serde_json::to_value(citation).unwrap_or_else(|_| serde_json::json!({})),
            section_hint: (!citation.doc_name.is_empty()).then_some(citation.doc_name.clone()),
            rare_terms,
        };
        next_id += 1;
        cards.push(card);
    }

    CardExtraction {
        cards,
        degrade_trace,
    }
}

fn sanitize_web_card_content(
    raw: &str,
    guard: Option<&GuardPipeline>,
    trace_id: Option<&str>,
) -> Result<String, String> {
    if let Some(guard) = guard {
        if let Some(check) = guard.check_content(raw, trace_id.map(str::to_string)) {
            if !check.passed {
                return Err("web card content redacted by content guard".into());
            }
        }
    }

    match UntrustedInputProcessor::sanitize_retrieval(raw, INJECTION_THRESHOLD) {
        SanitizedContent::Safe(_) => Ok(truncate_card_content(raw)),
        SanitizedContent::Rejected { reason } => Err(reason),
    }
}

fn truncate_card_content(raw: &str) -> String {
    let trimmed = raw.trim();
    if char_len(trimmed) <= MAX_CARD_CONTENT_CHARS {
        return trimmed.to_string();
    }
    let mut out = String::new();
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if !ch.is_whitespace() {
            count += 1;
        }
        if count > MAX_CARD_CONTENT_CHARS {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn extract_rare_terms(content: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tokens(content)
        .into_iter()
        .filter(|t| is_content_word(t))
        .filter(|t| seen.insert(t.clone()))
        .take(8)
        .collect()
}

fn classify_kind(kind: AgentKind, citation: &Citation) -> MaterialKind {
    if kind == AgentKind::Search || citation.layer.as_deref() == Some("search") {
        return MaterialKind::Fact;
    }
    MaterialKind::Fact
}

pub fn dedupe_cards(cards: Vec<MaterialCard>) -> Vec<MaterialCard> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for card in cards {
        let key = format!("{}:{}", card.content, card.source);
        if seen.insert(key) {
            out.push(card);
        }
    }
    out
}

pub fn build_reservoir(cards: &[MaterialCard]) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for card in cards {
        for term in &card.rare_terms {
            terms.insert(term.clone());
        }
    }
    terms.into_iter().collect()
}

pub fn citations_from_cards(cards: &[MaterialCard]) -> Vec<Citation> {
    cards
        .iter()
        .filter_map(|card| serde_json::from_value(card.source.clone()).ok())
        .collect()
}

pub fn used_card_ids(skeleton: &heavytail::skeleton::Skeleton) -> BTreeSet<String> {
    skeleton
        .sections
        .iter()
        .flat_map(|section| section.card_refs.iter().cloned())
        .collect()
}

pub fn filter_citations_for_cards(
    all: &[Citation],
    cards: &[MaterialCard],
    skeleton: &heavytail::skeleton::Skeleton,
) -> Vec<Citation> {
    let used = used_card_ids(skeleton);
    if used.is_empty() {
        return citations_from_cards(cards);
    }

    let selected: BTreeSet<String> = cards
        .iter()
        .filter(|c| used.contains(&c.id))
        .filter_map(|c| serde_json::from_value::<Citation>(c.source.clone()).ok())
        .map(|c| citation_key(&c))
        .collect();

    all.iter()
        .filter(|c| selected.contains(&citation_key(c)))
        .cloned()
        .collect()
}

fn citation_key(c: &Citation) -> String {
    format!(
        "{}:{}:{}",
        c.doc_id,
        c.chunk_id.as_deref().unwrap_or(""),
        c.citation_id
    )
}
