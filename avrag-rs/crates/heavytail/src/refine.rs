//! Refinement round runner: analyze → compile → rhythm/lexical passes → record (spec §10).

use anyhow::{Context, Result};

use crate::compiler::{compile, render_directives_zh, Directive, RoundDirectives};
use crate::llm::WriterLlm;
use crate::metrics::{analyze_sentences, FingerprintReport};
use crate::patch::{apply_patch, parse_patch, AllowSet, PatchError};
use crate::score::composite;
use crate::segment::char_len;
use crate::state::{ComplianceRecord, RoundRecord, WriterBudget, WriterPhase, WriterState};
use crate::validator::validate;
use crate::workspace::{DraftWorkspace, SentenceId};
use crate::StyleParams;

const PATCH_SYSTEM: &str = "\
你是中文写作修订助手。根据 PATCH DIRECTIVES 修改指定句子，仅输出 patch 格式行。\n\
每行格式：s<id>| <一句完整中文句子>\n\
每行必须且只能包含一个完整句子，以。！？之一结尾。\n\
不要输出未在 DIRECTIVES 中点名的句子。不要输出解释、标题或 markdown 围栏。";

const PATCH_TEMPERATURE: f32 = 0.3;

/// Run the evaluator-optimizer refinement loop until validation passes or budget is exhausted.
pub async fn refine(
    llm: &WriterLlm,
    ws: &mut DraftWorkspace,
    style: &StyleParams,
    reservoir: &[String],
    budget: &WriterBudget,
    state: &mut WriterState,
    on_round: Option<&(dyn Fn(usize, usize) + Sync)>,
) -> Result<()> {
    for round in 0..budget.max_rounds {
        if let Some(callback) = on_round {
            callback(round + 1, budget.max_rounds);
        }

        state.phase = WriterPhase::Refining { round: round + 1 };

        let fp = fingerprint_from_workspace(ws);
        if validate(&fp, style).passed {
            state.phase = WriterPhase::Done;
            return Ok(());
        }

        let seed = state.rounds.len() as u64 + round as u64 + 1;
        let directives = compile(ws, &fp, style, reservoir, budget, seed);
        let pre_fp = fp.clone();

        let (rhythm_patch, rhythm_applied) = run_patch_pass(
            llm,
            ws,
            &directives.rhythm,
            &directives.allow,
            "rhythm",
            &mut state.tokens_used,
        )
        .await?;

        let lexical_allow = if rhythm_applied {
            recompile_lexical_allow(ws, &pre_fp, style, reservoir, budget, seed)
        } else {
            directives.allow.clone()
        };

        let (lexical_patch, _lexical_applied) = run_patch_pass(
            llm,
            ws,
            &directives.lexical,
            &lexical_allow,
            "lexical",
            &mut state.tokens_used,
        )
        .await?;

        let post_fp = fingerprint_from_workspace(ws);
        let score = composite(&post_fp, style);
        let compliance = check_compliance(ws, &directives, &pre_fp, &post_fp);

        let patch_raw = combine_patch_raw(&rhythm_patch, &lexical_patch);
        let directives_json =
            serde_json::to_string(&directives).context("serialize round directives")?;

        state.record_round(RoundRecord {
            fingerprint: post_fp.clone(),
            directives_json,
            patch_raw,
            compliance,
            score,
        });

        if validate(&post_fp, style).passed {
            state.phase = WriterPhase::Done;
            return Ok(());
        }
    }

    state.phase = WriterPhase::Validating;
    Ok(())
}

fn fingerprint_from_workspace(ws: &DraftWorkspace) -> FingerprintReport {
    let sentences: Vec<(String, usize)> = ws
        .live()
        .map(|s| (s.text.clone(), s.para))
        .collect();
    analyze_sentences(&sentences)
}

/// After rhythm edits, lexical allow-set must reflect the updated workspace.
fn recompile_lexical_allow(
    ws: &DraftWorkspace,
    fp: &FingerprintReport,
    style: &StyleParams,
    reservoir: &[String],
    budget: &WriterBudget,
    seed: u64,
) -> AllowSet {
    compile(ws, fp, style, reservoir, budget, seed).allow
}

async fn run_patch_pass(
    llm: &WriterLlm,
    ws: &mut DraftWorkspace,
    directives: &[Directive],
    allow: &AllowSet,
    pass_name: &str,
    tokens_used: &mut usize,
) -> Result<(String, bool)> {
    if directives.is_empty() {
        return Ok((String::new(), false));
    }

    let canonical = ws.render_canonical();
    let directive_block = render_directives_zh(directives);
    let mut user = format!("{canonical}\n\n{directive_block}\n\n请仅输出 patch 行。");

    match fetch_and_apply_patch(llm, ws, allow, &user, tokens_used).await {
        Ok(raw) => Ok((raw, true)),
        Err(first_err) => {
            user.push_str("\n\n上一次 patch 解析失败：");
            user.push_str(&patch_error_message(&first_err));
            user.push_str("\n请修正后仅输出合法 patch 行。");

            match fetch_and_apply_patch(llm, ws, allow, &user, tokens_used).await {
                Ok(raw) => Ok((raw, true)),
                Err(second_err) => {
                    tracing::warn!(
                        pass = pass_name,
                        error = %patch_error_message(&second_err),
                        "skipping patch pass after retry"
                    );
                    Ok((String::new(), false))
                }
            }
        }
    }
}

async fn fetch_and_apply_patch(
    llm: &WriterLlm,
    ws: &mut DraftWorkspace,
    allow: &AllowSet,
    user: &str,
    tokens_used: &mut usize,
) -> Result<String, PatchError> {
    let (raw, tokens) = llm
        .prose(PATCH_SYSTEM, user, PATCH_TEMPERATURE)
        .await
        .map_err(|_| PatchError::Empty)?;
    *tokens_used += tokens as usize;

    let patch_text = extract_patch_block(&raw);
    let patch = parse_patch(&patch_text, allow)?;
    apply_patch(ws, &patch, allow);
    Ok(patch_text)
}

fn combine_patch_raw(rhythm: &str, lexical: &str) -> String {
    let mut out = String::new();
    if !rhythm.is_empty() {
        out.push_str("# rhythm pass\n");
        out.push_str(rhythm);
    }
    if !lexical.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("# lexical pass\n");
        out.push_str(lexical);
    }
    out
}

fn extract_patch_block(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        let body = rest
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .trim_start();
        if let Some(end) = body.rfind("```") {
            return body[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn patch_error_message(err: &PatchError) -> String {
    match err {
        PatchError::BadLine(n) => format!("第{n}行格式错误或 split 子句不完整"),
        PatchError::UnknownId(id) => format!("未知或未授权的句子 ID {id}"),
        PatchError::TombstonedId(id) => format!("句子 ID {id} 已被合并吸收，不可输出"),
        PatchError::NotSingleSentence(id) => format!("{id} 对应文本不是单句"),
        PatchError::Empty => "patch 为空".to_string(),
    }
}

/// Parse a Chinese bin label into `(min_chars, max_chars)`.
pub fn parse_bin_range(bin: &str) -> Option<(usize, usize)> {
    if bin.contains('内') {
        let max: usize = bin
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()?;
        return Some((1, max));
    }
    if bin.contains("以上") {
        let min: usize = bin
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()?;
        return Some((min, 100));
    }

    let digits: Vec<usize> = bin
        .split(|c: char| c == '–' || c == '-' || c == '—')
        .filter_map(|part| {
            part.chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        })
        .collect();
    if digits.len() >= 2 {
        Some((digits[0], digits[1]))
    } else {
        None
    }
}

fn length_in_bin(actual: usize, bin: &str) -> bool {
    let Some((lo, hi)) = parse_bin_range(bin) else {
        return false;
    };
    let tol_lo = ((lo as f64) * 0.7).floor() as usize;
    let tol_hi = ((hi as f64) * 1.3).ceil() as usize;
    actual >= tol_lo.max(1) && actual <= tol_hi
}

fn sentence_char_len(ws: &DraftWorkspace, id: &SentenceId) -> Option<usize> {
    ws.sentences
        .iter()
        .find(|s| &s.id == id && !s.tombstone)
        .map(|s| char_len(&s.text))
}

fn is_tombstoned(ws: &DraftWorkspace, id: &SentenceId) -> bool {
    ws.sentences
        .iter()
        .find(|s| &s.id == id)
        .is_some_and(|s| s.tombstone)
}

fn check_compliance(
    ws: &DraftWorkspace,
    directives: &RoundDirectives,
    pre_fp: &FingerprintReport,
    post_fp: &FingerprintReport,
) -> Vec<ComplianceRecord> {
    let mut out = Vec::new();
    for d in directives
        .rhythm
        .iter()
        .chain(directives.lexical.iter())
    {
        out.push(compliance_for_directive(ws, d, pre_fp, post_fp));
    }
    out
}

fn compliance_for_directive(
    ws: &DraftWorkspace,
    d: &Directive,
    pre_fp: &FingerprintReport,
    post_fp: &FingerprintReport,
) -> ComplianceRecord {
    match d {
        Directive::Split {
            id,
            children,
            short_bin,
            ..
        } => {
            let len_a = sentence_char_len(ws, &children.0);
            let len_b = sentence_char_len(ws, &children.1);
            let short_len = match (len_a, len_b) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            let achieved = short_len.map(|n| format!("{n}字"));
            let complied = short_len.is_some_and(|n| length_in_bin(n, short_bin));
            ComplianceRecord {
                directive: format!("SPLIT {id}"),
                complied,
                asked: Some(short_bin.clone()),
                achieved,
            }
        }
        Directive::Merge { keep, absorb, .. } => {
            let complied = is_tombstoned(ws, absorb);
            ComplianceRecord {
                directive: format!("MERGE {keep} absorb {absorb}"),
                complied,
                asked: Some("absorb 消失".into()),
                achieved: Some(if complied {
                    "已吸收".into()
                } else {
                    "仍存在".into()
                }),
            }
        }
        Directive::Extend { id, bin, .. } | Directive::Rewrite { id, bin, .. } => {
            let len = sentence_char_len(ws, id);
            let achieved = len.map(|n| format!("{n}字"));
            let complied = len.is_some_and(|n| length_in_bin(n, bin));
            let tag = if matches!(d, Directive::Extend { .. }) {
                "EXTEND"
            } else {
                "REWRITE"
            };
            ComplianceRecord {
                directive: format!("{tag} {id}"),
                complied,
                asked: Some(bin.clone()),
                achieved,
            }
        }
        Directive::Promote {
            id,
            replace,
            with_any_of,
            ..
        } => {
            let text = ws
                .sentences
                .iter()
                .find(|s| &s.id == id && !s.tombstone)
                .map(|s| s.text.as_str());
            let replacement_used = text.is_some_and(|t| {
                with_any_of
                    .iter()
                    .any(|w| t.contains(w.as_str()) && w != replace)
            });
            let before = pre_fp.word_freq.get(replace).copied().unwrap_or(0);
            let after = post_fp.word_freq.get(replace).copied().unwrap_or(0);
            let freq_reduced = after < before;
            let complied = replacement_used && freq_reduced;
            ComplianceRecord {
                directive: format!("PROMOTE {id}"),
                complied,
                asked: Some(format!("替换「{replace}」并降频")),
                achieved: Some(format!(
                    "freq {before}→{after}{}",
                    if replacement_used { "" } else { "，未替换" }
                )),
            }
        }
        Directive::Demote {
            word,
            max_count,
            in_sentences: _,
            ..
        } => {
            let after = post_fp.word_freq.get(word).copied().unwrap_or(0);
            let complied = after <= *max_count;
            ComplianceRecord {
                directive: format!("DEMOTE 「{word}」"),
                complied,
                asked: Some(format!("至多{max_count}次")),
                achieved: Some(format!("{after}次")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::FingerprintReport;
    use crate::state::WriterState;
    use crate::workspace::{ParagraphRecord, RhythmMode, SentenceRecord};
    use std::collections::BTreeMap;

    #[test]
    fn parse_bin_range_labels() {
        assert_eq!(parse_bin_range("10字以内"), Some((1, 10)));
        assert_eq!(parse_bin_range("约10–20字"), Some((10, 20)));
        assert_eq!(parse_bin_range("约55字以上"), Some((55, 100)));
    }

    #[test]
    fn length_in_bin_applies_thirty_percent_tolerance() {
        assert!(length_in_bin(7, "10字以内"));
        assert!(length_in_bin(13, "10字以内"));
        assert!(!length_in_bin(14, "10字以内"));
        assert!(length_in_bin(7, "约10–20字"));
        assert!(length_in_bin(26, "约10–20字"));
    }

    #[test]
    fn merge_compliance_detects_tombstone() {
        let mut ws = DraftWorkspace::default();
        ws.sentences = vec![
            SentenceRecord {
                id: SentenceId("s01".into()),
                text: "保留句。".into(),
                para: 0,
                tombstone: false,
            },
            SentenceRecord {
                id: SentenceId("s02".into()),
                text: "被吸收。".into(),
                para: 0,
                tombstone: true,
            },
        ];
        ws.paragraphs = vec![ParagraphRecord {
            idx: 0,
            rhythm: RhythmMode::Mixed,
        }];

        let d = Directive::Merge {
            keep: SentenceId("s01".into()),
            absorb: SentenceId("s02".into()),
            gain: 0.1,
        };
        let fp = FingerprintReport {
            sentence_lengths: vec![5, 4],
            mean_length: 4.5,
            cv: 0.1,
            autocorr_lag1: 0.0,
            lognormal_ks_stat: 0.0,
            total_tokens: 0,
            vocab_size: 0,
            ttr: 0.0,
            hapax_ratio: 0.0,
            zipf_exponent: 1.0,
            word_freq: BTreeMap::new(),
        };
        let rd = RoundDirectives {
            rhythm: vec![d.clone()],
            lexical: vec![],
            allow: AllowSet::default(),
        };
        let records = check_compliance(&ws, &rd, &fp, &fp);
        assert_eq!(records.len(), 1);
        assert!(records[0].complied);
    }

    fn agent_llm_configured() -> bool {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let env_path = repo_root.join(".env");
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }
        ["AGENT_LLM_BASE_URL", "AGENT_LLM_API_KEY", "AGENT_LLM_MODEL"]
            .into_iter()
            .all(|key| {
                std::env::var(key)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
            })
    }

    fn uniform_workspace(n: usize) -> DraftWorkspace {
        let mut ws = DraftWorkspace::default();
        let s = "这是一句长度恰好二十字左右的示例句子。";
        let prose = (0..n).map(|_| s).collect::<Vec<_>>().join("");
        ws.append_section(&prose, &[RhythmMode::Mixed]);
        ws
    }

    #[tokio::test]
    #[ignore = "requires live AGENT_LLM API and arm-b style draft; run with --ignored"]
    async fn refine_integration_improves_bands() {
        if !agent_llm_configured() {
            eprintln!("SKIP: AGENT_LLM_* not configured");
            return;
        }

        let llm = WriterLlm::from_env().expect("from_env");
        let style = StyleParams::default();
        let mut ws = uniform_workspace(20);
        let mut state = WriterState::default();
        let budget = WriterBudget {
            max_rounds: 3,
            ..WriterBudget::default()
        };
        let reservoir = vec!["罕见术语".into(), "风险引擎".into()];

        let initial = validate(&fingerprint_from_workspace(&ws), &style);
        let initial_passing = initial
            .metric_results
            .iter()
            .filter(|m| m.passed)
            .count();

        refine(&llm, &mut ws, &style, &reservoir, &budget, &mut state, None)
            .await
            .expect("refine");

        let final_report = validate(
            &state
                .rounds
                .last()
                .map(|r| r.fingerprint.clone())
                .unwrap_or_else(|| fingerprint_from_workspace(&ws)),
            &style,
        );
        let final_passing = final_report
            .metric_results
            .iter()
            .filter(|m| m.passed)
            .count();

        assert!(
            final_passing >= initial_passing + 2,
            "expected ≥2 newly satisfied bands, got {initial_passing} → {final_passing}"
        );

        if let Some(best) = &state.best {
            let mut prev = f64::NEG_INFINITY;
            for round in &state.rounds {
                if round.score.s >= prev {
                    prev = round.score.s;
                }
            }
            assert!(
                best.score >= prev - 1e-9,
                "best score should track non-decreasing retained versions"
            );
        }
    }
}
