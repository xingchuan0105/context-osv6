//! Option B: Chat agent is the sole user-facing answer exit.

use super::invariant::partial_notices_from_packs;
use super::types::{ChatExitMode, ChatHandoff, EvidencePack, PackStatus};

/// Build synthesize handoff from packs (fills partial_notices from §7.3).
pub fn synthesize_handoff(
    user_query: &str,
    packs: Vec<EvidencePack>,
    instruction: Option<String>,
) -> ChatHandoff {
    let mut notices = partial_notices_from_packs(&packs);
    // Human-readable product notices (short)
    for p in &packs {
        match p.status {
            PackStatus::Empty if p.channel == super::types::Channel::Rag => {
                notices.push(
                    "工作区未命中相关段落（已检索）。请基于可用证据作答；勿将未命中归因为用户未粘贴文档。"
                        .into(),
                );
            }
            PackStatus::Empty if p.channel == super::types::Channel::Search => {
                notices.push("网络检索未返回可用结果。".into());
            }
            PackStatus::Error if p.channel == super::types::Channel::Rag => {
                notices.push("工作区检索失败；若有网页证据可仅用网页并说明。".into());
            }
            PackStatus::Error if p.channel == super::types::Channel::Search => {
                notices.push("网络检索失败；若有文档证据可仅用文档并说明。".into());
            }
            _ => {}
        }
    }
    // Dedupe while preserving order
    let mut seen = std::collections::HashSet::new();
    notices.retain(|n| seen.insert(n.clone()));

    ChatHandoff {
        mode: ChatExitMode::Synthesize,
        user_query: user_query.to_string(),
        instruction,
        packs,
        partial_notices: notices,
    }
}

pub fn direct_handoff(user_query: &str) -> ChatHandoff {
    ChatHandoff {
        mode: ChatExitMode::Direct,
        user_query: user_query.to_string(),
        instruction: None,
        packs: vec![],
        partial_notices: vec![],
    }
}

/// System-side instruction block for Chat synthesize (injected into agent query/metadata).
pub fn render_synthesize_context(handoff: &ChatHandoff) -> String {
    let mut s = String::new();
    s.push_str("## Chat synthesize (internal)\n");
    s.push_str(
        "You are the sole user-facing answer agent. Use evidence packs below. \
         Do not claim the user failed to provide a document if a rag pack exists \
         (even when empty — say 未命中). Partial packs: answer from available evidence and state limits.\n\n",
    );
    // Citation marker contract (design §4.3): only for channels with usable items.
    s.push_str(&render_citation_contract(&handoff.packs));
    s.push('\n');
    if let Some(ins) = &handoff.instruction {
        s.push_str("### Orchestrator instruction\n");
        s.push_str(ins);
        s.push_str("\n\n");
    }
    if !handoff.partial_notices.is_empty() {
        s.push_str("### Partial notices\n");
        for n in &handoff.partial_notices {
            s.push_str("- ");
            s.push_str(n);
            s.push('\n');
        }
        s.push('\n');
    }
    s.push_str("### Evidence packs (JSON)\n```json\n");
    s.push_str(
        &serde_json::to_string_pretty(&handoff.packs).unwrap_or_else(|_| "[]".into()),
    );
    s.push_str("\n```\n\n### User question\n");
    s.push_str(&handoff.user_query);
    s
}

/// Citation marker rules for the packs that actually carry items.
///
/// Markers must line up with what the host can rebuild downstream
/// (`attach_worker_evidence`): doc `[[cite:id]]` ↔ rag item `id` (chunk_id),
/// web `[[web:n]]` ↔ 1-based position in the search pack `items` array
/// (matches `citation_index` from `web_search`).
fn render_citation_contract(packs: &[EvidencePack]) -> String {
    use super::types::Channel;
    let has_doc = packs
        .iter()
        .any(|p| p.channel == Channel::Rag && !p.items.is_empty());
    let has_web = packs
        .iter()
        .any(|p| p.channel == Channel::Search && !p.items.is_empty());

    let mut s = String::new();
    s.push_str("### Citation markers (required)\n");
    if !has_doc && !has_web {
        s.push_str("No usable evidence retrieved — do not emit any citation markers.\n");
        return s;
    }
    if has_doc {
        s.push_str(
            "- Workspace document facts: append `[[cite:ID]]` right after the claim, where `ID` \
             is the `id` of an item in the rag pack (copy verbatim). Every doc-grounded claim \
             needs its marker.\n",
        );
    }
    if has_web {
        s.push_str(
            "- Web facts: append `[[web:n]]` right after the claim, where `n` is the 1-based \
             position of the item in the search pack `items` array.\n",
        );
    }
    s.push_str(
        "- Never invent markers for packs that are empty or errored; never use `[[web:n]]` \
         for document facts or `[[cite:...]]` for web facts.\n",
    );
    s
}

/// Augment user query for agent run (synthesize path).
pub fn query_for_agent(handoff: &ChatHandoff) -> String {
    match handoff.mode {
        ChatExitMode::Direct => handoff.user_query.clone(),
        ChatExitMode::Synthesize => render_synthesize_context(handoff),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::{Channel, TaskBrief};

    #[test]
    fn empty_rag_adds_notice() {
        let packs = vec![EvidencePack {
            channel: Channel::Rag,
            status: PackStatus::Empty,
            dispatch_id: "1".into(),
            task_brief: TaskBrief::new("g"),
            items: vec![],
            notes: None,
            error: None,
        }];
        let h = synthesize_handoff("q", packs, None);
        assert!(h.partial_notices.iter().any(|n| n.contains("未命中")));
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("Evidence packs"));
    }

    fn pack_with_item(channel: Channel) -> EvidencePack {
        EvidencePack {
            channel,
            status: PackStatus::Ok,
            dispatch_id: "1".into(),
            task_brief: TaskBrief::new("g"),
            items: vec![super::super::types::EvidenceItem {
                id: "chunk-a".into(),
                title: None,
                text: "evidence".into(),
                score: None,
                uri: None,
            }],
            notes: None,
            error: None,
        }
    }

    #[test]
    fn citation_contract_matches_available_packs() {
        // Dual with items: both marker rules present.
        let h = synthesize_handoff(
            "q",
            vec![pack_with_item(Channel::Rag), pack_with_item(Channel::Search)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("[[cite:ID]]"), "doc rule missing: {ctx}");
        assert!(ctx.contains("[[web:n]]"), "web rule missing: {ctx}");

        // Rag empty: no doc marker rule, web rule stays, no-marker ban present.
        let mut rag_empty = pack_with_item(Channel::Rag);
        rag_empty.status = PackStatus::Empty;
        rag_empty.items = vec![];
        let h = synthesize_handoff("q", vec![rag_empty, pack_with_item(Channel::Search)], None);
        let ctx = render_synthesize_context(&h);
        assert!(!ctx.contains("[[cite:ID]]"));
        assert!(ctx.contains("[[web:n]]"));
        assert!(ctx.contains("Never invent markers"));

        // No usable evidence at all: explicit no-marker instruction.
        let mut search_empty = pack_with_item(Channel::Search);
        search_empty.status = PackStatus::Empty;
        search_empty.items = vec![];
        let h = synthesize_handoff("q", vec![search_empty], None);
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("do not emit any citation markers"));
    }
}
