//! MCP subtex tools — local user session only; the project directory is the
//! source of truth, the index store under the app-data dir is derived state.
//! Nothing here ever writes into the project directory.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use app_bootstrap::AppState;
use common::AppError;
use serde_json::{json, Value};
use subtex_core::managed_section::{find_managed_section, managed_section_body};
use subtex_core::scanner::{file_kind, scan_dir, scan_diff};
use subtex_core::RootHandle;
use subtex_index::outline::render_outline;
use subtex_index::{CloudEmbedder, Indexer, TextEmbedder};
use subtex_store_sqlite::SubtexStore;

use crate::auth_guard::require_user_session;
use crate::mcp::catalog;

const CONVENTION_DRAFT_TEMPLATE: &str =
    include_str!("../../../../../prompts/subtex/convention-draft.md");
const CORRECTION_DRAFT_TEMPLATE: &str =
    include_str!("../../../../../prompts/subtex/correction-draft.md");

const SEARCH_DEFAULT_LIMIT: usize = 8;
const SEARCH_MAX_LIMIT: usize = 25;
const OUTLINE_DEFAULT_TOKEN_BUDGET: usize = 1500;
const HIT_CONTENT_MAX_CHARS: usize = 1200;
const AGENTS_MD_FILE: &str = "AGENTS.md";
const AGENTS_MD_EXCERPT_LINES: usize = 40;

fn require_subtex_user(state: &AppState) -> Result<(), AppError> {
    require_user_session(
        state.auth(),
        "subtex tools require a signed-in local user session (CONTEXT_OS_USER_TOKEN / desktop session), not a workspace API key",
    )
}

fn subtex_root(arguments: &Value) -> Result<RootHandle, AppError> {
    let raw = arguments
        .get("root")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if raw.is_empty() {
        return Err(AppError::validation(
            "root_required",
            "root is required: the absolute path of the project directory",
        ));
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::validation(
            "root_must_be_absolute",
            "root must be an absolute directory path",
        ));
    }
    RootHandle::open(&path).map_err(|e| AppError::validation("root_unavailable", format!("{e}")))
}

fn open_store(handle: &RootHandle) -> Result<Arc<SubtexStore>, AppError> {
    SubtexStore::open(handle)
        .map(Arc::new)
        .map_err(|e| AppError::internal_code("subtex_store_open", format!("{e}")))
}

/// Read-side tools never create a store: a directory becomes attached only
/// via `subtex.init` (PRD F1 — no writes on un-init'd roots).
fn open_existing_store(handle: &RootHandle) -> Result<Arc<SubtexStore>, AppError> {
    if !handle.db_path().is_file() {
        return Err(AppError::validation(
            "root_not_initialized",
            format!(
                "no index store for {}; call subtex.init first",
                handle.root().display()
            ),
        ));
    }
    open_store(handle)
}

fn scan_root(root: &std::path::Path) -> Result<Vec<subtex_core::scanner::ScannedFile>, AppError> {
    scan_dir(root).map_err(|e| AppError::internal_code("subtex_scan", format!("{e}")))
}

/// First indexing pass so search answers immediately after init (small local
/// corpora); subtexd owns incremental updates afterwards.
async fn initial_index(
    root: &std::path::Path,
    store: Arc<SubtexStore>,
    files: &[subtex_core::scanner::ScannedFile],
) -> Result<subtex_index::DiffOutcome, AppError> {
    let embedder = CloudEmbedder::with_store_ledger(store.clone())
        .map(|embedder| Arc::new(embedder) as Arc<dyn TextEmbedder>);
    let indexer = Indexer::new(store, embedder);
    let diff = scan_diff(&HashMap::new(), files);
    indexer
        .index_diff(root, &diff)
        .await
        .map_err(|e| AppError::internal_code("subtex_initial_index", format!("{e:#}")))
}

fn type_distribution(files: &[subtex_core::scanner::ScannedFile]) -> BTreeMap<&'static str, usize> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for file in files {
        let path = PathBuf::from(&file.rel_path);
        *counts.entry(file_kind(&path).as_str()).or_default() += 1;
    }
    counts
}

fn top_level_entries(root: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    names.sort();
    names
}

fn read_agents_md(root: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(root.join(AGENTS_MD_FILE)).ok()
}

fn fill_template(template: &str, pairs: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (key, value) in pairs {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

/// Template files carry a usage note above a `---` separator and the actual
/// draft below; only the draft part goes into the writable field.
fn split_template(template: &str) -> (String, String) {
    match template.find("\n---\n") {
        Some(pos) => (
            template[..pos].trim().to_string(),
            template[pos + "\n---\n".len()..].trim().to_string(),
        ),
        None => (String::new(), template.trim().to_string()),
    }
}

fn convention_draft_parts(
    root: &std::path::Path,
    distribution: &BTreeMap<&'static str, usize>,
    top_level: &[String],
) -> (String, String) {
    let facts = format!(
        "- 文件总数：{}\n- 类型分布：{}\n- 顶层结构：{}",
        distribution.values().sum::<usize>(),
        distribution
            .iter()
            .map(|(kind, count)| format!("{kind} {count}"))
            .collect::<Vec<_>>()
            .join("，"),
        if top_level.is_empty() {
            "（空目录）".to_string()
        } else {
            top_level.join("，")
        }
    );
    let (note, template) = split_template(CONVENTION_DRAFT_TEMPLATE);
    let draft = fill_template(
        &template,
        &[
            ("root_name", root.display().to_string()),
            ("directory_facts", facts),
            (
                "output_locations",
                "（Agent 依据上方目录事实填写：成品与原料各放哪里）".to_string(),
            ),
            (
                "no_write_zones",
                "（Agent 依据上方目录事实填写：哪些位置不放新文件）".to_string(),
            ),
        ],
    );
    (note, draft)
}

fn index_readiness_json(store: &SubtexStore) -> Value {
    let summary = store
        .readiness_summary()
        .unwrap_or_default();
    json!({
        "vector_available": store.vector_available(),
        "files": summary.files,
        "lexical_ready": summary.lexical_ready,
        "outline_ready": summary.outline_ready,
        "vector_ready": summary.vector_ready,
    })
}

/// F1: directory facts + convention draft + first index pass, in one call.
pub(crate) async fn subtex_init(state: &AppState, arguments: &Value) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let root = handle.root().to_path_buf();
    let store = open_store(&handle)?;

    let files = scan_root(&root)?;
    let distribution = type_distribution(&files);
    let top_level = top_level_entries(&root);
    let existing = read_agents_md(&root);
    let managed_present = existing
        .as_deref()
        .and_then(find_managed_section)
        .is_some();

    let outcome = initial_index(&root, store.clone(), &files).await?;
    let unsupported: Vec<Value> = outcome
        .indexed
        .iter()
        .filter_map(|(path, o)| match o {
            subtex_index::FileIndexOutcome::Unsupported { reason } => {
                Some(json!({ "path": path, "reason": reason }))
            }
            _ => None,
        })
        .collect();
    let indexed_count = outcome
        .indexed
        .iter()
        .filter(|(_, o)| matches!(o, subtex_index::FileIndexOutcome::Indexed { .. }))
        .count();

    let (usage_note, draft) = convention_draft_parts(&root, &distribution, &top_level);
    Ok(catalog::success_result(
        "subtex.init",
        None,
        json!({
            "root": root.display().to_string(),
            "store_dir": handle.store_dir().display().to_string(),
            "files_scanned": files.len(),
            "index_result": {
                "indexed_files": indexed_count,
                "unsupported": unsupported,
                "failed": outcome.failed,
                "audio_deferred": outcome.indexed.iter()
                    .filter(|(_, o)| matches!(o, subtex_index::FileIndexOutcome::AudioDeferred))
                    .count(),
            },
            "file_type_distribution": distribution,
            "existing_structure": top_level,
            "existing_conventions": {
                "agents_md_exists": existing.is_some(),
                "subtex_managed_section_present": managed_present,
                "agents_md_excerpt": existing.as_deref().map(|content| {
                    content.lines().take(AGENTS_MD_EXCERPT_LINES).collect::<Vec<_>>().join("\n")
                }),
            },
            "convention_draft": draft,
            "convention_draft_usage_note": usage_note,
            "index_readiness": index_readiness_json(&store),
        }),
        vec![
            "Review the convention draft and merge it into AGENTS.md, touching only the content between the subtex:begin / subtex:end markers.",
            "Existing AGENTS.md content outside the markers must stay byte-identical.",
        ],
    ))
}

/// N4/F3: machine-readable readiness, jobs, usage for one directory.
pub(crate) async fn subtex_status(state: &AppState, arguments: &Value) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let store = open_existing_store(&handle)?;

    let summary = store
        .readiness_summary()
        .map_err(|e| AppError::internal_code("subtex_status", format!("{e}")))?;
    let pending_jobs = store
        .pending_job_count()
        .map_err(|e| AppError::internal_code("subtex_status", format!("{e}")))?;
    let usage: Vec<Value> = store
        .usage_totals()
        .map_err(|e| AppError::internal_code("subtex_status", format!("{e}")))?
        .into_iter()
        .map(|total| {
            json!({
                "kind": total.kind,
                "model": total.model,
                "unit": total.unit,
                "quantity": total.quantity,
            })
        })
        .collect();

    Ok(catalog::success_result(
        "subtex.status",
        None,
        json!({
            "root": handle.root().display().to_string(),
            "index_readiness": {
                "vector_available": store.vector_available(),
                "files": summary.files,
                "lexical_ready": summary.lexical_ready,
                "outline_ready": summary.outline_ready,
                "vector_ready": summary.vector_ready,
            },
            "pending_jobs": pending_jobs,
            "usage_totals": usage,
        }),
        vec![],
    ))
}

/// F1: regenerate the convention draft as the directory evolves.
pub(crate) async fn subtex_convention_draft(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let store = open_existing_store(&handle)?;
    let root = handle.root().to_path_buf();

    let files = scan_root(&root)?;
    let distribution = type_distribution(&files);
    let top_level = top_level_entries(&root);
    let existing = read_agents_md(&root);
    let (usage_note, draft) = convention_draft_parts(&root, &distribution, &top_level);

    Ok(catalog::success_result(
        "subtex.convention_draft",
        None,
        json!({
            "root": root.display().to_string(),
            "file_type_distribution": distribution,
            "existing_structure": top_level,
            "agents_md_exists": existing.is_some(),
            "convention_draft": draft,
            "convention_draft_usage_note": usage_note,
            "index_readiness": index_readiness_json(&store),
        }),
        vec!["Review the draft and write it back via the managed markers only."],
    ))
}

/// F5: turn one correction into an updated managed-section draft.
pub(crate) async fn subtex_correction_draft(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let root = handle.root().to_path_buf();

    let correction = arguments
        .get("correction")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if correction.is_empty() {
        return Err(AppError::validation(
            "correction_required",
            "correction is required: state the rule in one or two sentences",
        ));
    }

    let existing = read_agents_md(&root);
    let (usage_note, template) = split_template(CORRECTION_DRAFT_TEMPLATE);
    let current_body = match existing.as_deref().and_then(managed_section_body) {
        Some(body) => body,
        None => {
            // No managed section yet: seed the update from the convention
            // skeleton so the first correction becomes init-draft + rule.
            let files = scan_root(&root)?;
            let distribution = type_distribution(&files);
            let top_level = top_level_entries(&root);
            let (note, draft) = convention_draft_parts(&root, &distribution, &top_level);
            let _ = note;
            managed_section_body(&draft).unwrap_or_default()
        }
    };
    let draft = fill_template(
        &template,
        &[
            ("current_section_body", current_body),
            ("correction_rule", correction),
        ],
    );

    Ok(catalog::success_result(
        "subtex.correction_draft",
        None,
        json!({
            "root": root.display().to_string(),
            "agents_md_exists": existing.is_some(),
            "correction_draft": draft,
            "correction_draft_usage_note": usage_note,
        }),
        vec!["Review the updated section and write it back, keeping everything outside the markers untouched."],
    ))
}

/// F3/F4: hybrid retrieval with matched_via, provenance, and readiness.
pub(crate) async fn subtex_search(state: &AppState, arguments: &Value) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let store = open_existing_store(&handle)?;

    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if query.is_empty() {
        return Err(AppError::validation("query_required", "query is required"));
    }
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(SEARCH_DEFAULT_LIMIT)
        .clamp(1, SEARCH_MAX_LIMIT);

    let query_vector = if store.vector_available() {
        match CloudEmbedder::with_store_ledger(store.clone()) {
            Some(embedder) => match embedder.embed(&[query.clone()]).await {
                Ok(mut vectors) => vectors.pop(),
                Err(e) => {
                    tracing::warn!("subtex query embedding failed (lexical only): {e:#}");
                    None
                }
            },
            None => None,
        }
    } else {
        None
    };

    let result = store
        .search_hybrid(&query, query_vector.as_deref(), limit)
        .map_err(|e| AppError::internal_code("subtex_search", format!("{e}")))?;

    let hits: Vec<Value> = result
        .hits
        .iter()
        .map(|hit| {
            let matched_via = match (hit.lexical_rank.is_some(), hit.vector_rank.is_some()) {
                (true, true) => "lexical+vector",
                (true, false) => "lexical",
                (false, true) => "vector",
                (false, false) => "none",
            };
            let mut content = hit.hit.content.clone();
            if content.chars().count() > HIT_CONTENT_MAX_CHARS {
                content = content.chars().take(HIT_CONTENT_MAX_CHARS).collect::<String>() + "…";
            }
            json!({
                "path": hit.hit.path,
                "line_start": hit.hit.line_start.map(|l| l + 1),
                "line_end": hit.hit.line_end.map(|l| l + 1),
                "heading_path": hit.hit.heading_path,
                "content": content,
                "matched_via": matched_via,
                "rrf_score": hit.rrf_score,
                "lexical_rank": hit.lexical_rank,
                "vector_rank": hit.vector_rank,
            })
        })
        .collect();

    let truncated = result.hits.len() >= limit;
    Ok(catalog::success_result(
        "subtex.search",
        None,
        json!({
            "query": query,
            "hits": hits,
            "truncated": truncated,
            "truncation_note": truncated.then(|| format!(
                "hits truncated at limit {limit}; refine the query or raise limit (max {SEARCH_MAX_LIMIT})"
            )),
            "index_readiness": {
                "vector_available": result.vector_available,
                "lexical_used": result.lexical_used,
                "vector_used": result.vector_used,
                "files": result.readiness.files,
                "lexical_ready": result.readiness.lexical_ready,
                "outline_ready": result.readiness.outline_ready,
                "vector_ready": result.readiness.vector_ready,
            },
        }),
        vec![],
    ))
}

/// F4: token-budgeted global outline.
pub(crate) async fn subtex_outline(state: &AppState, arguments: &Value) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let store = open_existing_store(&handle)?;

    let token_budget = arguments
        .get("token_budget")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(OUTLINE_DEFAULT_TOKEN_BUDGET)
        .clamp(64, 8192);

    let outline = render_outline(&store, token_budget)
        .map_err(|e| AppError::internal_code("subtex_outline", format!("{e}")))?;

    Ok(catalog::success_result(
        "subtex.outline",
        None,
        json!({
            "root": handle.root().display().to_string(),
            "outline": outline.text,
            "token_count": outline.token_count,
            "token_budget": token_budget,
            "files": outline.files,
            "truncated": outline.truncated,
            "index_readiness": index_readiness_json(&store),
        }),
        vec![],
    ))
}

/// F2 front-end: transcription job list, submission, and batch confirmation.
pub(crate) async fn subtex_transcribe(
    state: &AppState,
    arguments: &Value,
) -> Result<Value, AppError> {
    require_subtex_user(state)?;
    let handle = subtex_root(arguments)?;
    let store = open_existing_store(&handle)?;
    let root = handle.root().to_path_buf();

    if let Some(paths) = arguments.get("paths").and_then(Value::as_array) {
        for value in paths {
            let rel = value.as_str().unwrap_or_default().trim().to_string();
            if rel.is_empty() {
                continue;
            }
            let rel = subtex_core::rel_under_root(&root, &rel).map_err(|e| {
                AppError::validation("path_outside_root", format!("{e}"))
            })?;
            let abs = root.join(&rel);
            if subtex_core::scanner::file_kind(&abs) != subtex_core::scanner::FileKind::Audio {
                return Err(AppError::validation(
                    "not_audio",
                    format!("{rel} is not an audio file"),
                ));
            }
            subtex_index::audio::enqueue_transcribe_job(&store, &root, &rel)
                .map_err(|e| AppError::internal_code("subtex_transcribe_enqueue", format!("{e:#}")))?;
        }
    }

    let jobs = store
        .jobs_of_kind("transcribe")
        .map_err(|e| AppError::internal_code("subtex_transcribe", format!("{e}")))?;
    let threshold = subtex_index::CONFIRM_THRESHOLD_SECS;
    let mut unconfirmed_total = 0.0f64;
    let job_rows: Vec<Value> = jobs
        .iter()
        .map(|job| {
            let duration = job
                .payload
                .as_ref()
                .and_then(|p| p.get("duration_secs"))
                .and_then(Value::as_f64);
            if job.state == "needs_confirmation" {
                unconfirmed_total += duration.unwrap_or(0.0);
            }
            json!({
                "path": job.file_path,
                "state": job.state,
                "duration_secs": duration,
                "attempts": job.attempts,
                "last_error": job.last_error,
            })
        })
        .collect();

    let confirm = arguments
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut confirmed_now = 0;
    if confirm {
        for job in &jobs {
            if job.state == "needs_confirmation" {
                store
                    .set_job_state(job.id, "pending")
                    .map_err(|e| AppError::internal_code("subtex_transcribe", format!("{e}")))?;
                confirmed_now += 1;
            }
        }
    }

    Ok(catalog::success_result(
        "subtex.transcribe",
        None,
        json!({
            "root": root.display().to_string(),
            "jobs": job_rows,
            "unconfirmed_total_secs": unconfirmed_total,
            "threshold_secs": threshold,
            "needs_confirmation": unconfirmed_total > threshold && !confirm,
            "confirmed_now": confirmed_now,
            "write_back_dir": subtex_index::TRANSCRIPTS_DIR,
        }),
        vec![],
    ))
}
