//! Option B: Chat agent is the sole user-facing answer exit.
//!
//! The synthesize brief carries evidence **by reference** (store `E{n}`
//! listings) plus worker digests and source-document identity — never raw
//! chunk dumps (design §3.5). The chat cites `[[E:id]]`; the host rewrites
//! those to product markers after the run (`workers::finalize_answer_evidence`).

use super::invariant::partial_notices_from_records;
use super::store::{EvidenceListing, SourceDoc};
use super::types::{Channel, ChannelNote, ChatExitMode, ChatHandoff, DispatchRecord, PackStatus};

/// Build synthesize handoff from the evidence store + dispatch ledger.
pub fn synthesize_handoff(
    user_query: &str,
    source_docs: Vec<SourceDoc>,
    listings: Vec<EvidenceListing>,
    channel_notes: Vec<ChannelNote>,
    records: &[DispatchRecord],
    instruction: Option<String>,
) -> ChatHandoff {
    let mut notices = partial_notices_from_records(records);
    // Human-readable product notices (short)
    for note in &channel_notes {
        match note.status {
            PackStatus::Empty if note.channel == Channel::Rag => {
                notices.push(
                    "工作区未命中相关段落（已检索）。请基于可用证据作答；勿将未命中归因为用户未粘贴文档。"
                        .into(),
                );
            }
            PackStatus::Empty if note.channel == Channel::Search => {
                notices.push(
                    "网络检索未返回可用结果。网页侧内容只能表述为未检索到，禁止给出网页编号引用。"
                        .into(),
                );
            }
            PackStatus::Error if note.channel == Channel::Rag => {
                notices.push("工作区检索失败；若有网页证据可仅用网页并说明。".into());
            }
            PackStatus::Error if note.channel == Channel::Search => {
                notices.push(
                    "网络检索失败；网页侧内容只能表述为未检索到，禁止给出网页编号引用。".into(),
                );
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
        source_docs,
        listings,
        channel_notes,
        partial_notices: notices,
    }
}

pub fn direct_handoff(user_query: &str) -> ChatHandoff {
    ChatHandoff {
        mode: ChatExitMode::Direct,
        user_query: user_query.to_string(),
        instruction: None,
        source_docs: vec![],
        listings: vec![],
        channel_notes: vec![],
        partial_notices: vec![],
    }
}

/// System-side instruction block for Chat synthesize (injected into agent query).
pub fn render_synthesize_context(handoff: &ChatHandoff) -> String {
    let mut s = String::new();
    s.push_str("## Chat synthesize (internal)\n");
    s.push_str(
        "You are the sole user-facing answer agent. Answer from the evidence below. \
         If the question is ambiguous, state your reading of it in one short sentence first \
         (理解口径). Do not claim the user failed to provide a document when workspace \
         retrieval ran — say 未命中 instead. Use the same language as the user.\n\n",
    );

    // Source-document identity: genre judgment must not be guessed from snippets.
    if !handoff.source_docs.is_empty() {
        s.push_str("### Source documents\n");
        for doc in &handoff.source_docs {
            match &doc.genre {
                Some(g) => s.push_str(&format!("- 《{}》(genre: {})\n", doc.file_name, g)),
                None => s.push_str(&format!("- 《{}》\n", doc.file_name)),
            }
        }
        s.push('\n');
    }

    // Channel outcomes: what ran and what came back (worker digests).
    if !handoff.channel_notes.is_empty() {
        s.push_str("### Channel outcomes\n");
        for note in &handoff.channel_notes {
            let status = match note.status {
                PackStatus::Ok => format!("ok, {} evidence items", note.item_count),
                PackStatus::Empty => "empty (ran, nothing usable)".to_string(),
                PackStatus::Error => format!(
                    "error ({})",
                    note.error.as_deref().unwrap_or("unknown")
                ),
            };
            s.push_str(&format!("- {}: {}\n", note.channel.as_str(), status));
            if let Some(n) = note.note.as_deref().filter(|n| !n.trim().is_empty()) {
                s.push_str("  worker digest: ");
                s.push_str(n);
                s.push('\n');
            }
        }
        s.push('\n');
    }

    // Evidence listings (references only).
    if !handoff.listings.is_empty() {
        s.push_str("### Evidence (cite by id)\n");
        for l in &handoff.listings {
            s.push_str(&format!(
                "- [{}] {} | {}\n",
                l.eid,
                l.label,
                l.preview.trim()
            ));
        }
        s.push('\n');
    }

    s.push_str(&render_citation_contract(handoff));
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
    s.push_str("### User question\n");
    s.push_str(&handoff.user_query);
    s
}

/// Citation marker rules: `[[E:id]]` only, only for listed ids, only for
/// channels that actually returned evidence.
fn render_citation_contract(handoff: &ChatHandoff) -> String {
    let has_doc = handoff.listings.iter().any(|l| l.channel == Channel::Rag);
    let has_web = handoff.listings.iter().any(|l| l.channel == Channel::Search);

    let mut s = String::new();
    s.push_str("### Citation markers (required)\n");
    if !has_doc && !has_web {
        s.push_str("No usable evidence retrieved — do not emit any citation markers.\n");
        return s;
    }
    s.push_str(
        "- Ground every evidence-based claim with `[[E:id]]` right after it, where `id` is one \
         of the evidence ids listed above (copy exactly, e.g. `[[E3]]`). One claim may carry \
         several markers: `[[E2]][[E5]]`.\n",
    );
    if !has_doc {
        s.push_str("- Workspace retrieval returned nothing usable: say 未命中 for document-side \
                    facts; do not cite document evidence.\n");
    }
    if !has_web {
        s.push_str(
            "- Web retrieval returned nothing usable: web-side content may only be presented as \
             general knowledge or 未检索到; do not cite web evidence.\n",
        );
    }
    s.push_str(
        "- Never invent ids not listed above; never emit `[[cite:...]]` or `[[web:n]]` — the \
         runtime converts your `[[E:id]]` markers itself.\n",
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
    use crate::orchestrator::store::EvidenceListing;

    fn listing(eid: &str, channel: Channel) -> EvidenceListing {
        EvidenceListing {
            eid: eid.into(),
            channel,
            label: "《立项报告》p5".into(),
            preview: "现状诊断内容".into(),
        }
    }

    fn note(channel: Channel, status: PackStatus, item_count: usize) -> ChannelNote {
        ChannelNote {
            channel,
            status,
            item_count,
            note: None,
            error: None,
        }
    }

    fn rec(channel: Channel, status: PackStatus) -> DispatchRecord {
        DispatchRecord {
            channel,
            dispatch_id: "d1".into(),
            status,
            item_count: 0,
            error: None,
        }
    }

    #[test]
    fn brief_has_doc_identity_listings_and_policy() {
        let h = synthesize_handoff(
            "q",
            vec![SourceDoc {
                doc_id: "d1".into(),
                file_name: "数字化转型IT立项报告.docx".into(),
                genre: Some("report".into()),
            }],
            vec![listing("E1", Channel::Rag), listing("E2", Channel::Search)],
            vec![note(Channel::Rag, PackStatus::Ok, 1)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("数字化转型IT立项报告.docx"), "doc identity: {ctx}");
        assert!(ctx.contains("genre: report"), "genre: {ctx}");
        assert!(ctx.contains("[E1]"), "listing: {ctx}");
        assert!(ctx.contains("[[E:id]]"), "E-marker rule: {ctx}");
        assert!(ctx.contains("理解口径"), "interpretation rule: {ctx}");
    }

    #[test]
    fn empty_search_forbids_web_markers() {
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![
                note(Channel::Rag, PackStatus::Ok, 1),
                note(Channel::Search, PackStatus::Empty, 0),
            ],
            &[
                rec(Channel::Rag, PackStatus::Ok),
                rec(Channel::Search, PackStatus::Empty),
            ],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("do not cite web evidence"), "web ban: {ctx}");
        assert!(
            h.partial_notices.iter().any(|n| n.contains("禁止给出网页编号引用")),
            "notice: {:?}",
            h.partial_notices
        );
    }

    #[test]
    fn no_evidence_no_markers() {
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![],
            vec![note(Channel::Rag, PackStatus::Empty, 0)],
            &[rec(Channel::Rag, PackStatus::Empty)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("do not emit any citation markers"));
        assert!(h.partial_notices.iter().any(|n| n.contains("未命中")));
    }
}
