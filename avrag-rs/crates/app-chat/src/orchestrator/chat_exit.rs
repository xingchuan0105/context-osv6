//! Answer phase helpers for the Product Agent runtime (Option D).
//!
//! The synthesize brief carries the **full host-decided evidence set**
//! (store `E{n}` + full bodies) plus worker digests and source-document
//! identity on the **agent query** (not system). Answer must not re-select
//! or invent evidence (2026-07-20). Cites `[[E:id]]`; the host rewrites those
//! to product markers after the run (`workers::finalize_answer_evidence`).
//!
//! Option D: Answer pack = `product-answer-base` + material blocks (P1-2: no
//! full `chat-base`); utility tools via custom ModeConfig; prose-only contract.

use super::invariant::partial_notices_from_records;
use super::store::{EvidenceEntry, EvidenceKind, EvidenceListing, SourceDoc};
use super::types::{Channel, ChannelNote, ChatExitMode, ChatHandoff, DispatchRecord, PackStatus};

/// Build synthesize handoff from the evidence store + dispatch ledger.
pub fn synthesize_handoff(
    user_query: &str,
    source_docs: Vec<SourceDoc>,
    listings: Vec<EvidenceListing>,
    targeted: Vec<EvidenceEntry>,
    channel_notes: Vec<ChannelNote>,
    records: &[DispatchRecord],
    instruction: Option<String>,
) -> ChatHandoff {
    let mut notices = partial_notices_from_records(records);
    // R7: notices come from the single ledger generator (invariant.rs). The
    // per-ChannelNote re-derivation that double-fired is removed.
    // Dedupe while preserving order
    let mut seen = std::collections::HashSet::new();
    notices.retain(|n| seen.insert(n.clone()));

    ChatHandoff {
        mode: ChatExitMode::Synthesize,
        user_query: user_query.to_string(),
        instruction,
        source_docs,
        listings,
        targeted,
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
        targeted: vec![],
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

    // S4 gray zone: premise-mismatch signals render BEFORE channel outcomes
    // (prominent) — the Answer must correct the premise first, never answer
    // under the wrong frame (design §3.2; q114).
    let mismatches: Vec<(&ChannelNote, &super::types::PremiseMismatch)> = handoff
        .channel_notes
        .iter()
        .filter_map(|n| n.handoff.as_ref()?.premise_mismatch.as_ref().map(|pm| (n, pm)))
        .collect();
    if !mismatches.is_empty() {
        s.push_str("### ⚠ 前提质疑 (premise mismatch — worker 发现题目前提与证据不符)\n");
        for (note, pm) in &mismatches {
            s.push_str(&format!(
                "- [{}] kind: {} — {}\n",
                note.channel.as_str(),
                pm.kind,
                pm.detail
            ));
            if let Some(subj) = &pm.actual_subject {
                s.push_str(&format!("  actual_subject: {subj}\n"));
            }
        }
        s.push_str(
            "作答前必须先纠正前提（点名真正主体/真正框架），再决定拒答或按纠正后口径作答；\
             不得为满足问题结构把其他主体的内容归入所问主体。\n\n",
        );
    }

    // Channel outcomes: structured worker handoff (coverage/gaps visible).
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
            if let Some(h) = note.handoff.as_ref() {
                s.push_str(&format!("  coverage: {}\n", h.coverage));
                if h.handoff_degraded {
                    // C4/P3 → S4 wording (supersedes 2026-07-27 P3 文案):
                    // worker output failed the output compiler — the Answer
                    // must not trust it (treat as uncovered). Diagnostic
                    // codes ride along when present.
                    if h.compile_diagnostics.is_empty() {
                        s.push_str("  ⚠ worker 输出未通过编译（诊断码见日志），按未覆盖处理\n");
                    } else {
                        s.push_str(&format!(
                            "  ⚠ worker 输出未通过编译（诊断码：{}），按未覆盖处理\n",
                            h.compile_diagnostics.join(", ")
                        ));
                    }
                }
                if !h.summary.trim().is_empty() {
                    s.push_str("  summary: ");
                    s.push_str(h.summary.trim());
                    s.push('\n');
                }
                if !h.key_facts.is_empty() {
                    s.push_str("  key_facts:\n");
                    for fact in &h.key_facts {
                        // S4: inferred facts stay labeled and never occupy
                        // fact position (design §3.2).
                        let claim = if fact.is_inferred() {
                            format!("（推断）{}", fact.claim)
                        } else {
                            fact.claim.clone()
                        };
                        if fact.evidence.is_empty() {
                            s.push_str(&format!("  - {claim}\n"));
                        } else {
                            s.push_str(&format!(
                                "  - {} (evidence: {})\n",
                                claim,
                                fact.evidence.join(", ")
                            ));
                        }
                    }
                    if h.key_facts.iter().any(|f| f.is_inferred()) {
                        s.push_str("  （推断内容不得作为事实引用）\n");
                    }
                }
                if !h.gaps.is_empty() {
                    s.push_str("  gaps:\n");
                    for g in &h.gaps {
                        s.push_str(&format!("  - {g}\n"));
                    }
                }
            } else if let Some(n) = note.note.as_deref().filter(|n| !n.trim().is_empty()) {
                s.push_str("  worker digest: ");
                s.push_str(n);
                s.push('\n');
            }
        }
        s.push('\n');
    }

    // Targeted doc orientation (genre / section map / doc summary) — full text,
    // orientation only: NOT citable (ids absent from the evidence list below).
    if !handoff.targeted.is_empty() {
        s.push_str("### 文档定向 (document targeting — orientation only, do NOT cite)\n");
        for t in &handoff.targeted {
            let name = t
                .doc_name
                .as_deref()
                .or(t.doc_id.as_deref())
                .unwrap_or("document");
            s.push_str(&format!("- 《{name}》\n{}\n", t.full_text.trim()));
        }
        s.push('\n');
    }

    // Full evidence bodies — intact chunks already filtered by the RAG pipeline
    // (dynamic TOPK/TOPN). Chat must use only these; do not re-fetch or invent.
    let citable: Vec<&EvidenceListing> = handoff
        .listings
        .iter()
        .filter(|l| l.kind != EvidenceKind::DocProfile)
        .collect();
    if !citable.is_empty() {
        s.push_str("### Evidence (complete set — cite by id only)\n");
        s.push_str(
            "The following is the **entire** evidence set for this turn (already selected by \
             the retrieval pipeline). Bodies are whole chunks — do not assume truncation. \
             Use only these passages. Do not claim you lack documents when this section is \
             non-empty. Cite with `[[E:id]]` using the id in brackets (e.g. `[[E3]]`).\n\n",
        );
        for l in citable {
            let body = if l.full_text.trim().is_empty() {
                l.preview.trim()
            } else {
                l.full_text.trim()
            };
            s.push_str(&format!("#### [{}] {}\n{}\n\n", l.eid, l.label, body));
        }
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
/// channels that actually returned evidence. Targeted (DocProfile) entries are
/// orientation context, not evidence — never citable.
fn render_citation_contract(handoff: &ChatHandoff) -> String {
    let has_doc = handoff
        .listings
        .iter()
        .any(|l| l.channel == Channel::Rag && l.kind != EvidenceKind::DocProfile);
    let has_web = handoff
        .listings
        .iter()
        .any(|l| l.channel == Channel::Search);

    let mut s = String::new();
    s.push_str("### Citation markers (required)\n");
    if !has_doc && !has_web {
        s.push_str("No usable evidence retrieved — do not emit any citation markers.\n");
        return s;
    }
    s.push_str(
        "- Ground every evidence-based claim with `[[E:id]]` right after it, where `id` is the \
         short store id listed above (copy exactly: `[[E3]]` — never paste chunk UUIDs into \
         the E-marker). One claim may carry several markers: `[[E2]][[E5]]`.\n",
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
    use crate::orchestrator::store::{EvidenceEntry, EvidenceListing};

    fn listing(eid: &str, channel: Channel) -> EvidenceListing {
        EvidenceListing {
            eid: eid.into(),
            channel,
            kind: match channel {
                Channel::Rag => EvidenceKind::DocChunk,
                Channel::Search => EvidenceKind::WebPage,
            },
            label: "《立项报告》p5".into(),
            preview: "现状诊断内容".into(),
            full_text: "现状诊断内容 — 完整证据正文用于作答".into(),
            chunk_id: Some("chunk-1".into()),
            doc_id: Some("d1".into()),
            score: Some(0.9),
            url: None,
        }
    }

    fn targeted_entry(eid: &str) -> EvidenceEntry {
        EvidenceEntry {
            eid: eid.into(),
            channel: Channel::Rag,
            kind: EvidenceKind::DocProfile,
            chunk_id: None,
            doc_id: Some("d1".into()),
            doc_name: Some("数字化转型IT立项报告.docx".into()),
            page: None,
            url: None,
            title: None,
            preview: "genre: report".into(),
            full_text: "genre: report\nsections: 现状诊断 (p3), 基础设施选型 (p12)".into(),
            score: None,
        }
    }

    fn note(channel: Channel, status: PackStatus, item_count: usize) -> ChannelNote {
        ChannelNote::with_handoff(channel, status, item_count, None, None)
    }

    fn note_with_handoff(
        channel: Channel,
        status: PackStatus,
        item_count: usize,
        summary: &str,
        coverage: &str,
        gaps: &[&str],
    ) -> ChannelNote {
        use crate::orchestrator::types::WorkerHandoff;
        ChannelNote::with_handoff(
            channel,
            status,
            item_count,
            Some(WorkerHandoff {
                summary: summary.into(),
                key_facts: vec![],
                coverage: coverage.into(),
                gaps: gaps.iter().map(|s| (*s).to_string()).collect(),
                handoff_degraded: false,
                compile_diagnostics: vec![],
                premise_mismatch: None,
            }),
            None,
        )
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
            vec![],
            vec![note(Channel::Rag, PackStatus::Ok, 1)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("数字化转型IT立项报告.docx"), "doc identity: {ctx}");
        assert!(ctx.contains("genre: report"), "genre: {ctx}");
        assert!(ctx.contains("[E1]"), "listing: {ctx}");
        assert!(
            ctx.contains("完整证据正文用于作答"),
            "full evidence body must be injected: {ctx}"
        );
        assert!(ctx.contains("complete set"), "complete-set rule: {ctx}");
        assert!(ctx.contains("[[E:id]]") || ctx.contains("[[E3]]"), "E-marker rule: {ctx}");
        assert!(ctx.contains("理解口径"), "interpretation rule: {ctx}");
    }

    #[test]
    fn empty_search_forbids_web_markers() {
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![],
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
            h.partial_notices.iter().any(|n| n.contains("网页编号引用")),
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
            vec![],
            vec![note(Channel::Rag, PackStatus::Empty, 0)],
            &[rec(Channel::Rag, PackStatus::Empty)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("do not emit any citation markers"));
        assert!(h.partial_notices.iter().any(|n| n.contains("未检索到任何证据")));
    }

    #[test]
    fn targeted_entries_render_orientation_but_not_citable() {
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E2", Channel::Rag)],
            vec![targeted_entry("E1")],
            vec![note(Channel::Rag, PackStatus::Ok, 2)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("文档定向"), "targeting section: {ctx}");
        assert!(ctx.contains("基础设施选型 (p12)"), "full text: {ctx}");
        assert!(ctx.contains("do NOT cite"), "not-citable rule: {ctx}");
        // Targeted orientation mentions doc name but citable list is E2 only.
        assert!(ctx.contains("#### [E2]"), "E2 stays citable: {ctx}");
        // E1 is only under 文档定向, not as a citable #### heading.
        let after_evidence = ctx
            .split("### Evidence (complete set")
            .nth(1)
            .expect("evidence section");
        assert!(
            !after_evidence.contains("#### [E1]"),
            "E1 must not be citable heading: {after_evidence}"
        );
    }

    #[test]
    fn structured_handoff_renders_coverage_and_gaps() {
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![],
            vec![note_with_handoff(
                Channel::Rag,
                PackStatus::Ok,
                3,
                "覆盖现状与目标两章",
                "partial",
                &["未找到投资估算章节"],
            )],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("coverage: partial"), "{ctx}");
        assert!(ctx.contains("覆盖现状与目标两章"), "{ctx}");
        assert!(ctx.contains("未找到投资估算章节"), "{ctx}");
        assert!(ctx.contains("gaps:"), "{ctx}");
    }

    // ---- S4: gray-zone rendering -------------------------------------------

    fn note_with_full_handoff(handoff: crate::orchestrator::types::WorkerHandoff) -> ChannelNote {
        ChannelNote::with_handoff(Channel::Rag, PackStatus::Ok, 2, Some(handoff), None)
    }

    fn handoff_with_facts(facts: Vec<crate::orchestrator::types::WorkerKeyFact>) -> crate::orchestrator::types::WorkerHandoff {
        crate::orchestrator::types::WorkerHandoff {
            summary: "s".into(),
            key_facts: facts,
            coverage: "partial".into(),
            gaps: vec![],
            handoff_degraded: false,
            compile_diagnostics: vec![],
            premise_mismatch: None,
        }
    }

    #[test]
    fn inferred_facts_render_labeled_and_not_citable() {
        use crate::orchestrator::types::WorkerKeyFact;
        let handoff = handoff_with_facts(vec![
            WorkerKeyFact {
                claim: "Y公司营销人员编制为 4 人".into(),
                evidence: vec!["chunk-a".into()],
                basis: "observed".into(),
            },
            WorkerKeyFact {
                claim: "访谈覆盖了全部 4 名营销人员".into(),
                evidence: vec![],
                basis: "inferred".into(),
            },
        ]);
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![],
            vec![note_with_full_handoff(handoff)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("- Y公司营销人员编制为 4 人 (evidence: chunk-a)"), "{ctx}");
        assert!(ctx.contains("- （推断）访谈覆盖了全部 4 名营销人员"), "{ctx}");
        assert!(ctx.contains("推断内容不得作为事实引用"), "{ctx}");
    }

    #[test]
    fn premise_mismatch_renders_before_channel_outcomes() {
        use crate::orchestrator::types::PremiseMismatch;
        let mut handoff = handoff_with_facts(vec![]);
        handoff.premise_mismatch = Some(PremiseMismatch {
            kind: "frame".into(),
            detail: "问题预设的 4P 拆解属于竞争对手南通四方".into(),
            actual_subject: Some("Y公司策略为 4R 框架".into()),
        });
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![],
            vec![note_with_full_handoff(handoff)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(ctx.contains("⚠ 前提质疑"), "{ctx}");
        assert!(ctx.contains("kind: frame"), "{ctx}");
        assert!(ctx.contains("4P 拆解属于竞争对手南通四方"), "{ctx}");
        assert!(ctx.contains("actual_subject: Y公司策略为 4R 框架"), "{ctx}");
        assert!(ctx.contains("先纠正前提"), "{ctx}");
        // Prominent: the block precedes Channel outcomes.
        let pm_pos = ctx.find("⚠ 前提质疑").unwrap();
        let outcomes_pos = ctx.find("### Channel outcomes").unwrap();
        assert!(pm_pos < outcomes_pos, "premise block must precede outcomes: {ctx}");
    }

    #[test]
    fn degraded_handoff_renders_compile_wording_with_codes() {
        let mut handoff = handoff_with_facts(vec![]);
        handoff.handoff_degraded = true;
        handoff.compile_diagnostics = vec!["E101".into(), "E103".into()];
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![],
            vec![note_with_full_handoff(handoff)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(
            ctx.contains("⚠ worker 输出未通过编译（诊断码：E101, E103），按未覆盖处理"),
            "{ctx}"
        );
        assert!(!ctx.contains("未通过校验"), "P3 wording superseded: {ctx}");
    }

    #[test]
    fn degraded_handoff_without_codes_falls_back_to_log_wording() {
        let mut handoff = handoff_with_facts(vec![]);
        handoff.handoff_degraded = true;
        let h = synthesize_handoff(
            "q",
            vec![],
            vec![listing("E1", Channel::Rag)],
            vec![],
            vec![note_with_full_handoff(handoff)],
            &[rec(Channel::Rag, PackStatus::Ok)],
            None,
        );
        let ctx = render_synthesize_context(&h);
        assert!(
            ctx.contains("⚠ worker 输出未通过编译（诊断码见日志），按未覆盖处理"),
            "{ctx}"
        );
    }
}
