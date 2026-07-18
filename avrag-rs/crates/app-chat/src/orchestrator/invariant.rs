//! §7.2 Completion invariant: each materialized channel must have a dispatch record.

use super::types::{Channel, DispatchRecord, TaskBrief};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingChannels {
    pub channels: Vec<Channel>,
}

impl std::fmt::Display for MissingChannels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.channels.iter().map(|c| c.as_str()).collect();
        write!(
            f,
            "missing dispatch for channel(s): {}",
            names.join(", ")
        )
    }
}

/// Return channels in `materialized` that have no finished dispatch record.
pub fn missing_dispatches(
    materialized: &[Channel],
    records: &[DispatchRecord],
) -> Vec<Channel> {
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

/// Default brief when recovering a skipped channel or first-wave dispatch.
pub fn default_brief(channel: Channel, user_query: &str) -> TaskBrief {
    let q = user_query.trim();
    let goal = match channel {
        Channel::Rag => format!(
            "First identify the document's type, purpose, and structure \
             (use doc profile/summary tools if available); then retrieve workspace evidence \
             relevant to: {q}. Extract key claims and distinctive facts for answering. \
             Do not write the final user-facing answer."
        ),
        Channel::Search => format!(
            "Search the web for evidence relevant to: {q}. \
             Use both Chinese and English query terms (translate to industry terminology). \
             Prefer authoritative / best-practice sources when the question implies comparison. \
             Do not write the final user-facing answer."
        ),
    };
    TaskBrief::new(goal)
}

/// §7.3 partial notices from the dispatch ledger (Chat synthesize policy input).
pub fn partial_notices_from_records(records: &[DispatchRecord]) -> Vec<String> {
    let mut notices = Vec::new();
    for r in records {
        match r.status {
            super::types::PackStatus::Empty => notices.push(format!(
                "{}: empty (no evidence retrieved; use 未命中 wording, not user-blame)",
                r.channel.as_str()
            )),
            super::types::PackStatus::Error => notices.push(format!(
                "{}: error ({})",
                r.channel.as_str(),
                r.error.as_deref().unwrap_or("unknown")
            )),
            super::types::PackStatus::Ok => {}
        }
    }
    notices
}

/// Detect banned attribution when retrieval was attempted (soft helper for tests / logs).
pub fn looks_like_user_did_not_provide_doc(answer: &str) -> bool {
    let a = answer.to_lowercase();
    // Incident class: blame user for missing pasted report body.
    // Do not flag meta instructions that merely mention 未提供 as a forbidden phrase.
    if answer.contains("勿将") || answer.contains("禁止说") || answer.contains("not user-blame") {
        return false;
    }
    (answer.contains("未提供")
        && (answer.contains("报告") || answer.contains("正文") || answer.contains("内容")))
        || (a.contains("did not provide") && (a.contains("report") || a.contains("document")))
        || (a.contains("you didn't provide") || a.contains("you did not provide"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::PackStatus;

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
    fn default_brief_rag_requires_orientation_first() {
        let b = default_brief(Channel::Rag, "方案差距");
        assert!(b.goal.contains("方案差距"));
        assert!(
            b.goal.contains("identify the document's type"),
            "orientation-first: {}",
            b.goal
        );
    }

    #[test]
    fn default_brief_search_requires_bilingual() {
        let b = default_brief(Channel::Search, "最佳实践");
        assert!(
            b.goal.contains("Chinese and English"),
            "bilingual: {}",
            b.goal
        );
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
        assert!(n[0].contains("rag"));
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
