//! Subtex resident daemon.
//!
//! Watches every attached root (stores under the Subtex data dir are the
//! known-root registry), reconciles each root on watcher events plus a
//! periodic mtime sweep, and consumes the per-store `jobs` table. Restart is
//! safe: anything missed while the daemon was down is caught by the sweep.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::{Context, Result};
use subtex_core::RootHandle;
use subtex_index::{CloudEmbedder, Indexer, TextEmbedder};
use subtex_store_sqlite::{JobRecord, SubtexStore};

#[derive(Debug, Clone)]
pub struct SubtexdConfig {
    pub data_dir: PathBuf,
    /// Seconds between full reconcile sweeps (watcher events reconcile sooner).
    pub reconcile_secs: u64,
    pub max_jobs_per_tick: usize,
}

impl SubtexdConfig {
    pub fn from_env() -> Result<Self> {
        let data_dir = match std::env::var_os("SUBTEX_DATA_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => subtex_core::default_data_dir()?,
        };
        let env_num = |key: &str, default: usize| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Ok(Self {
            data_dir,
            reconcile_secs: env_num("SUBTEXD_RECONCILE_SECS", 5).max(1) as u64,
            max_jobs_per_tick: env_num("SUBTEXD_MAX_JOBS_PER_TICK", 16).max(1),
        })
    }
}

#[derive(Clone)]
struct RootRuntime {
    handle: RootHandle,
    store: Arc<SubtexStore>,
    indexer: Indexer,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TickReport {
    pub reconciled_roots: Vec<String>,
    /// Roots attached since the previous tick (the caller attaches a watcher).
    pub new_roots: Vec<PathBuf>,
    pub indexed_files: usize,
    pub removed_files: usize,
    pub failed: usize,
    /// Per-file/per-job failure details (path + error), capped by source.
    pub failed_details: Vec<(String, String)>,
    pub jobs_done: usize,
}

impl TickReport {
    pub fn has_activity(&self) -> bool {
        !self.reconciled_roots.is_empty()
            || self.indexed_files > 0
            || self.removed_files > 0
            || self.failed > 0
            || self.jobs_done > 0
    }
}

pub struct Subtexd {
    cfg: SubtexdConfig,
    runtimes: RwLock<Vec<RootRuntime>>,
    dirty: Mutex<HashSet<usize>>,
    last_full_reconcile: Mutex<Option<Instant>>,
}

impl Subtexd {
    pub fn new(cfg: SubtexdConfig) -> Result<Self> {
        let subtexd = Self {
            cfg,
            runtimes: RwLock::new(Vec::new()),
            dirty: Mutex::new(HashSet::new()),
            last_full_reconcile: Mutex::new(None),
        };
        subtexd.refresh_roots()?;
        subtexd.requeue_all_running()?;
        Ok(subtexd)
    }

    fn requeue_all_running(&self) -> Result<()> {
        let runtimes = self.runtimes.read().expect("subtexd runtimes");
        for rt in runtimes.iter() {
            let n = rt.store.requeue_running().context("requeue running jobs")?;
            if n > 0 {
                tracing::info!(
                    "requeued {n} running jobs for {}",
                    rt.handle.root().display()
                );
            }
        }
        Ok(())
    }

    pub fn root_paths(&self) -> Vec<PathBuf> {
        self.runtimes
            .read()
            .expect("subtexd runtimes")
            .iter()
            .map(|rt| rt.handle.root().to_path_buf())
            .collect()
    }

    pub fn store_for(&self, root: &Path) -> Option<Arc<SubtexStore>> {
        self.runtimes
            .read()
            .expect("subtexd runtimes")
            .iter()
            .find(|rt| rt.handle.root() == root)
            .map(|rt| rt.store.clone())
    }

    /// (Re)read `<data>/roots/*/root.txt` — the known-root registry. Returns
    /// the paths of roots attached since the previous call.
    pub fn refresh_roots(&self) -> Result<Vec<PathBuf>> {
        let discovered = discover_roots(&self.cfg.data_dir)?;
        let known: HashSet<String> = self
            .runtimes
            .read()
            .expect("subtexd runtimes")
            .iter()
            .map(|rt| rt.handle.hash().to_string())
            .collect();
        let mut new_paths = Vec::new();
        let mut runtimes = self.runtimes.write().expect("subtexd runtimes");
        for handle in discovered {
            if known.contains(handle.hash()) {
                continue;
            }
            let store = Arc::new(SubtexStore::open(&handle).context("open subtex store")?);
            let embedder =
                CloudEmbedder::with_store_ledger(store.clone()).map(|e| Arc::new(e) as Arc<dyn TextEmbedder>);
            let indexer = Indexer::new(store.clone(), embedder);
            tracing::info!("subtexd attached root {}", handle.root().display());
            new_paths.push(handle.root().to_path_buf());
            runtimes.push(RootRuntime {
                handle,
                store,
                indexer,
            });
        }
        Ok(new_paths)
    }

    /// notify event entry: mark the runtime containing `path` dirty.
    pub fn mark_path_dirty(&self, path: &Path) {
        let index = {
            let runtimes = self.runtimes.read().expect("subtexd runtimes");
            runtimes
                .iter()
                .position(|rt| path.starts_with(rt.handle.root()))
        };
        if let Some(index) = index {
            self.dirty.lock().expect("subtexd dirty").insert(index);
        }
    }

    /// Watcher-equivalent trigger: force the next tick to reconcile this root.
    pub fn mark_root_dirty(&self, root: &Path) {
        let index = {
            let runtimes = self.runtimes.read().expect("subtexd runtimes");
            runtimes
                .iter()
                .position(|rt| rt.handle.root() == root)
        };
        if let Some(index) = index {
            self.dirty.lock().expect("subtexd dirty").insert(index);
        }
    }

    /// One daemon step: refresh roots, reconcile dirty roots (or everything
    /// when the sweep is due), then consume jobs.
    pub async fn tick(&self) -> Result<TickReport> {
        let mut report = TickReport::default();
        report.new_roots = self.refresh_roots()?;

        let sweep_due = {
            let last = self.last_full_reconcile.lock().expect("subtexd sweep");
            last.map_or(true, |at: Instant| {
                at.elapsed().as_secs() >= self.cfg.reconcile_secs
            })
        };
        let dirty: Vec<usize> = self
            .dirty
            .lock()
            .expect("subtexd dirty")
            .drain()
            .collect();
        let runtime_count = self.runtimes.read().expect("subtexd runtimes").len();
        let targets: Vec<usize> = if sweep_due || !report.new_roots.is_empty() {
            (0..runtime_count).collect()
        } else {
            dirty
        };
        if sweep_due {
            *self.last_full_reconcile.lock().expect("subtexd sweep") = Some(Instant::now());
        }

        let targets: Vec<RootRuntime> = {
            let runtimes = self.runtimes.read().expect("subtexd runtimes");
            targets.iter().filter_map(|i| runtimes.get(*i).cloned()).collect()
        };
        for rt in targets {
            match rt.indexer.reconcile(rt.handle.root()).await {
                Ok(outcome) => {
                    if !outcome.indexed.is_empty()
                        || !outcome.removed.is_empty()
                        || !outcome.failed.is_empty()
                    {
                        report.reconciled_roots.push(rt.handle.root().display().to_string());
                    }
                    report.indexed_files += outcome.indexed.len();
                    report.removed_files += outcome.removed.len();
                    report.failed += outcome.failed.len();
                    report.failed_details.extend(outcome.failed);
                    for (path, outcome) in &outcome.indexed {
                        if matches!(outcome, subtex_index::FileIndexOutcome::AudioDeferred) {
                            match subtex_index::audio::enqueue_transcribe_job(
                                &rt.store,
                                rt.handle.root(),
                                path,
                            ) {
                                Ok(true) => tracing::info!("queued transcription for {path}"),
                                Ok(false) => {}
                                Err(e) => tracing::warn!("queue transcription {path}: {e:#}"),
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("reconcile {}: {e:#}", rt.handle.root().display());
                    report.failed += 1;
                    report
                        .failed_details
                        .push((rt.handle.root().display().to_string(), format!("{e:#}")));
                }
            }
        }

        self.auto_confirm_transcriptions(&mut report).await?;
        self.consume_jobs(&mut report).await?;
        Ok(report)
    }

    /// Small transcription batches run automatically (PRD F2); a batch above
    /// the threshold stays in `needs_confirmation` until the agent confirms
    /// via `subtex.transcribe confirm: true`.
    async fn auto_confirm_transcriptions(&self, report: &mut TickReport) -> Result<()> {
        let runtimes = self.runtimes.read().expect("subtexd runtimes").clone();
        for rt in runtimes {
            let waiting: Vec<JobRecord> = rt
                .store
                .jobs_of_kind("transcribe")
                .context("list transcribe jobs")?
                .into_iter()
                .filter(|job| job.state == "needs_confirmation")
                .collect();
            let total: f64 = waiting
                .iter()
                .map(|job| {
                    job.payload
                        .as_ref()
                        .and_then(|p| p.get("duration_secs"))
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0)
                })
                .sum();
            if total > subtex_index::CONFIRM_THRESHOLD_SECS {
                tracing::info!(
                    "transcription batch {}s above threshold; waiting for confirmation",
                    total as u64
                );
                continue;
            }
            for job in waiting {
                rt.store.set_job_state(job.id, "pending").ok();
            }
        }
        Ok(())
    }

    async fn consume_jobs(&self, report: &mut TickReport) -> Result<()> {
        let runtimes = self.runtimes.read().expect("subtexd runtimes").clone();
        for rt in runtimes {
            for _ in 0..self.cfg.max_jobs_per_tick {
                let Some(job) = rt.store.claim_next_job().context("claim job")? else {
                    break;
                };
                match self.execute_job(&rt, &job).await {
                    Ok(()) => {
                        rt.store.complete_job(job.id).context("complete job")?;
                        report.jobs_done += 1;
                    }
                    Err(e) => {
                        tracing::warn!("job {} ({}) failed: {e:#}", job.id, job.kind);
                        rt.store.fail_job(job.id, &format!("{e:#}")).ok();
                        report.failed += 1;
                        report.failed_details.push((job.kind.clone(), format!("{e:#}")));
                    }
                }
            }
        }
        Ok(())
    }

    async fn execute_job(&self, rt: &RootRuntime, job: &JobRecord) -> Result<()> {
        match job.kind.as_str() {
            "reindex_all" => {
                rt.indexer.reconcile(rt.handle.root()).await?;
                Ok(())
            }
            "reindex_file" => {
                let path = job.file_path.as_deref().context("reindex_file needs file_path")?;
                match subtex_core::scanner::scan_file(rt.handle.root(), path)? {
                    Some(file) => {
                        rt.indexer.index_file(rt.handle.root(), &file).await?;
                        Ok(())
                    }
                    None => {
                        rt.store.delete_file(path)?;
                        Ok(())
                    }
                }
            }
            "delete_file" => {
                let path = job.file_path.as_deref().context("delete_file needs file_path")?;
                rt.store.delete_file(path)?;
                Ok(())
            }
            "transcribe" => {
                let path = job.file_path.as_deref().context("transcribe needs file_path")?;
                let hash = job
                    .payload
                    .as_ref()
                    .and_then(|p| p.get("hash"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if hash.is_empty() {
                    anyhow::bail!("transcribe payload missing content hash");
                }
                let known_duration = job
                    .payload
                    .as_ref()
                    .and_then(|p| p.get("duration_secs"))
                    .and_then(serde_json::Value::as_f64);
                let outcome = subtex_index::audio::transcribe_file(
                    rt.handle.root(),
                    path,
                    &hash,
                    known_duration,
                )
                .await?;
                subtex_index::audio::record_transcription_usage(
                    &rt.store,
                    outcome.duration_secs,
                    &subtex_index::audio::run_name(path, &hash),
                )?;
                tracing::info!(
                    "transcribed {} -> {}",
                    path,
                    outcome.transcript_dest.display()
                );
                Ok(())
            }
            other => anyhow::bail!("unknown job kind: {other}"),
        }
    }
}

fn discover_roots(data_dir: &Path) -> Result<Vec<RootHandle>> {
    let roots_dir = data_dir.join(subtex_core::ROOTS_DIR_NAME);
    if !roots_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut handles = Vec::new();
    for entry in std::fs::read_dir(&roots_dir)?.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Ok(pointer) = std::fs::read_to_string(entry.path().join(subtex_core::ROOT_POINTER_FILE))
        else {
            continue;
        };
        let root = PathBuf::from(pointer.trim());
        if root.as_os_str().is_empty() {
            continue;
        }
        // Root vanished or pointer corrupt: the store stays on disk untouched,
        // the daemon just does not see a root here.
        let Ok(handle) = RootHandle::with_data_dir(&root, data_dir.to_path_buf()) else {
            continue;
        };
        if handle.hash() != entry.file_name().to_string_lossy() {
            continue;
        }
        handles.push(handle);
    }
    Ok(handles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use subtex_core::scanner::scan_file;
    use subtex_store_sqlite::ReadinessSummary;
    use tempfile::TempDir;

    struct Fixture {
        _data_dir: TempDir,
        _root_dir: TempDir,
        root: PathBuf,
        subtexd: Subtexd,
    }

    fn attach_root(data_dir: &Path, root: &Path) {
        let handle = RootHandle::with_data_dir(
            &std::fs::canonicalize(root).unwrap(),
            data_dir.to_path_buf(),
        )
        .unwrap();
        SubtexStore::open(&handle).unwrap();
    }

    fn fixture() -> Fixture {
        let data_dir = TempDir::new().unwrap();
        let root_dir = TempDir::new().unwrap();
        let root = std::fs::canonicalize(root_dir.path()).unwrap();
        attach_root(data_dir.path(), &root);
        let subtexd = Subtexd::new(SubtexdConfig {
            data_dir: data_dir.path().to_path_buf(),
            reconcile_secs: 5,
            max_jobs_per_tick: 8,
        })
        .unwrap();
        Fixture {
            _data_dir: data_dir,
            _root_dir: root_dir,
            root,
            subtexd,
        }
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn tick_indexes_changes_and_removals() {
        let fx = fixture();
        write(&fx.root, "notes.md", "# 笔记\n\n数据库连接池说明。\n");

        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(
            report.reconciled_roots,
            vec![fx.root.display().to_string()],
            "report: {report:?}"
        );
        assert_eq!(report.indexed_files, 1, "report: {report:?}");

        let store = fx.subtexd.store_for(&fx.root).unwrap();
        assert!(!store.search_lexical("数据库", 5).unwrap().is_empty());

        write(&fx.root, "notes.md", "# 笔记\n\n改成了 rsync 备份说明。\n");
        fx.subtexd.mark_root_dirty(&fx.root);
        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(report.indexed_files, 1, "report: {report:?}");
        assert!(!store.search_lexical("rsync", 5).unwrap().is_empty());

        std::fs::remove_file(fx.root.join("notes.md")).unwrap();
        fx.subtexd.mark_root_dirty(&fx.root);
        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(report.removed_files, 1, "report: {report:?}");
        assert_eq!(
            store.readiness_summary().unwrap(),
            ReadinessSummary::default()
        );
    }

    #[tokio::test]
    async fn refresh_picks_up_new_roots_and_jobs_execute() {
        let data_dir = TempDir::new().unwrap();
        let root_dir = TempDir::new().unwrap();
        let root = std::fs::canonicalize(root_dir.path()).unwrap();
        let subtexd = Subtexd::new(SubtexdConfig {
            data_dir: data_dir.path().to_path_buf(),
            reconcile_secs: 5,
            max_jobs_per_tick: 8,
        })
        .unwrap();
        assert!(subtexd.root_paths().is_empty());

        // A store appears while the daemon runs (subtex.init in avrag-api).
        attach_root(data_dir.path(), &root);
        write(&root, "a.md", "# 标题\n\njob 驱动的内容。\n");
        let report = subtexd.tick().await.unwrap();
        assert_eq!(report.new_roots, vec![root.clone()]);
        assert_eq!(report.indexed_files, 1);
        assert_eq!(subtexd.root_paths(), vec![root.clone()]);

        // Single-file reindex via the jobs table.
        write(&root, "a.md", "# 标题\n\n更新后的 job 内容。\n");
        let store = subtexd.store_for(&root).unwrap();
        store.enqueue_job("reindex_file", Some("a.md"), None).unwrap();
        let report = subtexd.tick().await.unwrap();
        assert_eq!(report.jobs_done, 1);
        assert!(!store.search_lexical("更新后的", 5).unwrap().is_empty());

        // Unknown kinds land in failed, not crash the loop.
        store.enqueue_job("bogus_kind", None, None).unwrap();
        let report = subtexd.tick().await.unwrap();
        assert_eq!(report.failed, 1);
    }

    #[tokio::test]
    async fn cached_scan_skips_unchanged_files() {
        let fx = fixture();
        write(&fx.root, "a.md", "first version\n");
        fx.subtexd.tick().await.unwrap();
        let store = fx.subtexd.store_for(&fx.root).unwrap();

        let before = store.file_scan_state().unwrap();
        assert!(before.contains_key("a.md"));
        // Unchanged content + unchanged mtime → cache path produces no diff.
        let files = subtex_core::scanner::scan_dir_cached(&fx.root, &before).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content_hash, before["a.md"].content_hash);

        // scan_file covers the single-file job path.
        let single = scan_file(&fx.root, "a.md").unwrap().unwrap();
        assert_eq!(single.rel_path, "a.md");
        assert!(scan_file(&fx.root, "missing.md").unwrap().is_none());
    }

    /// Fake transcribe.sh: prints the RUN_DIR/TRANSCRIPT contract for the
    /// input file without any network or billing.
    fn fake_script(dir: &Path) -> PathBuf {
        let script = dir.join("fake-transcribe.sh");
        std::fs::write(
            &script,
            "#!/usr/bin/env bash\n\
             printf 'fake transcript\\n' > \"$2.md\"\n\
             echo \"RUN_DIR=$(dirname \"$2\")\"\n\
             echo \"TRANSCRIPT=$2.md\"\n",
        )
        .unwrap();
        script
    }

    fn set_script_env(path: &Path) {
        // Edition 2024: process-global env mutation is unsafe.
        unsafe { std::env::set_var("ASR_TRANSCRIBE_SCRIPT", path) };
    }

    /// `ASR_TRANSCRIBE_SCRIPT` is process-global; transcribe tests must not overlap.
    static TRANSCRIBE_ENV: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn transcribe_job_executes_and_writes_back() {
        let _env = TRANSCRIBE_ENV.lock().await;
        let fx = fixture();
        let script_dir = TempDir::new().unwrap();
        set_script_env(&fake_script(script_dir.path()));

        write(&fx.root, "meetings/a.m4a", "fake audio bytes");
        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(report.jobs_done, 1, "report: {report:?}");

        let store = fx.subtexd.store_for(&fx.root).unwrap();
        let transcript = fx.root.join("transcripts/a.md");
        assert!(transcript.is_file());
        assert_eq!(std::fs::read_to_string(&transcript).unwrap(), "fake transcript\n");
        let totals = store.usage_totals().unwrap();
        assert!(totals.iter().any(|t| t.kind == "transcription"));

        // The write-back is a real project file → the next sweep indexes it.
        fx.subtexd.mark_root_dirty(&fx.root);
        let _ = fx.subtexd.tick().await.unwrap();
        assert!(store
            .search_lexical("fake transcript", 5)
            .unwrap()
            .iter()
            .any(|hit| hit.path.starts_with("transcripts/")));

        // Done job for the same hash must not re-run on the next sweep.
        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(report.jobs_done, 0, "report: {report:?}");
    }

    #[tokio::test]
    async fn big_transcription_batch_waits_for_confirmation() {
        let _env = TRANSCRIBE_ENV.lock().await;
        let fx = fixture();
        let script_dir = TempDir::new().unwrap();
        set_script_env(&fake_script(script_dir.path()));

        write(&fx.root, "big.m4a", "fake audio bytes");
        let store = fx.subtexd.store_for(&fx.root).unwrap();
        let id = store
            .enqueue_job_in_state(
                "transcribe",
                Some("big.m4a"),
                Some(&serde_json::json!({ "hash": "h1", "duration_secs": 8000.0 })),
                "needs_confirmation",
            )
            .unwrap();

        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(report.jobs_done, 0, "report: {report:?}");
        assert!(!fx.root.join("transcripts/big.md").exists());
        let job = store.latest_job("transcribe", "big.m4a").unwrap().unwrap();
        assert_eq!(job.state, "needs_confirmation");

        // Agent confirms via subtex.transcribe → pending → executed.
        store.set_job_state(id, "pending").unwrap();
        fx.subtexd.mark_root_dirty(&fx.root);
        let report = fx.subtexd.tick().await.unwrap();
        assert_eq!(report.jobs_done, 1, "report: {report:?}");
        assert!(fx.root.join("transcripts/big.md").exists());
    }
}
