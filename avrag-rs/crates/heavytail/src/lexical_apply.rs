//! Deterministic lexical edits for WriteRefine `write_refine_lexical`.

use crate::workspace::{DraftWorkspace, SentenceId};

/// One successful lexical edit on a live sentence.
#[derive(Debug, Clone)]
pub struct LexicalEdit {
    pub id: String,
    pub before: String,
    pub after: String,
}

/// Result of a lexical apply attempt.
#[derive(Debug, Clone, Default)]
pub struct LexicalApplyResult {
    pub edits: Vec<LexicalEdit>,
    pub errors: Vec<String>,
}

/// Replace `from` with `to` in selected (or all) live sentences, up to `max`.
pub fn apply_replace_term(
    workspace: &mut DraftWorkspace,
    from: &str,
    to: &str,
    sentence_ids: &[SentenceId],
    max: usize,
) -> LexicalApplyResult {
    if from.is_empty() || to.is_empty() {
        return LexicalApplyResult {
            errors: vec!["empty from/to".into()],
            ..Default::default()
        };
    }
    apply_on_sentences(workspace, sentence_ids, max, |text| {
        if !text.contains(from) {
            return None;
        }
        Some(text.replacen(from, to, 1))
    })
}

/// Repeat `term` once more in selected sentences (simple append-before-period heuristic).
pub fn apply_repeat_term(
    workspace: &mut DraftWorkspace,
    term: &str,
    sentence_ids: &[SentenceId],
    max: usize,
) -> LexicalApplyResult {
    if term.chars().count() < 2 {
        return LexicalApplyResult {
            errors: vec!["term too short".into()],
            ..Default::default()
        };
    }
    apply_on_sentences(workspace, sentence_ids, max, |text| {
        if text.contains(term) {
            // Already present — try to insert a second occurrence before final punct.
            let (body, punct) = split_final_punct(text);
            if body.contains(term) {
                return Some(format!("{body}{term}{punct}"));
            }
        }
        let (body, punct) = split_final_punct(text);
        Some(format!("{body}{term}{punct}"))
    })
}

fn split_final_punct(text: &str) -> (&str, &str) {
    let trimmed = text.trim_end();
    for p in ['。', '！', '？', '.', '!', '?'] {
        if let Some(stripped) = trimmed.strip_suffix(p) {
            return (stripped, &trimmed[stripped.len()..]);
        }
    }
    (trimmed, "")
}

fn apply_on_sentences(
    workspace: &mut DraftWorkspace,
    sentence_ids: &[SentenceId],
    max: usize,
    mut edit: impl FnMut(&str) -> Option<String>,
) -> LexicalApplyResult {
    let mut result = LexicalApplyResult::default();
    let targets: Vec<SentenceId> = if sentence_ids.is_empty() {
        workspace.live().map(|s| s.id.clone()).collect()
    } else {
        sentence_ids.to_vec()
    };

    for id in targets {
        if result.edits.len() >= max {
            break;
        }
        let id_str = id.0.clone();
        let Some(before) = workspace.get(&id).map(|s| s.text.clone()) else {
            result.errors.push(format!("unknown id {id_str}"));
            continue;
        };
        let Some(after) = edit(&before) else {
            continue;
        };
        if after == before {
            continue;
        }
        if let Some(rec) = workspace.get_mut(&id) {
            rec.text = after.clone();
            result.edits.push(LexicalEdit {
                id: id_str,
                before,
                after,
            });
        } else {
            result.errors.push(format!("unknown id {id_str}"));
        }
    }

    if result.edits.is_empty() && result.errors.is_empty() {
        result.errors.push("no sentences matched".into());
    }
    result
}
