//! Arm-C feedforward generator: AR(1) length schedule and v1 §7 Phase A briefs.

use anyhow::{Context, Result};
use rand::RngCore;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::llm::WriterLlm;
use crate::metrics::analyze_sentences;
use crate::score::{L_MAX, L_MIN};
use crate::skeleton::{Skeleton, SkeletonSection};
use crate::workspace::{DraftWorkspace, RhythmMode};
use crate::StyleParams;

const FEEDFORWARD_SYSTEM: &str = "\
你是中文长文写作助手。严格按段落任务中的逐句长度与内容意图写作，\
保持语义连贯，输出自然流畅的正文。";

/// Open-loop arm-C drafting: one LLM call per paragraph using v1 §7 briefs.
pub async fn draft_feedforward(
    llm: &WriterLlm,
    topic: &str,
    skeleton: &Skeleton,
    style: &StyleParams,
    ws: &mut DraftWorkspace,
    seed: u64,
) -> Result<()> {
    let mut accumulated = String::new();
    let mut paragraph_idx = 0usize;

    for (section_idx, section) in skeleton.sections.iter().enumerate() {
        let paragraphs = section_paragraph_plans(section);
        for (para_in_section, rhythm) in paragraphs.into_iter().enumerate() {
            paragraph_idx += 1;
            let sentence_count = estimate_sentence_count(section, paragraphs_len(section));
            let schedule = ar1_schedule(
                sentence_count,
                style,
                seed
                    .wrapping_add(section_idx as u64 * 1_000)
                    .wrapping_add(para_in_section as u64 * 17),
            );
            let sentences = schedule
                .into_iter()
                .enumerate()
                .map(|(i, target_length)| FeedforwardSentence {
                    target_length,
                    content_intent: if i == 0 {
                        format!("围绕「{}」展开", section.heading)
                    } else {
                        "承接上文，推进论述".to_string()
                    },
                    rare_words: vec![],
                })
                .collect::<Vec<_>>();

            let brief = feedforward_brief(topic, paragraph_idx, &accumulated, &sentences);
            let (prose, _tokens) = llm
                .prose(FEEDFORWARD_SYSTEM, &brief, 0.7)
                .await
                .with_context(|| {
                    format!(
                        "feedforward paragraph {paragraph_idx} (section {})",
                        section_idx + 1
                    )
                })?;

            ws.append_section(prose.trim(), &[rhythm]);
            accumulated = ws.render_plain();
        }
    }

    Ok(())
}

fn section_paragraph_plans(section: &SkeletonSection) -> Vec<RhythmMode> {
    if section.paragraphs.is_empty() {
        vec![RhythmMode::Mixed]
    } else {
        section.paragraphs.iter().map(|p| p.rhythm).collect()
    }
}

fn paragraphs_len(section: &SkeletonSection) -> usize {
    section.paragraphs.len().max(1)
}

fn estimate_sentence_count(section: &SkeletonSection, num_paragraphs: usize) -> usize {
    let per_para = section.target_chars / num_paragraphs.max(1);
    (per_para / 20).clamp(3, 8)
}

/// Count planned LLM calls for dry-run reporting.
pub fn count_feedforward_calls(skeleton: &Skeleton) -> usize {
    skeleton
        .sections
        .iter()
        .map(|s| paragraphs_len(s))
        .sum()
}

/// Count planned LLM calls for skeleton + section drafting.
pub fn count_section_draft_calls(skeleton: &Skeleton) -> usize {
    skeleton.sections.len()
}

/// Build a deterministic skeleton placeholder for dry-run (no LLM).
pub fn stub_skeleton(topic: &str, target_chars: usize) -> Skeleton {
    Skeleton {
        title: topic.to_string(),
        sections: vec![
            SkeletonSection {
                heading: "引言".into(),
                key_points: vec!["背景".into(), "问题".into()],
                card_refs: vec![],
                target_chars: target_chars / 3,
                paragraphs: vec![crate::skeleton::ParagraphPlan {
                    rhythm: RhythmMode::Mixed,
                }],
            },
            SkeletonSection {
                heading: "主体".into(),
                key_points: vec!["分析".into(), "案例".into()],
                card_refs: vec![],
                target_chars: target_chars / 3,
                paragraphs: vec![
                    crate::skeleton::ParagraphPlan {
                        rhythm: RhythmMode::ShortBurst,
                    },
                    crate::skeleton::ParagraphPlan {
                        rhythm: RhythmMode::LongFlow,
                    },
                ],
            },
            SkeletonSection {
                heading: "结语".into(),
                key_points: vec!["总结".into()],
                card_refs: vec![],
                target_chars: target_chars / 3,
                paragraphs: vec![crate::skeleton::ParagraphPlan {
                    rhythm: RhythmMode::Mixed,
                }],
            },
        ],
    }
}

/// Fingerprint a finished draft workspace.
pub fn fingerprint_workspace(ws: &DraftWorkspace) -> crate::metrics::FingerprintReport {
    let sentences: Vec<(String, usize)> = ws
        .live()
        .map(|s| (s.text.clone(), s.para))
        .collect();
    analyze_sentences(&sentences)
}

/// One sentence slot in a feedforward paragraph brief (v1 §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedforwardSentence {
    pub target_length: usize,
    pub content_intent: String,
    pub rare_words: Vec<String>,
}

/// AR(1) log-space sentence-length schedule (v1 §3.1/§6).
///
/// `var_z = ln(1+cv²)`; `σ_ε = sqrt(var_z·(1−φ²))`; `μ_z = ln(median)`; `z_0 = μ_z`.
/// Each step: `z_i = μ_z·(1−φ) + φ·z_{i−1} + ε_i`, then `l_i = clamp(round(exp(z_i)), 5, 100)`.
pub fn ar1_schedule(n: usize, style: &StyleParams, seed: u64) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }

    let var_z = (1.0 + style.cv * style.cv).ln();
    let sigma_eps = (var_z * (1.0 - style.phi * style.phi)).sqrt();
    let mu_z = style.median_length.ln();
    let intercept = mu_z * (1.0 - style.phi);

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let l_min = L_MIN.round() as usize;
    let l_max = L_MAX.round() as usize;

    let mut z = mu_z;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let eps: f64 = sample_standard_normal(&mut rng) * sigma_eps;
        z = intercept + style.phi * z + eps;
        let len = z.exp().round().clamp(L_MIN, L_MAX) as usize;
        out.push(len.clamp(l_min, l_max));
    }
    out
}

/// Box–Muller draw from N(0, 1).
fn sample_standard_normal(rng: &mut ChaCha8Rng) -> f64 {
    let u1 = (rng.next_u64() as f64 / u64::MAX as f64).max(f64::MIN_POSITIVE);
    let u2 = rng.next_u64() as f64 / u64::MAX as f64;
    (-2.0_f64 * u1.ln()).sqrt() * (2.0_f64 * std::f64::consts::PI * u2).cos()
}

/// v1 §7 length-bin prose description for a target character count.
pub fn length_bin(chars: usize) -> &'static str {
    match chars {
        0..=10 => "极短，一个断句，干脆利落",
        11..=20 => "短句，简洁有力",
        21..=35 => "中等长度，正常陈述",
        36..=55 => "长句，包含从句或列举",
        _ => "极长，复合句，信息密集",
    }
}

/// Build the per-paragraph Phase A prompt (v1 §7).
pub fn feedforward_brief(
    topic: &str,
    paragraph_idx: usize,
    accumulated_text: &str,
    sentences: &[FeedforwardSentence],
) -> String {
    let mut out = format!("你正在写一篇关于{topic}的文章。\n\n");

    if accumulated_text.trim().is_empty() {
        out.push_str("[Context] 已写内容：（尚无）\n\n");
    } else {
        out.push_str("[Context] 已写内容：\n");
        out.push_str(accumulated_text.trim());
        out.push_str("\n\n");
    }

    out.push_str(&format!(
        "[Task] 请写第{paragraph_idx}段，共{}句话。\n",
        sentences.len()
    ));
    out.push_str("本段每句话的长度要求：\n");

    for (idx, slot) in sentences.iter().enumerate() {
        let n = idx + 1;
        out.push_str(&format!(
            "  第{n}句：{}（约{}字）— 内容意图：{}\n",
            length_bin(slot.target_length),
            slot.target_length,
            slot.content_intent
        ));
    }

    let vocab_lines: Vec<String> = sentences
        .iter()
        .enumerate()
        .filter_map(|(idx, slot)| {
            if slot.rare_words.is_empty() {
                None
            } else {
                Some(format!(
                    "  第{}句请自然地包含以下词语：{}",
                    idx + 1,
                    slot.rare_words.join("、")
                ))
            }
        })
        .collect();

    if !vocab_lines.is_empty() {
        out.push_str("\n词汇要求：\n");
        for line in vocab_lines {
            out.push_str(&line);
            out.push('\n');
        }
    }

    out.push_str("\n要求：句子之间语义连贯，内容自然流畅。");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::length_metrics;

    #[test]
    fn ar1_schedule_is_deterministic_for_seed() {
        let style = StyleParams::default();
        let a = ar1_schedule(50, &style, 42);
        let b = ar1_schedule(50, &style, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn ar1_schedule_clamps_to_bounds() {
        let style = StyleParams {
            cv: 1.2,
            phi: 0.9,
            median_length: 80.0,
            ..StyleParams::default()
        };
        for len in ar1_schedule(200, &style, 7) {
            assert!((5..=100).contains(&len));
        }
    }

    #[test]
    fn ar1_schedule_statistical_properties() {
        let style = StyleParams::default();
        let lengths = ar1_schedule(10_000, &style, 99);
        let (_, cv, autocorr, _) = length_metrics(&lengths);

        assert!(
            (cv - style.cv).abs() < 0.1,
            "empirical CV {cv} not within ±0.1 of target {}",
            style.cv
        );
        assert!(
            (autocorr - style.phi).abs() < 0.1,
            "empirical autocorr {autocorr} not within ±0.1 of target {}",
            style.phi
        );
    }

    #[test]
    fn length_bin_descriptions_match_v1_ranges() {
        assert_eq!(length_bin(8), "极短，一个断句，干脆利落");
        assert_eq!(length_bin(15), "短句，简洁有力");
        assert_eq!(length_bin(30), "中等长度，正常陈述");
        assert_eq!(length_bin(45), "长句，包含从句或列举");
        assert_eq!(length_bin(70), "极长，复合句，信息密集");
    }

    #[test]
    fn feedforward_brief_renders_sentence_lines() {
        let brief = feedforward_brief(
            "城市更新",
            2,
            "第一段已写内容。",
            &[
                FeedforwardSentence {
                    target_length: 8,
                    content_intent: "点出主题".to_string(),
                    rare_words: vec![],
                },
                FeedforwardSentence {
                    target_length: 32,
                    content_intent: "展开论据".to_string(),
                    rare_words: vec!["存量改造".to_string()],
                },
            ],
        );

        assert!(brief.contains("你正在写一篇关于城市更新的文章。"));
        assert!(brief.contains("已写内容：\n第一段已写内容。"));
        assert!(brief.contains("请写第2段，共2句话。"));
        assert!(brief.contains("第1句：极短，一个断句，干脆利落（约8字）— 内容意图：点出主题"));
        assert!(brief.contains("第2句：中等长度，正常陈述（约32字）— 内容意图：展开论据"));
        assert!(brief.contains("第2句请自然地包含以下词语：存量改造"));
        assert!(brief.ends_with("要求：句子之间语义连贯，内容自然流畅。"));
    }

    #[test]
    fn feedforward_brief_empty_context_placeholder() {
        let brief = feedforward_brief(
            "测试",
            1,
            "   ",
            &[FeedforwardSentence {
                target_length: 20,
                content_intent: "开篇".to_string(),
                rare_words: vec![],
            }],
        );
        assert!(brief.contains("已写内容：（尚无）"));
    }
}
