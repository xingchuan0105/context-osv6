//! CLI: M3 three-arm drafting experiment runner (spec §16 GATE 2).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use heavytail::draft::draft_sections;
use heavytail::experiment::{
    evaluate_m3_decisions, render_summary_markdown, ArmSummary, TopicRunResult,
};
use heavytail::feedforward::{
    count_feedforward_calls, count_section_draft_calls, draft_feedforward, fingerprint_workspace,
    stub_skeleton,
};
use heavytail::llm::WriterLlm;
use heavytail::score::composite;
use heavytail::skeleton::plan_skeleton;
use heavytail::validator::validate;
use heavytail::workspace::DraftWorkspace;
use heavytail::StyleParams;

const DEFAULT_TOPICS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/experiment-topics.txt");
const DEFAULT_OUT: &str = "heavytail-out";
const DEFAULT_TARGET_CHARS: usize = 1800;
const R4_LINE_AB_TOPIC_COUNT: usize = 3;

#[derive(Debug, Clone)]
struct Config {
    topics_path: PathBuf,
    arms: Vec<char>,
    out_root: PathBuf,
    dry_run: bool,
    limit_topics: Option<usize>,
    refine_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmKind {
    A,
    B,
    C,
    BLines,
}

impl ArmKind {
    fn dir_name(self) -> &'static str {
        match self {
            ArmKind::A => "arm-a",
            ArmKind::B => "arm-b",
            ArmKind::C => "arm-c",
            ArmKind::BLines => "arm-b-lines",
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err:#}");
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cfg = parse_args()?;

    if let Some(refine_dir) = cfg.refine_dir {
        return run_refine_stub(&refine_dir);
    }

    let topics = load_topics(&cfg.topics_path, cfg.limit_topics)?;
    if topics.is_empty() {
        bail!("no topics loaded from {}", cfg.topics_path.display());
    }

    let run_id = timestamp_run_id();
    let run_dir = cfg.out_root.join(&run_id);
    fs::create_dir_all(&run_dir)?;

    if cfg.dry_run {
        print_dry_run_plan(&topics, &cfg.arms, &run_dir);
        return Ok(());
    }

    let llm = WriterLlm::from_env().context("WriterLlm::from_env (set AGENT_LLM_* in .env)")?;
    let style = StyleParams::default();

    let mut arm_a_results = Vec::new();
    let mut arm_b_results = Vec::new();
    let mut arm_c_results = Vec::new();
    let mut arm_b_lines_results = Vec::new();

    for (idx, topic) in topics.iter().enumerate() {
        let topic_num = idx + 1;
        eprintln!("[{topic_num}/{}] {topic}", topics.len());

        if cfg.arms.contains(&'a') {
            let result = run_topic_arm(
                &llm,
                &run_dir,
                topic,
                topic_num,
                ArmKind::A,
                &style,
                DEFAULT_TARGET_CHARS,
            )
            .await?;
            arm_a_results.push(result);
        }

        if cfg.arms.contains(&'b') {
            let result = run_topic_arm(
                &llm,
                &run_dir,
                topic,
                topic_num,
                ArmKind::B,
                &style,
                DEFAULT_TARGET_CHARS,
            )
            .await?;
            arm_b_results.push(result);

            if topic_num <= R4_LINE_AB_TOPIC_COUNT {
                let lines = run_topic_arm(
                    &llm,
                    &run_dir,
                    topic,
                    topic_num,
                    ArmKind::BLines,
                    &style,
                    DEFAULT_TARGET_CHARS,
                )
                .await?;
                arm_b_lines_results.push(lines);
            }
        }

        if cfg.arms.contains(&'c') {
            let result = run_topic_arm(
                &llm,
                &run_dir,
                topic,
                topic_num,
                ArmKind::C,
                &style,
                DEFAULT_TARGET_CHARS,
            )
            .await?;
            arm_c_results.push(result);
        }
    }

    let summary_a = ArmSummary::from_results("a", &arm_a_results);
    let summary_b = ArmSummary::from_results("b", &arm_b_results);
    let summary_c = ArmSummary::from_results("c", &arm_c_results);
    let summary_b_lines = if arm_b_lines_results.is_empty() {
        None
    } else {
        Some(ArmSummary::from_results("b-lines", &arm_b_lines_results))
    };

    let b_cv = summary_b.mean_cv;
    let b_hapax = summary_b.mean_hapax;
    let decisions = evaluate_m3_decisions(&summary_a, &summary_b, &summary_c);
    let mut summary_md = render_summary_markdown(
        &run_id,
        &topics,
        &[summary_a, summary_b, summary_c],
        summary_b_lines.as_ref(),
        &decisions,
    );

    if let Some(ref bl) = summary_b_lines {
        summary_md.push_str(&format!(
            "\n## R4 prose vs line-per-sentence (arm b, first {} topics)\n\n\
             Free prose mean CV {b_cv:.4} vs lines mean CV {:.4}; \
             hapax {b_hapax:.4} vs {:.4}.\n",
            bl.topic_count,
            bl.mean_cv,
            bl.mean_hapax,
        ));
    }

    fs::write(run_dir.join("summary.md"), summary_md)?;

    eprintln!("Wrote {}", run_dir.join("summary.md").display());
    for v in &decisions.verdicts {
        eprintln!("VERDICT: {}", v.verdict);
    }

    Ok(())
}

fn run_refine_stub(refine_dir: &Path) -> Result<()> {
    eprintln!(
        "heavytail-experiment --refine: not implemented (M4). \
         Would load drafts from {} and append refinement results to summary.md.",
        refine_dir.display()
    );
    Ok(())
}

async fn run_topic_arm(
    llm: &WriterLlm,
    run_dir: &Path,
    topic: &str,
    topic_num: usize,
    arm: ArmKind,
    style: &StyleParams,
    target_chars: usize,
) -> Result<TopicRunResult> {
    let arm_dir = run_dir.join(arm.dir_name());
    fs::create_dir_all(&arm_dir)?;

    let mut tokens_used = 0;
    let skeleton = plan_skeleton(llm, topic, target_chars, &[], &mut tokens_used)
        .await
        .with_context(|| format!("plan_skeleton topic {topic_num} arm {}", arm.dir_name()))?;

    let mut ws = DraftWorkspace::new();
    match arm {
        ArmKind::A => {
            draft_sections(
                llm,
                &skeleton,
                style,
                &[],
                &mut ws,
                false,
                false,
                None,
                false,
                &mut tokens_used,
                None,
            )
            .await?;
        }
        ArmKind::B => {
            draft_sections(
                llm,
                &skeleton,
                style,
                &[],
                &mut ws,
                true,
                true,
                None,
                false,
                &mut tokens_used,
                None,
            )
            .await?;
        }
        ArmKind::BLines => {
            draft_sections(
                llm,
                &skeleton,
                style,
                &[],
                &mut ws,
                true,
                true,
                None,
                true,
                &mut tokens_used,
                None,
            )
            .await?;
        }
        ArmKind::C => {
            let seed = topic_seed(topic, topic_num);
            draft_feedforward(llm, topic, &skeleton, style, &mut ws, seed).await?;
        }
    }

    let draft_text = ws.render_plain();
    let fp = fingerprint_workspace(&ws);
    let score = composite(&fp, style);
    let validation = validate(&fp, style);

    let draft_name = format!("topic-{topic_num:02}.draft.txt");
    let fp_name = format!("topic-{topic_num:02}.fingerprint.json");
    let draft_path = arm_dir.join(&draft_name);
    fs::write(&draft_path, &draft_text)?;
    fs::write(
        arm_dir.join(&fp_name),
        serde_json::to_string_pretty(&fp)?,
    )?;

    Ok(TopicRunResult {
        topic_idx: topic_num,
        topic: topic.to_string(),
        draft_path: format!("{}/{draft_name}", arm.dir_name()),
        fingerprint: fp,
        score,
        validation,
    })
}

fn topic_seed(topic: &str, topic_num: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    topic.hash(&mut h);
    topic_num.hash(&mut h);
    h.finish()
}

fn load_topics(path: &Path, limit: Option<usize>) -> Result<Vec<String>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read topics file {}", path.display()))?;
    let mut topics: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect();
    if let Some(n) = limit {
        topics.truncate(n);
    }
    Ok(topics)
}

fn timestamp_run_id() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn print_dry_run_plan(topics: &[String], arms: &[char], run_dir: &Path) {
    let skeleton = stub_skeleton("示例主题", DEFAULT_TARGET_CHARS);
    let sk_calls = 1;
    let section_calls = count_section_draft_calls(&skeleton);
    let ff_calls = count_feedforward_calls(&skeleton);

    eprintln!("DRY-RUN: would write artifacts under {}", run_dir.display());
    eprintln!("Topics: {}", topics.len());
    eprintln!("Arms: {}", arms.iter().collect::<String>());

    for (idx, topic) in topics.iter().enumerate() {
        let n = idx + 1;
        for arm in arms {
            match *arm {
                'a' | 'b' => {
                    eprintln!(
                        "  topic-{n:02} arm-{arm}: plan_skeleton x{sk_calls} + draft_sections x{section_calls} [{topic}]"
                    );
                }
                'c' => {
                    eprintln!(
                        "  topic-{n:02} arm-c: plan_skeleton x{sk_calls} + draft_feedforward x{ff_calls} [{topic}]"
                    );
                }
                _ => eprintln!("  topic-{n:02} arm-{arm}: unknown arm (skipped)"),
            }
            if *arm == 'b' && n <= R4_LINE_AB_TOPIC_COUNT {
                eprintln!(
                    "  topic-{n:02} arm-b-lines: plan_skeleton x{sk_calls} + draft_sections x{section_calls} (one sentence per line)"
                );
            }
        }
    }

    let total_sk = topics.len() * arms.len();
    let mut total_draft = 0usize;
    for arm in arms {
        match *arm {
            'a' | 'b' => total_draft += topics.len() * section_calls,
            'c' => total_draft += topics.len() * ff_calls,
            _ => {}
        }
    }
    let lines_extra = arms.contains(&'b') as usize * topics.len().min(R4_LINE_AB_TOPIC_COUNT) * section_calls;
    eprintln!(
        "Estimated LLM calls: skeleton ~{total_sk}, drafting ~{}, lines A/B extra ~{lines_extra}, total ~{}",
        total_draft,
        total_sk + total_draft + lines_extra
    );
}

fn parse_args() -> Result<Config> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        process::exit(if args.is_empty() { 1 } else { 0 });
    }

    if args.first().map(String::as_str) == Some("--refine") {
        let dir = args
            .get(1)
            .map(PathBuf::from)
            .context("usage: heavytail-experiment --refine <heavytail-out/<ts>>")?;
        return Ok(Config {
            topics_path: PathBuf::from(DEFAULT_TOPICS),
            arms: vec![],
            out_root: PathBuf::from(DEFAULT_OUT),
            dry_run: false,
            limit_topics: None,
            refine_dir: Some(dir),
        });
    }

    let mut topics_path = PathBuf::from(DEFAULT_TOPICS);
    let mut arms = vec!['a', 'b', 'c'];
    let mut out_root = PathBuf::from(DEFAULT_OUT);
    let mut dry_run = false;
    let mut limit_topics = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--topics" => {
                i += 1;
                topics_path = PathBuf::from(args.get(i).context("--topics requires a path")?);
            }
            "--arms" => {
                i += 1;
                let raw = args.get(i).context("--arms requires a,b,c list")?;
                arms = parse_arms(raw)?;
            }
            "--out" => {
                i += 1;
                out_root = PathBuf::from(args.get(i).context("--out requires a directory")?);
            }
            "--dry-run" => dry_run = true,
            "--limit-topics" => {
                i += 1;
                limit_topics = Some(
                    args.get(i)
                        .context("--limit-topics requires a number")?
                        .parse()
                        .context("--limit-topics must be a positive integer")?,
                );
            }
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }

    if arms.is_empty() {
        bail!("at least one arm required");
    }

    Ok(Config {
        topics_path,
        arms,
        out_root,
        dry_run,
        limit_topics,
        refine_dir: None,
    })
}

fn parse_arms(raw: &str) -> Result<Vec<char>> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let arm = part.trim();
        if arm.is_empty() {
            continue;
        }
        let c = arm
            .chars()
            .next()
            .context("empty arm token")?;
        if !matches!(c, 'a' | 'b' | 'c') {
            bail!("invalid arm {c:?}; expected a, b, or c");
        }
        if !out.contains(&c) {
            out.push(c);
        }
    }
    if out.is_empty() {
        bail!("no valid arms in {raw:?}");
    }
    Ok(out)
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  heavytail-experiment [--topics <file>] [--arms a,b,c] [--out heavytail-out]");
    eprintln!("                       [--dry-run] [--limit-topics N]");
    eprintln!("  heavytail-experiment --refine <heavytail-out/<ts>>   (M4 stub)");
    eprintln!();
    eprintln!("Arms:");
    eprintln!("  a  plain skeleton + sections (no priming, no MPC hints)");
    eprintln!("  b  priming + MPC deficit hints (+ R4 line-per-sentence on first 3 topics)");
    eprintln!("  c  v1 feedforward Phase A briefs");
}
