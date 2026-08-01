//! rejudge — 离线重判 JUDGE_ERROR 题(2026-08-01 M1)。
//!
//! 读 run 目录的 `q*.artifact.json` 中持久化的 `judge_input` 快照,重建
//! `JudgeInput`,重新调 judge(共享 cache + 退避重试),重算 label,更新
//! artifact 的 `score_v2` 与 `judge.json`。答案不重生成——JUDGE_ERROR 题
//! 的答案已完整,只差判分。
//!
//! 用法(JUDGE_LLM_* 凭据在 .env,与 product_e2e 同款):
//! ```bash
//! cd avrag-rs
//! set -a && source .env && set +a
//! cargo run -p rag_quality --bin rejudge -- \
//!   crates/app/tests/e2e_output/rag_eval_v2/v2_20260801-112850
//! # 只重判指定题:
//! cargo run -p rag_quality --bin rejudge -- <run_dir> q133 q067
//! ```

use std::path::PathBuf;

use rag_quality::eval_v2::aggregate::{LabelInput, label_for};
use rag_quality::eval_v2::{
    self, ContextSource, JudgeCache, JudgeClient, JudgeInput, JudgeOutput, JudgeStatus,
    JudgeThresholds, ScoreV2,
};
use serde_json::Value;

/// 与 product_e2e 的 live_judge_call 同款:cached miss → live call,
/// transport 错误指数退避(1s/2s)最多 3 次,JSON parse 失败重试一次。
async fn judge_attempt(
    judge: &JudgeClient,
    cache: &JudgeCache,
    messages: &[avrag_llm::ChatMessage],
    input: &JudgeInput,
) -> (JudgeStatus, Option<JudgeOutput>, Option<String>, String) {
    let key = JudgeCache::key(judge.model(), input);
    if let Some(raw) = cache.load(&key, judge.model(), input) {
        if let Ok(parsed) = eval_v2::parse_judge_output(&raw) {
            return (
                JudgeStatus::Ok,
                Some(parsed),
                Some(raw),
                "cache_hit".to_string(),
            );
        }
    }
    let complete = || judge.complete(messages);
    let resp = {
        let mut last_err: Option<String> = None;
        let mut out = None;
        for attempt in 0..3 {
            match complete().await {
                Ok(r) => {
                    out = Some(r);
                    break;
                }
                Err(e) if attempt < 2 => {
                    let wait = 1u64 << attempt;
                    eprintln!(
                        "  judge transport error ({e}); retry {}/3 after {wait}s",
                        attempt + 1
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    last_err = Some(e.to_string());
                }
                Err(e) => {
                    last_err = Some(e.to_string());
                    break;
                }
            }
        }
        match out {
            Some(r) => r,
            None => {
                return (
                    JudgeStatus::Error,
                    None,
                    None,
                    format!("judge transport error after 3 attempts: {:?}", last_err),
                );
            }
        }
    };
    let raw = resp.content;
    match eval_v2::parse_judge_output(&raw) {
        Ok(parsed) => {
            cache.store(&key, judge.model(), input, &raw);
            (JudgeStatus::Ok, Some(parsed), Some(raw), "ok".to_string())
        }
        Err(first_err) => {
            eprintln!("  judge JSON parse failed ({first_err}); retrying once");
            match complete().await {
                Ok(resp2) => {
                    let raw2 = resp2.content;
                    match eval_v2::parse_judge_output(&raw2) {
                        Ok(parsed) => {
                            cache.store(&key, judge.model(), input, &raw2);
                            (
                                JudgeStatus::Ok,
                                Some(parsed),
                                Some(raw2),
                                "ok_after_retry".to_string(),
                            )
                        }
                        Err(e2) => (
                            JudgeStatus::Error,
                            None,
                            Some(raw2),
                            format!("judge parse failed after retry: {e2}"),
                        ),
                    }
                }
                Err(e) => (
                    JudgeStatus::Error,
                    None,
                    Some(raw),
                    format!("judge retry transport error (first parse: {first_err}): {e}"),
                ),
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: rejudge <run_dir> [qnum...]");
        std::process::exit(2);
    }
    let run_dir = PathBuf::from(&args[1]);
    let only: Vec<String> = args[2..].to_vec();
    if !run_dir.is_dir() {
        anyhow::bail!("run dir not found: {}", run_dir.display());
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let judge = JudgeClient::from_env()?;
        let cache_dir = run_dir.parent().expect("run dir parent").join("cache");
        let cache = JudgeCache::new(&cache_dir);
        println!("judge model = {}", judge.model());
        println!("run dir = {}", run_dir.display());
        println!("cache dir = {}", cache_dir.display());

        let mut entries: Vec<PathBuf> = std::fs::read_dir(&run_dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".artifact.json"))
            })
            .collect();
        entries.sort();

        let mut rejudged = 0usize;
        let mut skipped = 0usize;
        for path in entries {
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let qnum = fname.trim_end_matches(".artifact.json").to_string();
            if !only.is_empty() && !only.contains(&qnum) {
                continue;
            }
            let artifact: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
            let Some(ji) = artifact.get("judge_input") else {
                eprintln!("{qnum}: no judge_input snapshot (pre-M1 artifact); skip");
                skipped += 1;
                continue;
            };
            let judge_input: JudgeInput = serde_json::from_value(ji.clone())?;
            let old_score: ScoreV2 = serde_json::from_value(artifact["score_v2"].clone())?;
            if old_score.judge_status != JudgeStatus::Error {
                eprintln!(
                    "{qnum}: judge_status={:?}, skip (only JUDGE_ERROR rejudged)",
                    old_score.judge_status
                );
                continue;
            }
            let messages = vec![
                avrag_llm::ChatMessage::system(eval_v2::SYSTEM_PROMPT.to_string()),
                avrag_llm::ChatMessage::user(eval_v2::build_user_prompt(&judge_input)),
            ];
            let (status, parsed, _raw, note) =
                judge_attempt(&judge, &cache, &messages, &judge_input).await;
            if status != JudgeStatus::Ok || parsed.is_none() {
                eprintln!("{qnum}: still {status:?} ({note})");
                skipped += 1;
                continue;
            }
            let parsed = parsed.unwrap();
            let label = label_for(&LabelInput {
                has_infra_error: false,
                judge_status: JudgeStatus::Ok,
                gold_exists: old_score.selection.golden_count > 0,
                no_context: judge_input.context_source == ContextSource::NoContext,
                expect_no_retrieval: judge_input.expect_no_retrieval,
                expected_should_answer: judge_input.expected_should_answer,
                retrieval_recall: old_score.retrieval.recall,
                cited_gold_hits: old_score.selection.golden_matched_in_cited,
                judge: Some(&parsed),
                thresholds: &JudgeThresholds::default(),
            });
            let new_score = ScoreV2 {
                query: old_score.query.clone(),
                subset: old_score.subset.clone(),
                retrieval: old_score.retrieval.clone(),
                selection: old_score.selection.clone(),
                judge: Some(parsed.clone()),
                judge_status: JudgeStatus::Ok,
                label,
                reference_answer: old_score.reference_answer.clone(),
                model_answer: old_score.model_answer.clone(),
                context_source: judge_input.context_source,
                expect_no_retrieval: judge_input.expect_no_retrieval,
            };
            let mut updated = artifact.clone();
            updated["score_v2"] = serde_json::to_value(&new_score)?;
            updated["judge_status"] = serde_json::json!("ok");
            updated["judge_label"] = serde_json::json!(label.as_str());
            std::fs::write(&path, serde_json::to_string_pretty(&updated)?)?;
            // 同步更新 judge.json(判分产物)。
            let judge_path = path.with_file_name(format!("{qnum}.judge.json"));
            if judge_path.exists() {
                let mut jj: Value = serde_json::from_str(&std::fs::read_to_string(&judge_path)?)?;
                jj["judge_status"] = serde_json::json!("ok");
                jj["note"] = serde_json::json!(note);
                jj["parsed"] = serde_json::to_value(&parsed)?;
                std::fs::write(&judge_path, serde_json::to_string_pretty(&jj)?)?;
            }
            println!(
                "{qnum}: {:?} -> {:?} (corr={} faith={}, {note})",
                old_score.label, label, parsed.answer_correctness.score, parsed.faithfulness.score
            );
            rejudged += 1;
        }
        println!("rejudged={rejudged} skipped={skipped}");
        Ok(())
    })
}
