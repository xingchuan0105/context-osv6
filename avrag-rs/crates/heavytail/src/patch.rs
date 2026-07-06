//! Patch grammar parser, allow-set validation, and workspace splicer (spec §5.3).

use std::collections::{BTreeMap, BTreeSet};

use crate::segment::is_single_sentence;
use crate::workspace::{DraftWorkspace, SentenceId, SentenceRecord};

/// IDs permitted in a refinement patch for one round.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowSet {
    /// EXTEND/REWRITE/PROMOTE targets and MERGE keep-id.
    pub replace: BTreeSet<SentenceId>,
    /// Parent → declared split children.
    pub split_children: BTreeMap<SentenceId, (SentenceId, SentenceId)>,
    /// MERGE absorb-ids — must not appear in patch output; tombstoned on apply.
    pub tombstone_on_apply: BTreeSet<SentenceId>,
}

impl AllowSet {
    fn allows(&self, id: &SentenceId) -> bool {
        if self.replace.contains(id) {
            return true;
        }
        self.split_children
            .values()
            .any(|(a, b)| a == id || b == id)
    }

    fn is_split_child(&self, id: &SentenceId) -> bool {
        self.split_children
            .values()
            .any(|(a, b)| a == id || b == id)
    }

    fn split_parent_of(&self, child: &SentenceId) -> Option<&SentenceId> {
        self.split_children
            .iter()
            .find(|(_, (a, b))| a == child || b == child)
            .map(|(parent, _)| parent)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    BadLine(usize),
    UnknownId(SentenceId),
    TombstonedId(SentenceId),
    NotSingleSentence(SentenceId),
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub lines: Vec<(SentenceId, String)>,
}

/// Reject-whole-patch semantics: any violation returns `Err`.
///
/// Line grammar: `^(s[0-9]+[a-z]*)\|\s?(.+)$`
pub fn parse_patch(raw: &str, allow: &AllowSet) -> Result<Patch, PatchError> {
    let mut lines = Vec::new();
    let mut seen_ids: BTreeSet<SentenceId> = BTreeSet::new();
    let mut split_children_seen: BTreeMap<SentenceId, BTreeSet<SentenceId>> = BTreeMap::new();

    for (line_no, raw_line) in raw.lines().enumerate() {
        let line_num = line_no + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((id_str, text)) = line.split_once('|') else {
            return Err(PatchError::BadLine(line_num));
        };

        let id_str = id_str.trim();
        if !SentenceId::is_valid(id_str) {
            return Err(PatchError::BadLine(line_num));
        }
        let id = SentenceId::new(id_str);
        let text = text.trim_start().to_string();

        if text.is_empty() {
            return Err(PatchError::BadLine(line_num));
        }

        if allow.tombstone_on_apply.contains(&id) {
            return Err(PatchError::TombstonedId(id));
        }

        if !allow.allows(&id) {
            return Err(PatchError::UnknownId(id));
        }

        if !is_single_sentence(&text) {
            return Err(PatchError::NotSingleSentence(id));
        }

        if !seen_ids.insert(id.clone()) {
            return Err(PatchError::BadLine(line_num));
        }

        if let Some(parent) = allow.split_parent_of(&id) {
            split_children_seen
                .entry(parent.clone())
                .or_default()
                .insert(id.clone());
        }

        lines.push((id, text));
    }

    if lines.is_empty() {
        return Err(PatchError::Empty);
    }

    for (parent, (ca, cb)) in &allow.split_children {
        let seen = split_children_seen.get(parent);
        let has_a = seen.is_some_and(|s| s.contains(ca));
        let has_b = seen.is_some_and(|s| s.contains(cb));
        if has_a ^ has_b {
            return Err(PatchError::BadLine(0));
        }
        if has_a && has_b && seen_ids.contains(parent) {
            return Err(PatchError::BadLine(0));
        }
    }

    Ok(Patch { lines })
}

/// Splice patch lines into the workspace. Returns IDs that changed (including tombstones).
pub fn apply_patch(
    ws: &mut DraftWorkspace,
    patch: &Patch,
    allow: &AllowSet,
) -> Vec<SentenceId> {
    let patch_map: BTreeMap<SentenceId, String> = patch
        .lines
        .iter()
        .map(|(id, text)| (id.clone(), text.clone()))
        .collect();
    let patch_ids: BTreeSet<SentenceId> = patch_map.keys().cloned().collect();

    let untouched_before: BTreeMap<SentenceId, String> = ws
        .live()
        .filter(|s| {
            !patch_ids.contains(&s.id)
                && !allow.split_children.contains_key(&s.id)
                && !allow.tombstone_on_apply.contains(&s.id)
        })
        .map(|s| (s.id.clone(), s.text.clone()))
        .collect();

    let mut changed = Vec::new();

    for (parent, (ca, cb)) in &allow.split_children {
        if patch_map.contains_key(ca) && patch_map.contains_key(cb) {
            let Some(parent_idx) = ws.find_index(parent) else {
                continue;
            };
            let para = ws.sentences[parent_idx].para;
            ws.sentences[parent_idx].tombstone = true;
            changed.push(parent.clone());

            let ca_rec = SentenceRecord {
                id: ca.clone(),
                text: patch_map[ca].clone(),
                para,
                tombstone: false,
            };
            let cb_rec = SentenceRecord {
                id: cb.clone(),
                text: patch_map[cb].clone(),
                para,
                tombstone: false,
            };
            ws.sentences.insert(parent_idx, ca_rec);
            ws.sentences.insert(parent_idx + 1, cb_rec);
            changed.push(ca.clone());
            changed.push(cb.clone());
        }
    }

    for (id, text) in &patch.lines {
        if allow.is_split_child(id) {
            continue;
        }
        if let Some(rec) = ws.get_mut(id) {
            rec.text = text.clone();
            rec.tombstone = false;
            changed.push(id.clone());
        }
    }

    for absorb in &allow.tombstone_on_apply {
        if let Some(rec) = ws.get_mut(absorb) {
            rec.tombstone = true;
            changed.push(absorb.clone());
        }
    }

    debug_assert!(unchanged_live_match(ws, &untouched_before));

    changed.sort();
    changed.dedup();
    changed
}

fn unchanged_live_match(ws: &DraftWorkspace, before: &BTreeMap<SentenceId, String>) -> bool {
    for (id, text) in before {
        let Some(after) = ws.get(id) else {
            return false;
        };
        if after.tombstone || after.text.as_bytes() != text.as_bytes() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::RhythmMode;

    fn sample_ws() -> DraftWorkspace {
        let mut ws = DraftWorkspace::default();
        ws.append_section("第一句很长需要拆分。第二句短。第三句。", &[RhythmMode::Mixed]);
        ws
    }

    fn id(s: &str) -> SentenceId {
        SentenceId::new(s)
    }

    #[test]
    fn apply_rewrite_replaces_text() {
        let mut ws = sample_ws();
        let s1 = ws.sentences[0].id.clone();
        let allow = AllowSet {
            replace: [s1.clone()].into_iter().collect(),
            ..Default::default()
        };
        let patch = parse_patch(&format!("{}| 改写后的第一句。", s1), &allow).unwrap();
        apply_patch(&mut ws, &patch, &allow);
        assert_eq!(ws.sentences[0].text, "改写后的第一句。");
        assert_eq!(ws.live().count(), 3);
    }

    #[test]
    fn apply_split_inserts_children() {
        let mut ws = sample_ws();
        let parent = ws.sentences[0].id.clone();
        let (ca, cb) = parent.children();
        let allow = AllowSet {
            split_children: [(parent.clone(), (ca.clone(), cb.clone()))]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let raw = format!("{}| 前半句。\n{}| 后半句也很长。", ca, cb);
        let patch = parse_patch(&raw, &allow).unwrap();
        apply_patch(&mut ws, &patch, &allow);
        assert!(ws.is_tombstoned(&parent));
        assert_eq!(ws.get(&ca).unwrap().text, "前半句。");
        assert_eq!(ws.get(&cb).unwrap().text, "后半句也很长。");
        assert_eq!(ws.live().count(), 4);
    }

    #[test]
    fn apply_merge_tombstones_absorb() {
        let mut ws = sample_ws();
        let keep = ws.sentences[0].id.clone();
        let absorb = ws.sentences[1].id.clone();
        let allow = AllowSet {
            replace: [keep.clone()].into_iter().collect(),
            tombstone_on_apply: [absorb.clone()].into_iter().collect(),
            ..Default::default()
        };
        let patch = parse_patch(&format!("{}| 合并后的长句。", keep), &allow).unwrap();
        apply_patch(&mut ws, &patch, &allow);
        assert_eq!(ws.get(&keep).unwrap().text, "合并后的长句。");
        assert!(ws.is_tombstoned(&absorb));
        assert_eq!(ws.live().count(), 2);
    }

    #[test]
    fn malformed_patch_battery() {
        let ws = sample_ws();
        let s1 = ws.sentences[0].id.clone();
        let s2 = ws.sentences[1].id.clone();
        let (ca, cb) = s1.children();
        let s2_tombstone_line = format!("{}| 已标记删除。", s2);
        let two_sentences_line = format!("{}| 第一句。第二句。", s1);
        let lone_child_line = format!("{}| 只有这一个孩子。", ca);
        let absorb_line = format!("{}| 合并吸收句不应出现。", ws.sentences[1].id);

        let cases: Vec<(&str, AllowSet, PatchError)> = vec![
            (
                "s99| 不存在的编号。",
                AllowSet {
                    replace: [s1.clone()].into_iter().collect(),
                    ..Default::default()
                },
                PatchError::UnknownId(id("s99")),
            ),
            (
                &s2_tombstone_line,
                AllowSet {
                    replace: [s1.clone()].into_iter().collect(),
                    tombstone_on_apply: [s2.clone()].into_iter().collect(),
                    ..Default::default()
                },
                PatchError::TombstonedId(s2.clone()),
            ),
            (
                "缺少分隔符的一行。",
                AllowSet {
                    replace: [s1.clone()].into_iter().collect(),
                    ..Default::default()
                },
                PatchError::BadLine(1),
            ),
            (
                &two_sentences_line,
                AllowSet {
                    replace: [s1.clone()].into_iter().collect(),
                    ..Default::default()
                },
                PatchError::NotSingleSentence(s1.clone()),
            ),
            (
                "",
                AllowSet {
                    replace: [s1.clone()].into_iter().collect(),
                    ..Default::default()
                },
                PatchError::Empty,
            ),
            (
                &lone_child_line,
                AllowSet {
                    split_children: [(s1.clone(), (ca.clone(), cb.clone()))]
                        .into_iter()
                        .collect(),
                    ..Default::default()
                },
                PatchError::BadLine(0),
            ),
            (
                &absorb_line,
                AllowSet {
                    replace: [s1.clone()].into_iter().collect(),
                    tombstone_on_apply: [ws.sentences[1].id.clone()].into_iter().collect(),
                    ..Default::default()
                },
                PatchError::TombstonedId(ws.sentences[1].id.clone()),
            ),
        ];

        let allow_ok = AllowSet {
            replace: [s1.clone()].into_iter().collect(),
            ..Default::default()
        };
        let ok_line = format!("{}| 引用\u{201c}内部。句号\u{201d}合法。", s1);
        assert!(parse_patch(&ok_line, &allow_ok).is_ok());

        for (raw, allow, expected) in cases {
            let got = parse_patch(raw, &allow).unwrap_err();
            assert_eq!(got, expected, "raw={raw:?}");
        }

        let got = parse_patch(&format!("{}| 合法句。", s1), &AllowSet::default()).unwrap_err();
        assert_eq!(got, PatchError::UnknownId(s1));
    }

    #[test]
    fn untouched_sentences_preserved_on_rewrite() {
        let mut ws = sample_ws();
        let s1 = ws.sentences[0].id.clone();
        let s3 = ws.sentences[2].id.clone();
        let s3_before = ws.get(&s3).unwrap().text.clone();
        let allow = AllowSet {
            replace: [s1.clone()].into_iter().collect(),
            ..Default::default()
        };
        let patch = parse_patch(&format!("{}| 只改第一句。", s1), &allow).unwrap();
        apply_patch(&mut ws, &patch, &allow);
        assert_eq!(ws.get(&s3).unwrap().text, s3_before);
    }
}
