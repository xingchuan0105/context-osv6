//! Directive compiler and Chinese prompt rendering (spec §10, §5.3).

use std::collections::BTreeSet;

use crate::lexops::{enumerate_lexops, LexOp};
use crate::metrics::FingerprintReport;
use crate::patch::AllowSet;
use crate::placement::plan_placement;
use crate::score::{L_MAX, L_MIN};
use crate::sensitivity::brute_delta_s;
use crate::state::WriterBudget;
use crate::tokenize::tokens;
use crate::workspace::{DraftWorkspace, SentenceId, SentenceRecord};
use crate::StyleParams;

const OVERSHOOT: f64 = 1.3;

/// Sentence-length bin for prompt translation (v1 spec §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LengthBin {
    XShort,
    Short,
    Medium,
    Long,
    XLong,
}

/// Map a character count to a length bin (v1 §7 ranges).
pub fn length_bin(chars: usize) -> LengthBin {
    match chars {
        0..=10 => LengthBin::XShort,
        11..=20 => LengthBin::Short,
        21..=35 => LengthBin::Medium,
        36..=55 => LengthBin::Long,
        _ => LengthBin::XLong,
    }
}

fn bin_label(chars: usize, prefer_within: bool) -> String {
    let bin = length_bin(chars);
    match bin {
        LengthBin::XShort if prefer_within => "10字以内".to_string(),
        LengthBin::XShort => "约5–10字".to_string(),
        LengthBin::Short => "约10–20字".to_string(),
        LengthBin::Medium => "约20–35字".to_string(),
        LengthBin::Long => "约35–55字".to_string(),
        LengthBin::XLong => "约55字以上".to_string(),
    }
}

fn overshoot_target(current: usize, target: usize) -> usize {
    let current = current as f64;
    let target = target as f64;
    let overshot = current + OVERSHOOT * (target - current);
    overshot.round().clamp(L_MIN, L_MAX) as usize
}

fn find_sentences_with_word(live: &[&SentenceRecord], word: &str) -> Vec<SentenceId> {
    live.iter()
        .filter(|s| tokens(&s.text).iter().any(|t| t == word) || s.text.contains(word))
        .map(|s| s.id.clone())
        .collect()
}

fn find_merge_partner(
    sent_idx: usize,
    live: &[&SentenceRecord],
    fp: &FingerprintReport,
    target: usize,
    style: &StyleParams,
) -> Option<usize> {
    let para = live[sent_idx].para;
    let current = fp.sentence_lengths[sent_idx];
    let short_threshold = style.median_length as usize;

    let mut best: Option<(usize, usize)> = None;
    for neighbor in [sent_idx.checked_sub(1), Some(sent_idx + 1)] {
        let Some(neighbor) = neighbor else { continue };
        if neighbor >= live.len() || neighbor == sent_idx {
            continue;
        }
        if live[neighbor].para != para {
            continue;
        }
        let nlen = fp.sentence_lengths[neighbor];
        if nlen <= short_threshold.min(target) && nlen < current {
            if best.map_or(true, |(_, bl)| nlen < bl) {
                best = Some((neighbor, nlen));
            }
        }
    }
    best.map(|(idx, _)| idx)
}

fn pick_weave(sentence: &str, reservoir: &[String]) -> Option<String> {
    reservoir
        .iter()
        .find(|term| !sentence.contains(term.as_str()))
        .cloned()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Directive {
    Split {
        id: SentenceId,
        children: (SentenceId, SentenceId),
        short_bin: String,
        gain: f64,
    },
    Merge {
        keep: SentenceId,
        absorb: SentenceId,
        gain: f64,
    },
    Extend {
        id: SentenceId,
        bin: String,
        weave: Option<String>,
        gain: f64,
    },
    Rewrite {
        id: SentenceId,
        bin: String,
        gain: f64,
    },
    Promote {
        id: SentenceId,
        replace: String,
        with_any_of: Vec<String>,
        gain: f64,
    },
    Demote {
        word: String,
        max_count: usize,
        in_sentences: Vec<SentenceId>,
        gain: f64,
    },
}

impl Directive {
    fn gain(&self) -> f64 {
        match self {
            Self::Split { gain, .. }
            | Self::Merge { gain, .. }
            | Self::Extend { gain, .. }
            | Self::Rewrite { gain, .. }
            | Self::Promote { gain, .. }
            | Self::Demote { gain, .. } => *gain,
        }
    }

    fn is_rhythm(&self) -> bool {
        !matches!(self, Self::Promote { .. } | Self::Demote { .. })
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoundDirectives {
    pub rhythm: Vec<Directive>,
    pub lexical: Vec<Directive>,
    pub allow: AllowSet,
}

/// Deterministic directive compilation (spec §10 step 2).
pub fn compile(
    ws: &DraftWorkspace,
    fp: &FingerprintReport,
    style: &StyleParams,
    reservoir: &[String],
    budget: &WriterBudget,
    seed: u64,
) -> RoundDirectives {
    let live: Vec<&SentenceRecord> = ws.live().collect();
    let para_of: Vec<usize> = live.iter().map(|s| s.para).collect();

    let plan = plan_placement(fp, &para_of, style, budget.max_rhythm_ops, seed);

    let mut ranked_edits: Vec<(usize, usize, f64)> = plan
        .edits
        .iter()
        .map(|&(sent_idx, target)| {
            let current = fp.sentence_lengths[sent_idx];
            let overshoot = overshoot_target(current, target);
            let gain = brute_delta_s(fp, style, sent_idx, overshoot).abs();
            (sent_idx, target, gain)
        })
        .collect();
    ranked_edits.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut rhythm = Vec::new();
    let mut used_indices: BTreeSet<usize> = BTreeSet::new();

    for (sent_idx, target, _) in ranked_edits {
        if rhythm.len() >= budget.max_rhythm_ops {
            break;
        }
        if used_indices.contains(&sent_idx) {
            continue;
        }

        let current = fp.sentence_lengths[sent_idx];
        if current == target {
            continue;
        }

        let overshoot = overshoot_target(current, target);
        let gain = brute_delta_s(fp, style, sent_idx, overshoot);
        let id = live[sent_idx].id.clone();

        if current > target {
            if current > 2 * target {
                let children = id.children();
                rhythm.push(Directive::Split {
                    id: id.clone(),
                    children,
                    short_bin: bin_label(overshoot, true),
                    gain,
                });
                used_indices.insert(sent_idx);
            } else {
                rhythm.push(Directive::Rewrite {
                    id,
                    bin: bin_label(overshoot, overshoot <= 10),
                    gain,
                });
                used_indices.insert(sent_idx);
            }
        } else if let Some(partner) =
            find_merge_partner(sent_idx, &live, fp, target, style)
        {
            if used_indices.contains(&partner) {
                let weave = pick_weave(&live[sent_idx].text, reservoir);
                rhythm.push(Directive::Extend {
                    id,
                    bin: bin_label(overshoot, false),
                    weave,
                    gain,
                });
                used_indices.insert(sent_idx);
            } else {
                let keep_idx = sent_idx.min(partner);
                let absorb_idx = sent_idx.max(partner);
                let keep = live[keep_idx].id.clone();
                let absorb = live[absorb_idx].id.clone();
                rhythm.push(Directive::Merge {
                    keep,
                    absorb,
                    gain,
                });
                used_indices.insert(sent_idx);
                used_indices.insert(partner);
            }
        } else {
            let weave = pick_weave(&live[sent_idx].text, reservoir);
            rhythm.push(Directive::Extend {
                id,
                bin: bin_label(overshoot, false),
                weave,
                gain,
            });
            used_indices.insert(sent_idx);
        }
    }

    rhythm.sort_by(|a, b| {
        b.gain()
            .partial_cmp(&a.gain())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut lexical = compile_lexical_directives(
        fp,
        &live,
        reservoir,
        style,
        budget.max_lexical_ops,
        &rhythm,
    );
    lexical.sort_by(|a, b| {
        b.gain()
            .partial_cmp(&a.gain())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let allow = build_allow_set(&rhythm, &lexical);

    RoundDirectives {
        rhythm,
        lexical,
        allow,
    }
}

fn compile_lexical_directives(
    fp: &FingerprintReport,
    live: &[&SentenceRecord],
    reservoir: &[String],
    style: &StyleParams,
    cap: usize,
    rhythm: &[Directive],
) -> Vec<Directive> {
    let rhythm_blocked: BTreeSet<SentenceId> = rhythm
        .iter()
        .flat_map(|d| match d {
            Directive::Split { id, .. } => vec![id.clone()],
            Directive::Merge { keep, absorb, .. } => vec![keep.clone(), absorb.clone()],
            Directive::Extend { id, .. } | Directive::Rewrite { id, .. } => vec![id.clone()],
            _ => vec![],
        })
        .collect();
    let mut ops = enumerate_lexops(fp, reservoir, style);
    ops.sort_by(|a, b| {
        lexop_gain(b)
            .partial_cmp(&lexop_gain(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut out = Vec::new();
    for op in ops.into_iter().take(cap) {
        match op {
            LexOp::Promote {
                word,
                replacement_pool,
                delta_s,
            } => {
                let ids = find_sentences_with_word(live, &word);
                let Some(id) = ids.into_iter().find(|id| !rhythm_blocked.contains(id)) else {
                    continue;
                };
                out.push(Directive::Promote {
                    id,
                    replace: word,
                    with_any_of: replacement_pool,
                    gain: delta_s,
                });
            }
            LexOp::Demote {
                word,
                max_count,
                delta_s,
                ..
            } => {
                let in_sentences: Vec<SentenceId> = find_sentences_with_word(live, &word)
                    .into_iter()
                    .filter(|id| !rhythm_blocked.contains(id))
                    .collect();
                if in_sentences.is_empty() {
                    continue;
                }
                out.push(Directive::Demote {
                    word,
                    max_count,
                    in_sentences,
                    gain: delta_s,
                });
            }
        }
    }
    out
}

fn lexop_gain(op: &LexOp) -> f64 {
    match op {
        LexOp::Promote { delta_s, .. } | LexOp::Demote { delta_s, .. } => *delta_s,
    }
}

fn build_allow_set(rhythm: &[Directive], lexical: &[Directive]) -> AllowSet {
    let mut allow = AllowSet::default();

    for d in rhythm.iter().chain(lexical.iter()) {
        match d {
            Directive::Split { id, children, .. } => {
                allow.split_children.insert(id.clone(), children.clone());
            }
            Directive::Merge { keep, absorb, .. } => {
                allow.replace.insert(keep.clone());
                allow.tombstone_on_apply.insert(absorb.clone());
            }
            Directive::Extend { id, .. } | Directive::Rewrite { id, .. } | Directive::Promote { id, .. } => {
                allow.replace.insert(id.clone());
            }
            Directive::Demote { in_sentences, .. } => {
                for id in in_sentences {
                    allow.replace.insert(id.clone());
                }
            }
        }
    }

    for parent in allow.split_children.keys() {
        allow.replace.remove(parent);
    }

    allow
}

/// Render the `PATCH DIRECTIVES` block for LLM consumption (spec §5.3).
pub fn render_directives_zh(d: &[Directive]) -> String {
    if d.is_empty() {
        return String::new();
    }

    let mut rhythm_lines = Vec::new();
    let mut lexical_lines = Vec::new();

    for directive in d {
        let line = match directive {
            Directive::Split {
                id,
                children,
                short_bin,
                ..
            } => format!(
                "- SPLIT {id} → {}, {} | 短侧：{short_bin}",
                children.0, children.1
            ),
            Directive::Merge { keep, absorb, .. } => {
                format!("- MERGE {keep} 吸收 {absorb} | 合并为一句长句")
            }
            Directive::Extend { id, bin, weave, .. } => {
                if let Some(term) = weave {
                    format!("- EXTEND {id} | {bin} | 可自然融入：{term}")
                } else {
                    format!("- EXTEND {id} | {bin}")
                }
            }
            Directive::Rewrite { id, bin, .. } => format!("- REWRITE {id} | {bin}"),
            Directive::Promote {
                id,
                replace,
                with_any_of,
                ..
            } => {
                let alts = with_any_of.join("、");
                format!("- PROMOTE {id} | 将「{replace}」替换为「{alts}」之一")
            }
            Directive::Demote {
                word,
                max_count,
                in_sentences,
                ..
            } => {
                let ids: Vec<_> = in_sentences.iter().map(|s| s.to_string()).collect();
                format!(
                    "- DEMOTE 「{word}」| 全文至多出现{max_count}次（涉及 {}）",
                    ids.join("、")
                )
            }
        };

        if directive.is_rhythm() {
            rhythm_lines.push(line);
        } else {
            lexical_lines.push(line);
        }
    }

    let mut out = String::from("PATCH DIRECTIVES\n");
    if !rhythm_lines.is_empty() {
        out.push_str("\n[节奏]\n");
        for line in &rhythm_lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !lexical_lines.is_empty() {
        out.push_str("\n[词汇]\n");
        for line in &lexical_lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("\n规则：仅输出上述句子；未点名的句子不得出现在输出中。\n");
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Collect every sentence ID referenced by directives (for allow-set tests).
pub fn directive_ids(d: &[Directive]) -> BTreeSet<SentenceId> {
    let mut ids = BTreeSet::new();
    for directive in d {
        match directive {
            Directive::Split { id, children, .. } => {
                ids.insert(id.clone());
                ids.insert(children.0.clone());
                ids.insert(children.1.clone());
            }
            Directive::Merge { keep, absorb, .. } => {
                ids.insert(keep.clone());
                ids.insert(absorb.clone());
            }
            Directive::Extend { id, .. }
            | Directive::Rewrite { id, .. }
            | Directive::Promote { id, .. } => {
                ids.insert(id.clone());
            }
            Directive::Demote { in_sentences, .. } => {
                for id in in_sentences {
                    ids.insert(id.clone());
                }
            }
        }
    }
    ids
}

/// IDs permitted in a patch derived from the allow-set (excludes tombstone-only absorbs).
pub fn allow_patch_ids(allow: &AllowSet) -> BTreeSet<SentenceId> {
    let mut ids = allow.replace.clone();
    for (_, (a, b)) in &allow.split_children {
        ids.insert(a.clone());
        ids.insert(b.clone());
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{analyze_sentences, length_metrics, FingerprintReport};
    use crate::workspace::RhythmMode;
    use std::collections::BTreeMap;

    fn uniform_sentence() -> &'static str {
        "这是一句长度恰好二十字左右的示例句子。"
    }

    fn uniform_workspace(n: usize, para_size: usize) -> DraftWorkspace {
        let mut ws = DraftWorkspace::default();
        let s = uniform_sentence();
        let mut chunks = Vec::new();
        for chunk in (0..n).collect::<Vec<_>>().chunks(para_size) {
            chunks.push(chunk.iter().map(|_| s.to_string()).collect::<Vec<_>>().concat());
        }
        let prose = chunks.join("\n\n");
        let para_count = chunks.len();
        let rhythms: Vec<RhythmMode> = (0..para_count).map(|_| RhythmMode::Mixed).collect();
        ws.append_section(&prose, &rhythms);
        ws
    }

    fn fingerprint_from_ws(ws: &DraftWorkspace) -> FingerprintReport {
        let sentences: Vec<(String, usize)> = ws
            .live()
            .map(|s| (s.text.clone(), s.para))
            .collect();
        analyze_sentences(&sentences)
    }

    fn synthetic_fp_with_freq(
        lengths: Vec<usize>,
        word_freq: BTreeMap<String, usize>,
    ) -> FingerprintReport {
        let (mean_length, cv, autocorr_lag1, lognormal_ks_stat) = length_metrics(&lengths);
        let total_tokens: usize = word_freq.values().sum();
        let vocab_size = word_freq.len();
        let hapax_ratio = if vocab_size == 0 {
            0.0
        } else {
            word_freq.values().filter(|&&c| c == 1).count() as f64 / vocab_size as f64
        };
        FingerprintReport {
            sentence_lengths: lengths,
            mean_length,
            cv,
            autocorr_lag1,
            lognormal_ks_stat,
            total_tokens,
            vocab_size,
            ttr: if total_tokens == 0 {
                0.0
            } else {
                vocab_size as f64 / total_tokens as f64
            },
            hapax_ratio,
            zipf_exponent: 1.0,
            word_freq,
        }
    }

    #[test]
    fn uniform_workspace_produces_splits_and_extends() {
        let ws = uniform_workspace(24, 6);
        let fp = fingerprint_from_ws(&ws);
        let style = StyleParams::default();
        let budget = WriterBudget::default();
        let rd = compile(&ws, &fp, &style, &["罕见术语".into()], &budget, 42);

        let has_split = rd.rhythm.iter().any(|d| matches!(d, Directive::Split { .. }));
        let has_extend = rd.rhythm.iter().any(|d| matches!(d, Directive::Extend { .. }));
        assert!(has_split, "expected at least one SPLIT directive");
        assert!(has_extend, "expected at least one EXTEND directive");
    }

    #[test]
    fn compile_respects_rhythm_and_lexical_caps() {
        let ws = uniform_workspace(24, 6);
        let fp = fingerprint_from_ws(&ws);
        let style = StyleParams::default();
        let budget = WriterBudget {
            max_rhythm_ops: 3,
            max_lexical_ops: 2,
            ..WriterBudget::default()
        };

        let mut freq = BTreeMap::new();
        freq.insert("双频词".into(), 2);
        freq.insert("三频词".into(), 3);
        freq.insert("此外".into(), 8);
        let fp_lex = synthetic_fp_with_freq(fp.sentence_lengths.clone(), freq);

        let rd = compile(
            &ws,
            &fp_lex,
            &style,
            &["新词甲".into(), "新词乙".into()],
            &budget,
            7,
        );

        assert!(rd.rhythm.len() <= 3);
        assert!(rd.lexical.len() <= 2);
    }

    #[test]
    fn allow_set_matches_directives_only() {
        let ws = uniform_workspace(20, 5);
        let fp = fingerprint_from_ws(&ws);
        let style = StyleParams::default();
        let budget = WriterBudget::default();
        let rd = compile(&ws, &fp, &style, &["素材词".into()], &budget, 11);

        let all_directives: Vec<_> = rd.rhythm.iter().chain(rd.lexical.iter()).cloned().collect();
        let dir_ids = directive_ids(&all_directives);
        let patch_ids = allow_patch_ids(&rd.allow);

        for d in &all_directives {
            match d {
                Directive::Split { id, children, .. } => {
                    assert!(rd.allow.split_children.contains_key(id));
                    assert_eq!(rd.allow.split_children[id], *children);
                    assert!(!rd.allow.replace.contains(id));
                }
                Directive::Merge { keep, absorb, .. } => {
                    assert!(rd.allow.replace.contains(keep));
                    assert!(rd.allow.tombstone_on_apply.contains(absorb));
                }
                Directive::Extend { id, .. }
                | Directive::Rewrite { id, .. }
                | Directive::Promote { id, .. } => {
                    assert!(rd.allow.replace.contains(id));
                }
                Directive::Demote { in_sentences, .. } => {
                    for id in in_sentences {
                        assert!(rd.allow.replace.contains(id));
                    }
                }
            }
        }

        for id in &patch_ids {
            assert!(
                dir_ids.contains(id),
                "allow-set id {id} not referenced by any directive"
            );
        }

        assert!(
            rd.allow.replace.is_subset(&patch_ids),
            "replace set should not exceed patchable ids"
        );
    }

    #[test]
    fn render_directives_zh_snapshot() {
        let directives = vec![
            Directive::Split {
                id: SentenceId("s03".into()),
                children: (SentenceId("s03a".into()), SentenceId("s03b".into())),
                short_bin: "10字以内".into(),
                gain: 0.04,
            },
            Directive::Extend {
                id: SentenceId("s09".into()),
                bin: "约35–55字".into(),
                weave: Some("风险引擎".into()),
                gain: 0.03,
            },
            Directive::Promote {
                id: SentenceId("s12".into()),
                replace: "影响".into(),
                with_any_of: vec!["冲击".into(), "波及".into()],
                gain: 0.02,
            },
        ];

        let rendered = render_directives_zh(&directives);
        let expected = "\
PATCH DIRECTIVES

[节奏]
- SPLIT s03 → s03a, s03b | 短侧：10字以内
- EXTEND s09 | 约35–55字 | 可自然融入：风险引擎

[词汇]
- PROMOTE s12 | 将「影响」替换为「冲击、波及」之一

规则：仅输出上述句子；未点名的句子不得出现在输出中。";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn overshoot_pushes_beyond_target() {
        assert_eq!(overshoot_target(40, 20), 14);
        assert_eq!(overshoot_target(10, 50), 62);
    }

    #[test]
    fn length_bin_ranges() {
        assert_eq!(length_bin(8), LengthBin::XShort);
        assert_eq!(length_bin(15), LengthBin::Short);
        assert_eq!(length_bin(30), LengthBin::Medium);
        assert_eq!(length_bin(45), LengthBin::Long);
        assert_eq!(length_bin(70), LengthBin::XLong);
    }
}
