//! WriteRefine M4 comparison arm.
//!
//! Mirrors `heavytail-experiment --refine <run_dir>` but drives the production
//! `WriteRefineLoopRunner` (the Agent loop) instead of the legacy fixed-round
//! `heavytail::refine::refine()`. The heavytail crate cannot depend on app-chat
//! (circular dependency), so the WriteRefine arm lives here on the app-chat
//! side. Together the two binaries form the M4 A/B comparison:
//!
//! - `heavytail-experiment --refine <dir>`        → legacy guided refine (baseline)
//! - `app-chat`'s `refine-experiment --refine <dir>` → WriteRefine Agent loop
//!
//! Usage:
//!     refine-experiment --refine <heavytail-out/<ts>> [--no-budget] [--force] [--topic N]
//!
//! `--no-budget` disables token / react / revise / research caps (M4 fair-run).
//! `--force` overwrites existing outputs in the output directory.
//!
//! Inputs (same contract as the legacy arm):
//!   <run_dir>/arm-b/topic-NN.draft.txt   pre-generated free-prose drafts
//!   <run_dir>/arm-b/topic-NN.fingerprint.json  (optional) pre-refine fingerprint
//!   <run_dir>/topics.txt                  (optional) topic labels
//!
//! Outputs:
//!   <run_dir>/arm-b-refined-wr/topic-NN.draft.txt           (default budget)
//!   <run_dir>/arm-b-refined-wr-nobudget/topic-NN.draft.txt   (--no-budget)
//!   stdout: M4 exit markdown (pre/post S, bands, compliance)
//!
//! On-demand research inside the loop is disabled in this harness (a stub
//! agent service): the comparison targets the refine loop itself, and the
//! reservoir is pre-supplied by arm-b — matching the legacy arm's empty
//! reservoir. A `write_refine_research` call returns a graceful tool error.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use app_chat::agents::events::{AgentEventSink, CollectingSink};
use app_chat::agents::runtime::{Agent, AgentRequest, AgentRunResult};
use app_chat::agents::service::UnifiedAgentService;
use app_chat::agents::AgentKind;
use app_chat::writer::SubagentInvoker;
use app_chat::writer::{
    FinishReason, MaterialPack, RefineContext, RefineLoopBudget, WriteRefineLoopRunner,
};

use common::AppError;

use heavytail::diagnosis::diagnose_pre_refine;
use heavytail::experiment::{
    compliance_rate_from_rounds, evaluate_m4_exit, render_m4_markdown, M4TopicResult,
};
use heavytail::feedforward::fingerprint_workspace;
use heavytail::llm::WriterLlm;
use heavytail::experiment::build_refine_reservoir;
use heavytail::persona::{
    generate_persona, load_persona, persona_seed_default, save_persona, PersonaCard,
};
use heavytail::score::composite;
use heavytail::state::WriterState;
use heavytail::validator::validate;
use heavytail::workspace::DraftWorkspace;
use heavytail::StyleParams;

/// Stub agent that rejects every run — on-demand research is disabled in this
/// comparison harness. `handle_research` maps the error into a tool error and
/// the loop continues with revise/finish.
struct ResearchDisabledAgent;

#[async_trait::async_trait]
impl Agent for ResearchDisabledAgent {
    async fn run(
        &self,
        _request: AgentRequest,
        _sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        Err(AppError::internal(
            "research disabled in refine-experiment harness",
        ))
    }
}

struct ExperimentOptions {
    run_dir: PathBuf,
    no_budget: bool,
    force: bool,
    topic: Option<usize>,
    persona_seed: Option<u64>,
    persona_replay: Option<PathBuf>,
    no_persona: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = parse_args(std::env::args().skip(1).collect())?;
    run_refine(&opts).await
}

fn parse_args(args: Vec<String>) -> Result<ExperimentOptions> {
    let mut run_dir = None;
    let mut no_budget = false;
    let mut force = false;
    let mut topic = None;
    let mut persona_seed = None;
    let mut persona_replay = None;
    let mut no_persona = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--refine" => {
                run_dir = Some(
                    args.get(i + 1)
                        .map(PathBuf::from)
                        .context("usage: refine-experiment --refine <heavytail-out/<ts>> [--no-budget] [--force] [--topic N] [--persona-seed N] [--persona-replay PATH] [--no-persona]")?,
                );
                i += 2;
            }
            "--no-budget" => {
                no_budget = true;
                i += 1;
            }
            "--force" => {
                force = true;
                i += 1;
            }
            "--topic" => {
                topic = Some(
                    args.get(i + 1)
                        .and_then(|s| s.parse().ok())
                        .context("usage: --topic requires a 1-based topic number")?,
                );
                i += 2;
            }
            "--persona-seed" => {
                persona_seed = Some(
                    args.get(i + 1)
                        .and_then(|s| s.parse().ok())
                        .context("usage: --persona-seed requires a u64")?,
                );
                i += 2;
            }
            "--persona-replay" => {
                persona_replay = Some(
                    args.get(i + 1)
                        .map(PathBuf::from)
                        .context("usage: --persona-replay requires a path")?,
                );
                i += 2;
            }
            "--no-persona" => {
                no_persona = true;
                i += 1;
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    let run_dir = run_dir.context(
        "usage: refine-experiment --refine <heavytail-out/<ts>> [--no-budget] [--force] [--topic N] [--persona-seed N] [--persona-replay PATH] [--no-persona]",
    )?;
    Ok(ExperimentOptions {
        run_dir,
        no_budget,
        force,
        topic,
        persona_seed,
        persona_replay,
        no_persona,
    })
}

async fn run_refine(opts: &ExperimentOptions) -> Result<()> {
    let run_dir = &opts.run_dir;
    if !run_dir.is_dir() {
        bail!("--refine directory does not exist: {}", run_dir.display());
    }

    let llm = WriterLlm::from_env().context("WriterLlm::from_env (set AGENT_LLM_* in .env)")?;
    let style = StyleParams::default();

    // Research-disabled sub-worker service (see module docs).
    let service = Arc::new(UnifiedAgentService::new(Box::new(ResearchDisabledAgent)));
    let invoker = SubagentInvoker::new(service, None);

    let persona_enabled = !opts.no_persona;
    let out_dir = if persona_enabled {
        run_dir.join("arm-b-refined-wr-persona")
    } else {
        run_dir.join("arm-b-refined-wr-fix")
    };
    std::fs::create_dir_all(&out_dir)?;
    if persona_enabled {
        std::fs::create_dir_all(out_dir.join("personas"))?;
    }

    let topics = load_experiment_topics(run_dir)?;
    if opts.no_budget {
        eprintln!(
            "budget: gate harness (react_cap={}, max_revise={}, research unlimited)",
            app_chat::writer::refine_loop::WRITE_REFINE_HARD_REACT_CAP,
            app_chat::writer::refine_loop::WRITE_REFINE_GATE_MAX_REVISE,
        );
    } else {
        eprintln!("budget: default RefineLoopBudget");
    }
    eprintln!("output: {}", out_dir.display());
    if persona_enabled {
        eprintln!("persona: enabled (seed/replay per flags)");
    } else {
        eprintln!("persona: disabled (--no-persona)");
    }
    eprintln!("core band finish gate: ON (hapax/zipf must pass before finish)");
    eprintln!(
        "react hard cap: {} iterations (--no-budget still enforces this)",
        app_chat::writer::refine_loop::WRITE_REFINE_HARD_REACT_CAP
    );

    let checkpoint_dir =
        std::env::temp_dir().join(format!("heavytail-refine-exp-{}", process_id()));

    // Collect topic-NN.draft.txt files in order.
    let mut draft_paths: Vec<(usize, PathBuf)> = Vec::new();
    let arm_b = run_dir.join("arm-b");
    if !arm_b.is_dir() {
        bail!("missing arm-b/ draft directory: {}", arm_b.display());
    }
    for entry in std::fs::read_dir(&arm_b)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(num) = name
            .strip_prefix("topic-")
            .and_then(|s| s.strip_suffix(".draft.txt"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            draft_paths.push((num, entry.path()));
        }
    }
    draft_paths.sort_by_key(|(n, _)| *n);
    if let Some(only) = opts.topic {
        draft_paths.retain(|(n, _)| *n == only);
        if draft_paths.is_empty() {
            bail!("--topic {only} not found under arm-b/");
        }
    }
    if draft_paths.is_empty() {
        bail!("no topic-NN.draft.txt files found in {}", arm_b.display());
    }

    let total = draft_paths.len();
    let mut m4_results = Vec::new();

    let budget = if opts.no_budget {
        RefineLoopBudget {
            enforce_core_band_finish_gate: true,
            ..RefineLoopBudget::unlimited()
        }
    } else {
        RefineLoopBudget {
            enforce_core_band_finish_gate: true,
            ..RefineLoopBudget::default()
        }
    };

    for (topic_num, draft_path) in draft_paths {
        let topic = format!("topic-{topic_num:02}");
        let topic_title = topics
            .get(topic_num.saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| topic.clone());
        let fp_out = out_dir.join(format!("topic-{topic_num:02}.fingerprint.json"));
        if fp_out.is_file() && !opts.force {
            eprintln!("  skip {topic} (already present; pass --force to overwrite)");
            continue;
        }

        eprintln!("[{topic_num}/{total}] WriteRefine {topic}");
        let draft = std::fs::read_to_string(&draft_path)
            .with_context(|| format!("read {}", draft_path.display()))?;

        let pre_fp = fingerprint_workspace(&DraftWorkspace::from_plain(&draft));
        let pre_score = composite(&pre_fp, &style).s;

        let persona = resolve_experiment_persona(
            &llm,
            &topic_title,
            topic_num,
            opts,
            &out_dir,
        )
        .await?;

        let reservoir = build_refine_reservoir(&topic_title, &pre_fp, persona.as_ref());
        eprintln!(
            "  reservoir ({} terms): {}",
            reservoir.len(),
            reservoir.iter().take(8).cloned().collect::<Vec<_>>().join("、")
        );

        let mut ws = DraftWorkspace::from_plain(&draft);
        let diag = diagnose_pre_refine(&ws, &style, &reservoir);
        let material_pack = MaterialPack::with_reservoir(reservoir.clone());
        let mut ctx = RefineContext::new(std::mem::take(&mut ws), diag, material_pack, persona);

        let parent_request = experiment_request(&topic_title);

        let runner =
            WriteRefineLoopRunner::new(&llm, &invoker, &parent_request, style.clone(), budget.clone());
        let mut state = WriterState::default();
        let sink = CollectingSink::new();
        runner
            .run(&mut ctx, &reservoir, &mut state, &sink, &checkpoint_dir)
            .await
            .context("WriteRefine loop failed")?;

        let post_fp = fingerprint_workspace(&ctx.workspace);
        let post_score = composite(&post_fp, &style).s;
        let validation = validate(&post_fp, &style);
        let compliance_rate = compliance_rate_from_rounds(&state.rounds);

        std::fs::write(
            out_dir.join(format!("topic-{topic_num:02}.draft.txt")),
            ctx.workspace.render_plain(),
        )?;
        std::fs::write(
            &fp_out,
            serde_json::to_string_pretty(&post_fp).context("serialize fingerprint")?,
        )?;

        let finish_reason = ctx
            .finish_reason
            .map(|r: FinishReason| format!("{r:?}"))
            .unwrap_or_else(|| "none".into());
        eprintln!(
            "  {topic}: pass={} compliance={:.0}% revise_rounds={} react={} research={} tokens={} S {:.4}→{:.4} ({})",
            validation.passed,
            compliance_rate * 100.0,
            ctx.revise_rounds_used,
            ctx.react_iteration + 1,
            ctx.research_calls_used,
            ctx.tokens_used,
            pre_score,
            post_score,
            finish_reason,
        );

        m4_results.push(M4TopicResult {
            topic_idx: topic_num,
            topic,
            rounds: ctx.revise_rounds_used,
            passed: validation.passed,
            compliance_rate,
            pre_score,
            post_score,
        });
    }

    let report = evaluate_m4_exit(&m4_results);
    println!("{}", render_m4_markdown(&report));
    Ok(())
}

fn experiment_request(topic: &str) -> AgentRequest {
    AgentRequest {
        kind: AgentKind::WriteRefine,
        query: topic.to_string(),
        notebook_id: None,
        session_id: Some(format!("refine-experiment-{topic}")),
        doc_scope: vec![],
        messages: vec![],
        user_preferences: None,
        debug: false,
        stream: false,
        language: Some("zh".to_string()),
        auth_context: Value::Object(Default::default()),
        docscope_metadata: None,
        metadata: Default::default(),
        cancellation_token: None,
        guard_pipeline: None,
        preferred_tools: vec![],
        format_hint: None,
        max_iterations: None,
    }
}

fn process_id() -> u32 {
    std::process::id()
}

async fn resolve_experiment_persona(
    llm: &WriterLlm,
    topic_title: &str,
    topic_num: usize,
    opts: &ExperimentOptions,
    out_dir: &std::path::Path,
) -> Result<Option<PersonaCard>> {
    if opts.no_persona {
        return Ok(None);
    }
    let persona_path = out_dir.join(format!("personas/topic-{topic_num:02}.persona.json"));
    let persona = if let Some(path) = &opts.persona_replay {
        load_persona(path).with_context(|| format!("load persona replay {}", path.display()))?
    } else {
        let seed = opts
            .persona_seed
            .unwrap_or_else(|| persona_seed_default(topic_title));
        let mut tokens = 0usize;
        generate_persona(llm, topic_title, seed, &mut tokens)
            .await
            .context("generate persona")?
    };
    save_persona(&persona_path, &persona)
        .with_context(|| format!("save persona {}", persona_path.display()))?;
    Ok(Some(persona))
}

fn load_experiment_topics(run_dir: &std::path::Path) -> Result<Vec<String>> {
    const DEFAULT_TOPICS: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../heavytail/experiment-topics.txt"
    );
    let path = if run_dir.join("topics.txt").is_file() {
        run_dir.join("topics.txt")
    } else {
        PathBuf::from(DEFAULT_TOPICS)
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read topics from {}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}
