//! MPC sectionwise drafting: deficit hints + sequential free-prose sections (spec §9).

use anyhow::{Context, Result};

use crate::llm::WriterLlm;
use crate::metrics::{analyze_sentences, FingerprintReport};
use crate::score::quantile_targets;
use crate::skeleton::{MaterialCard, Skeleton, SkeletonSection};
use crate::workspace::{DraftWorkspace, RhythmMode};
use crate::StyleParams;

/// Writing-style priming (Task 18 moves this to the capability skill slot).
pub const PRIMING: &str = "\
长短交错；不避 10 字以内的极短句；偶用 50 字以上的复合长句；\
少用高频套话；优先具体名词、数字与术语。";

const PLAIN_SYSTEM: &str = "你是中文长文写作助手。按任务要求输出自然流畅的正文。";

const SHORT_SENTENCE_CHARS: usize = 10;
const LONG_SENTENCE_CHARS: usize = 50;
const MAX_DEFICIT_HINTS: usize = 3;
const RESERVOIR_HINT_TERMS: usize = 5;

/// ≤ 3 hints from the running fingerprint vs style targets, rendered in Chinese.
pub fn deficit_hints(
    fp: &FingerprintReport,
    style: &StyleParams,
    reservoir: &[String],
) -> Vec<String> {
    if fp.sentence_lengths.is_empty() {
        return Vec::new();
    }

    let targets = quantile_targets(fp.sentence_lengths.len(), style);
    let mut hints = Vec::new();

    let expected_short = targets
        .iter()
        .filter(|&&t| t <= SHORT_SENTENCE_CHARS as f64)
        .count();
    let actual_short = fp
        .sentence_lengths
        .iter()
        .filter(|&&l| l <= SHORT_SENTENCE_CHARS)
        .count();
    if actual_short < expected_short {
        let need = (expected_short - actual_short).max(1);
        hints.push(format!(
            "目前全文短句偏少：本段安排至少 {need} 句 {SHORT_SENTENCE_CHARS} 字以内的短句"
        ));
    }

    let expected_long = targets
        .iter()
        .filter(|&&t| t >= LONG_SENTENCE_CHARS as f64)
        .count();
    let actual_long = fp
        .sentence_lengths
        .iter()
        .filter(|&&l| l >= LONG_SENTENCE_CHARS)
        .count();
    if actual_long < expected_long && hints.len() < MAX_DEFICIT_HINTS {
        let need = (expected_long - actual_long).max(1);
        hints.push(format!(
            "目前全文长句偏少：本段安排至少 {need} 句 {LONG_SENTENCE_CHARS} 字以上的复合长句"
        ));
    }

    if hints.len() < MAX_DEFICIT_HINTS {
        let terms: Vec<String> = reservoir
            .iter()
            .filter(|term| !term.is_empty() && !fp.word_freq.contains_key(term.as_str()))
            .take(RESERVOIR_HINT_TERMS)
            .cloned()
            .collect();
        if !terms.is_empty() {
            hints.push(format!("可自然使用的词：{}", terms.join("、")));
        }
    }

    hints.truncate(MAX_DEFICIT_HINTS);
    hints
}

/// Draft each skeleton section sequentially.
///
/// - `mpc`: deficit hints (M3 arm a vs b)
/// - `primed`: style priming in system prompt (arm a = false, arm b = true)
/// - `priming`: override priming text (or `None` to use [`PRIMING`] when primed)
/// - `one_sentence_per_line`: R4 line-per-sentence drafting variant on arm b
/// - `on_section`: called at each section boundary with `(section_index_1based, total_sections)`
pub async fn draft_sections(
    llm: &WriterLlm,
    skeleton: &Skeleton,
    style: &StyleParams,
    cards: &[MaterialCard],
    ws: &mut DraftWorkspace,
    mpc: bool,
    primed: bool,
    priming: Option<&str>,
    one_sentence_per_line: bool,
    tokens_used: &mut usize,
    on_section: Option<&dyn Fn(usize, usize)>,
) -> Result<()> {
    let card_map: std::collections::BTreeMap<&str, &MaterialCard> =
        cards.iter().map(|c| (c.id.as_str(), c)).collect();
    let reservoir = build_reservoir(cards);

    let mut section_prose: Vec<String> = Vec::new();
    let mut running_fp = fingerprint_from_workspace(ws);
    let total_sections = skeleton.sections.len();
    let priming_text = priming.unwrap_or(PRIMING);

    for (idx, section) in skeleton.sections.iter().enumerate() {
        if let Some(callback) = on_section {
            callback(idx + 1, total_sections);
        }

        let user = build_section_brief(
            skeleton,
            idx,
            section,
            &section_prose,
            &card_map,
            if mpc {
                deficit_hints(&running_fp, style, &reservoir)
            } else {
                Vec::new()
            },
            one_sentence_per_line,
        );

        let system = if primed {
            format!("{PLAIN_SYSTEM}\n\n{priming_text}")
        } else {
            PLAIN_SYSTEM.to_string()
        };
        let (prose, tokens) = llm
            .prose(&system, &user, 0.7)
            .await
            .with_context(|| format!("draft section {} ({})", idx + 1, section.heading))?;
        *tokens_used += tokens as usize;

        let rhythms: Vec<RhythmMode> = section
            .paragraphs
            .iter()
            .map(|p| p.rhythm)
            .collect();
        ws.append_section(prose.trim(), &rhythms);
        section_prose.push(prose.trim().to_string());
        running_fp = fingerprint_from_workspace(ws);
    }

    Ok(())
}

fn build_reservoir(cards: &[MaterialCard]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for card in cards {
        for term in &card.rare_terms {
            if seen.insert(term.clone()) {
                out.push(term.clone());
            }
        }
    }
    out
}

fn fingerprint_from_workspace(ws: &DraftWorkspace) -> FingerprintReport {
    let sentences: Vec<(String, usize)> = ws
        .live()
        .map(|s| (s.text.clone(), s.para))
        .collect();
    analyze_sentences(&sentences)
}

fn build_section_brief(
    skeleton: &Skeleton,
    section_idx: usize,
    section: &SkeletonSection,
    prior_prose: &[String],
    cards: &std::collections::BTreeMap<&str, &MaterialCard>,
    deficit: Vec<String>,
    one_sentence_per_line: bool,
) -> String {
    let skeleton_json =
        serde_json::to_string_pretty(skeleton).unwrap_or_else(|_| "{}".to_string());

    let mut context = String::new();
    context.push_str("【骨架（全文固定）】\n");
    context.push_str(&skeleton_json);
    context.push('\n');

    if section_idx > 0 {
        context.push_str("\n【已完成章节摘要】\n");
        for (i, prose) in prior_prose.iter().enumerate() {
            if i + 1 == section_idx {
                context.push_str(&format!(
                    "上一节（第 {} 节）全文：\n{prose}\n",
                    i + 1
                ));
            } else if i + 1 < section_idx {
                let summary = one_line_summary(prose);
                context.push_str(&format!(
                    "第 {} 节摘要：{summary}\n",
                    i + 1
                ));
            }
        }
    }

    context.push_str("\n【本节素材】\n");
    if section.card_refs.is_empty() {
        context.push_str("（无指定卡片）\n");
    } else {
        for id in &section.card_refs {
            if let Some(card) = cards.get(id.as_str()) {
                context.push_str(&format!(
                    "- [{id}] {}：{}\n",
                    format_kind(&card.kind),
                    card.content
                ));
            }
        }
    }

    let rhythms: Vec<_> = section
        .paragraphs
        .iter()
        .map(|p| rhythm_label(p.rhythm))
        .collect();

    let mut task = format!(
        "\n【任务】撰写第 {} / {} 节「{}」\n\
         要点：{}\n\
         目标篇幅：约 {} 字\n\
         段落节奏：{}\n",
        section_idx + 1,
        skeleton.sections.len(),
        section.heading,
        section.key_points.join("；"),
        section.target_chars,
        rhythms.join("、")
    );

    if !deficit.is_empty() {
        task.push_str("\n【节奏提示】\n");
        for hint in deficit {
            task.push_str("- ");
            task.push_str(&hint);
            task.push('\n');
        }
    }

    task.push_str(
        "\n请直接输出本节正文（自由散文，不要标题、不要句子编号、不要 markdown）。\
         按段落用空行分隔；每句以 。！？ 之一结尾。",
    );
    if one_sentence_per_line {
        task.push_str(" 每句单独占一行（一行一句）。");
    }

    format!("{context}{task}")
}

fn one_line_summary(prose: &str) -> String {
    let flat: String = prose.split_whitespace().collect();
    let max = 80usize;
    if flat.chars().count() <= max {
        flat
    } else {
        flat.chars().take(max).collect::<String>() + "…"
    }
}

fn format_kind(kind: &crate::skeleton::MaterialKind) -> &'static str {
    use crate::skeleton::MaterialKind;
    match kind {
        MaterialKind::Fact => "事实",
        MaterialKind::Quote => "引述",
        MaterialKind::Figure => "数据",
        MaterialKind::Term => "术语",
        MaterialKind::Inspiration => "灵感",
    }
}

fn rhythm_label(mode: RhythmMode) -> &'static str {
    match mode {
        RhythmMode::ShortBurst => "短句爆发",
        RhythmMode::LongFlow => "长句铺陈",
        RhythmMode::Mixed => "混合",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::char_len;
    use crate::skeleton::ParagraphPlan;

    fn uniform_fp(n: usize, len: usize) -> FingerprintReport {
        let text = "中".repeat(len) + "。";
        let sentences: Vec<(String, usize)> = (0..n).map(|i| (text.clone(), i)).collect();
        analyze_sentences(&sentences)
    }

    #[test]
    fn deficit_hints_empty_for_empty_draft() {
        let fp = FingerprintReport {
            sentence_lengths: vec![],
            mean_length: 0.0,
            cv: 0.0,
            autocorr_lag1: 0.0,
            lognormal_ks_stat: 0.0,
            total_tokens: 0,
            vocab_size: 0,
            ttr: 0.0,
            hapax_ratio: 0.0,
            zipf_exponent: 0.0,
            word_freq: Default::default(),
        };
        let hints = deficit_hints(&fp, &StyleParams::default(), &["术语".into()]);
        assert!(hints.is_empty());
    }

    #[test]
    fn deficit_hints_short_sentence_for_uniform_draft() {
        let fp = uniform_fp(12, 20);
        assert!(
            fp.sentence_lengths.iter().all(|&l| l > SHORT_SENTENCE_CHARS),
            "fixture should be uniform mid-length"
        );
        let hints = deficit_hints(&fp, &StyleParams::default(), &[]);
        assert!(
            hints.iter().any(|h| h.contains("短句偏少")),
            "expected short-sentence hint, got: {hints:?}"
        );
        assert!(hints.len() <= MAX_DEFICIT_HINTS);
    }

    #[test]
    fn deficit_hints_reservoir_terms() {
        let mut fp = uniform_fp(8, 20);
        fp.word_freq.insert("已用词".into(), 3);
        let hints = deficit_hints(
            &fp,
            &StyleParams::default(),
            &["量子纠缠".into(), "已用词".into()],
        );
        assert!(
            hints.iter().any(|h| h.contains("量子纠缠") && !h.contains("已用词")),
            "expected reservoir hint with unused term, got: {hints:?}"
        );
    }

    #[test]
    fn one_line_summary_truncates() {
        let long = "字".repeat(120);
        let s = one_line_summary(&long);
        assert!(s.chars().count() <= 81);
    }

    #[test]
    fn char_len_fixture_uniform_twenty() {
        let s = "中".repeat(19) + "。";
        assert_eq!(char_len(&s), 20);
    }

    #[tokio::test]
    #[ignore = "requires live AGENT_LLM API; run with --ignored --nocapture"]
    async fn draft_sections_smoke() {
        let llm = WriterLlm::from_env().expect("from_env");
        let skeleton = Skeleton {
            title: "测试".into(),
            sections: vec![SkeletonSection {
                heading: "引言".into(),
                key_points: vec!["背景".into(), "问题".into()],
                card_refs: vec![],
                target_chars: 200,
                paragraphs: vec![ParagraphPlan {
                    rhythm: RhythmMode::Mixed,
                }],
            }],
        };
        let mut ws = DraftWorkspace::new();
        draft_sections(
            &llm,
            &skeleton,
            &StyleParams::default(),
            &[],
            &mut ws,
            true,
            true,
            None,
            false,
            &mut 0,
            None,
        )
        .await
        .expect("draft_sections");
        assert!(!ws.render_plain().is_empty());
    }
}
