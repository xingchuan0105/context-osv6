//! §7.2 Completion invariant: each materialized channel must have a dispatch record.

use super::types::{Channel, DispatchRecord, TaskBrief};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingChannels {
    pub channels: Vec<Channel>,
}

impl std::fmt::Display for MissingChannels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.channels.iter().map(|c| c.as_str()).collect();
        write!(f, "missing dispatch for channel(s): {}", names.join(", "))
    }
}

/// Return channels in `materialized` that have no finished dispatch record.
pub fn missing_dispatches(materialized: &[Channel], records: &[DispatchRecord]) -> Vec<Channel> {
    materialized
        .iter()
        .copied()
        .filter(|ch| !records.iter().any(|r| r.channel == *ch))
        .collect()
}

/// §7.2: synthesize/finalize only when every materialized channel has ≥1 record.
pub fn assert_complete(
    materialized: &[Channel],
    records: &[DispatchRecord],
) -> Result<(), MissingChannels> {
    let missing = missing_dispatches(materialized, records);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(MissingChannels { channels: missing })
    }
}

/// Default brief: the finish-gate fallback when a channel must be dispatched
/// but no LLM-written brief exists (O1 structural first wave / §7.2 recovery).
///
/// Deliberately **policy-free**: retrieval doctrine (document orientation,
/// bilingual queries, de-referencing) lives in the prompt layer
/// (`capability-rag.md` / `capability-search.md` / `orchestrator-base.md`),
/// not in code. V2's ReAct orchestrator writes real briefs itself.
pub fn default_brief(channel: Channel, user_query: &str) -> TaskBrief {
    let _ = channel;
    TaskBrief::new(user_query.trim())
}

/// §7.3 partial notices from the dispatch ledger (Chat synthesize policy input).
///
/// R7: THE single partial-notice generator (previously double-fired with
/// chat_exit's ChannelNote loop, now removed). Empty channels carry the hard
/// zero-evidence instruction (P3): declare uncovered only — never fill from
/// worker narrative or common sense.
///
/// A1 (2026-07-28): notices aggregate PER CHANNEL, not per dispatch record —
/// a channel may be dispatched several times (re-dispatch). The zero-evidence
/// hard instruction fires only when EVERY record for the channel is Empty
/// (no items at all across all dispatches); any Ok record for the channel
/// suppresses it entirely (a sibling dispatch DID return evidence — firing
/// would contradict the Evidence section, q114 autopsy). Error notices get
/// the same treatment: they fire only when the channel has no Ok record.
pub fn partial_notices_from_records(records: &[DispatchRecord]) -> Vec<String> {
    let mut notices = Vec::new();
    for channel in [Channel::Rag, Channel::Search] {
        let rs: Vec<&DispatchRecord> = records.iter().filter(|r| r.channel == channel).collect();
        if rs.is_empty() {
            continue;
        }
        // Any successful dispatch for this channel suppresses its notices.
        if rs.iter().any(|r| r.status == super::types::PackStatus::Ok) {
            continue;
        }
        use super::types::PackStatus;
        let all_empty = rs.iter().all(|r| r.status == PackStatus::Empty);
        if all_empty {
            match channel {
                Channel::Rag => notices.push(
                    "工作区未检索到任何证据 — 只能声明未覆盖；禁止使用 worker 叙述或常识补写具体事实。"
                        .to_string(),
                ),
                Channel::Search => notices.push(
                    "网络检索未返回可用结果 — 只能表述为未检索到；禁止补写具体网页事实或网页编号引用。"
                        .to_string(),
                ),
            }
            continue;
        }
        if let Some(err_rec) = rs.iter().find(|r| r.status == PackStatus::Error) {
            match channel {
                Channel::Rag => notices.push(format!(
                    "工作区检索失败（{}）；若有网页证据可仅用网页并说明。",
                    err_rec.error.as_deref().unwrap_or("unknown")
                )),
                Channel::Search => notices.push(format!(
                    "网络检索失败（{}）；网页侧内容只能表述为未检索到，禁止给出网页编号引用。",
                    err_rec.error.as_deref().unwrap_or("unknown")
                )),
            }
        }
    }
    notices
}

/// Detect banned attribution when retrieval was attempted (soft helper for tests / logs).
pub fn looks_like_user_did_not_provide_doc(answer: &str) -> bool {
    let a = answer.to_lowercase();
    // Incident class: blame user for missing pasted report body.
    // Do not flag meta instructions that merely mention 未提供 as a forbidden phrase.
    if answer.contains("勿将") || answer.contains("禁止说") || answer.contains("not user-blame")
    {
        return false;
    }
    (answer.contains("未提供")
        && (answer.contains("报告") || answer.contains("正文") || answer.contains("内容")))
        || (a.contains("did not provide") && (a.contains("report") || a.contains("document")))
        || (a.contains("you didn't provide") || a.contains("you did not provide"))
}

#[cfg(test)]
mod tests {
    use super::super::types::PackStatus;
    use super::*;

    fn rec(ch: Channel) -> DispatchRecord {
        DispatchRecord {
            channel: ch,
            dispatch_id: "d1".into(),
            status: PackStatus::Ok,
            item_count: 1,
            error: None,
        }
    }

    #[test]
    fn dual_requires_both_records() {
        let mat = vec![Channel::Rag, Channel::Search];
        assert!(assert_complete(&mat, &[rec(Channel::Search)]).is_err());
        assert!(assert_complete(&mat, &[rec(Channel::Rag), rec(Channel::Search)]).is_ok());
    }

    #[test]
    fn empty_materialized_always_ok() {
        assert!(assert_complete(&[], &[]).is_ok());
    }

    #[test]
    fn web_only_does_not_cover_rag() {
        let mat = vec![Channel::Rag];
        assert_eq!(
            missing_dispatches(&mat, &[rec(Channel::Search)]),
            vec![Channel::Rag]
        );
    }

    #[test]
    fn default_brief_is_policy_free_passthrough() {
        // Retrieval doctrine lives in prompt files, not in code (2026-07-18
        // layering review). The fallback brief is just the user query.
        let b = default_brief(Channel::Rag, "方案差距");
        assert_eq!(b.goal, "方案差距");
        let b = default_brief(Channel::Search, "  最佳实践  ");
        assert_eq!(b.goal, "最佳实践");
    }

    #[test]
    fn partial_notices_for_empty_channel() {
        let records = vec![DispatchRecord {
            channel: Channel::Rag,
            dispatch_id: "x".into(),
            status: PackStatus::Empty,
            item_count: 0,
            error: None,
        }];
        let n = partial_notices_from_records(&records);
        assert_eq!(n.len(), 1);
        // P3: the empty-channel notice is the hard zero-evidence instruction.
        assert!(n[0].contains("未检索到任何证据"), "{:?}", n[0]);
        assert!(
            n[0].contains("禁止使用 worker 叙述或常识补写"),
            "{:?}",
            n[0]
        );
    }

    // ---- A1: per-channel aggregation (q114 autopsy) -------------------------

    fn rec_with(channel: Channel, status: PackStatus, item_count: usize) -> DispatchRecord {
        DispatchRecord {
            channel,
            dispatch_id: "d".into(),
            status,
            item_count,
            error: None,
        }
    }

    #[test]
    fn ok_sibling_dispatch_suppresses_zero_evidence_notice() {
        // q114: first dispatch returned 124 items, re-dispatch came back empty
        // — the zero-evidence hard instruction must NOT fire.
        let records = vec![
            rec_with(Channel::Rag, PackStatus::Ok, 124),
            rec_with(Channel::Rag, PackStatus::Empty, 0),
        ];
        let n = partial_notices_from_records(&records);
        assert!(!n.iter().any(|s| s.contains("未检索到任何证据")), "{n:?}");
    }

    #[test]
    fn all_empty_dispatches_still_fire_notice() {
        let records = vec![
            rec_with(Channel::Rag, PackStatus::Empty, 0),
            rec_with(Channel::Rag, PackStatus::Empty, 0),
        ];
        let n = partial_notices_from_records(&records);
        assert_eq!(n.len(), 1, "one aggregated notice: {n:?}");
        assert!(n[0].contains("未检索到任何证据"), "{n:?}");
    }

    #[test]
    fn single_ok_record_fires_no_notice() {
        let records = vec![rec_with(Channel::Rag, PackStatus::Ok, 5)];
        assert!(partial_notices_from_records(&records).is_empty());
    }

    #[test]
    fn error_notice_suppressed_by_ok_sibling() {
        let mut err = rec_with(Channel::Search, PackStatus::Error, 0);
        err.error = Some("timeout".into());
        let records = vec![rec_with(Channel::Search, PackStatus::Ok, 3), err];
        let n = partial_notices_from_records(&records);
        assert!(!n.iter().any(|s| s.contains("检索失败")), "{n:?}");
    }

    #[test]
    fn banned_copy_detector() {
        assert!(looks_like_user_did_not_provide_doc(
            "由于您未提供具体的转型报告内容，无法对比"
        ));
        assert!(!looks_like_user_did_not_provide_doc(
            "工作区未命中相关段落；以下基于网页。"
        ));
    }
}
