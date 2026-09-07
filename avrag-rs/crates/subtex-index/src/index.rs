use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use ingestion::chunker::{build_ir_chunk_plan, ChunkPolicy};
use ingestion::ir::MD_LINE_END_KEY;
use ingestion::ir::MD_LINE_START_KEY;
use subtex_core::scanner::{DirDiff, ScannedFile};
use subtex_store_sqlite::{ChunkRecord, SubtexStore};

use crate::embed::TextEmbedder;
use crate::parse;

#[derive(Debug, Clone, PartialEq)]
pub enum FileIndexOutcome {
    Indexed {
        chunks: usize,
        embedded: usize,
        vector_available: bool,
    },
    Unsupported {
        reason: String,
    },
    /// Audio files are handled by the transcription path (W4), then enter the
    /// index as ordinary transcripts.
    AudioDeferred,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DiffOutcome {
    pub indexed: Vec<(String, FileIndexOutcome)>,
    pub removed: Vec<String>,
    pub failed: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct Indexer {
    store: Arc<SubtexStore>,
    /// `None` when no embedding credentials are configured: the vector layer
    /// then stays unready and lexical + outline keep answering.
    embedder: Option<Arc<dyn TextEmbedder>>,
}

impl Indexer {
    pub fn new(store: Arc<SubtexStore>, embedder: Option<Arc<dyn TextEmbedder>>) -> Self {
        Self { store, embedder }
    }

    pub async fn index_diff(&self, root: &Path, diff: &DirDiff) -> anyhow::Result<DiffOutcome> {
        let mut outcome = DiffOutcome::default();
        for file in diff.added.iter().chain(diff.modified.iter()) {
            match self.index_file(root, file).await {
                Ok(o) => outcome.indexed.push((file.rel_path.clone(), o)),
                Err(e) => outcome.failed.push((file.rel_path.clone(), format!("{e:#}"))),
            }
        }
        for rel in &diff.removed {
            match self.remove_file(rel) {
                Ok(_) => outcome.removed.push(rel.clone()),
                Err(e) => outcome.failed.push((rel.clone(), format!("{e:#}"))),
            }
        }
        Ok(outcome)
    }

    pub async fn index_file(&self, root: &Path, file: &ScannedFile) -> anyhow::Result<FileIndexOutcome> {
        let abs = root.join(&file.rel_path);
        let bytes = tokio::fs::read(&abs)
            .await
            .with_context(|| format!("read {}", abs.display()))?;

        if parse::is_audio_file(&file.rel_path) {
            // Persist the hash so the next sweep does not treat the file as new.
            self.remember_unindexed(file)?;
            return Ok(FileIndexOutcome::AudioDeferred);
        }

        let document_id = uuid::Uuid::new_v4();
        let ir = match parse::parse_light(&document_id.to_string(), &file.rel_path, &bytes) {
            Some(ir) => ir,
            None => match parse::parse_heavy(document_id, &file.rel_path, &bytes).await {
                Ok(ir) => ir,
                Err(e) => {
                    self.remember_unindexed(file)?;
                    return Ok(FileIndexOutcome::Unsupported { reason: format!("{e:#}") });
                }
            },
        };

        let plan = build_ir_chunk_plan(&ir, &file.rel_path, &ChunkPolicy::default());
        let records: Vec<ChunkRecord> = plan
            .text_chunks
            .iter()
            .map(|c| ChunkRecord {
                chunk_id: uuid::Uuid::new_v4().to_string(),
                seq: c.cursor as i64,
                content: c.text.clone(),
                chunk_type: c.block_type.as_str().to_string(),
                heading_path: (!c.section_path.is_empty()).then(|| c.section_path.join(" > ")),
                page: c.page.map(i64::from),
                line_start: c.metadata.get(MD_LINE_START_KEY).and_then(|v| v.parse().ok()),
                line_end: c.metadata.get(MD_LINE_END_KEY).and_then(|v| v.parse().ok()),
            })
            .collect();

        let file_id = self
            .store
            .upsert_file(&file.rel_path, &file.content_hash, file.size, file.mtime_ms)?;
        self.store.replace_file_chunks(&file_id, &records)?;
        self.store.replace_file_outline(&file_id, &extract_outline(&ir))?;
        self.store.mark_file_indexed(&file_id)?;

        let lexical = !records.is_empty();
        let mut embedded = 0usize;
        if let Some(embedder) = &self.embedder {
            if self.store.vector_available() && !records.is_empty() {
                let texts: Vec<String> = records.iter().map(|r| r.content.clone()).collect();
                match embedder.embed(&texts).await {
                    Ok(vectors) if vectors.len() == records.len() => {
                        let pairs: Vec<(String, Vec<f32>)> = records
                            .iter()
                            .map(|r| r.chunk_id.clone())
                            .zip(vectors)
                            .collect();
                        embedded = self.store.set_chunk_vectors(&pairs)?;
                    }
                    Ok(other) => {
                        anyhow::bail!(
                            "embedder returned {} vectors for {} chunks of {}",
                            other.len(),
                            records.len(),
                            file.rel_path
                        );
                    }
                    Err(e) => {
                        // Degrade: lexical + outline stay ready, vector layer
                        // retries on the next indexing pass.
                        tracing::warn!(
                            "embedding failed for {} (vector stays unready): {e:#}",
                            file.rel_path
                        );
                    }
                }
            }
        }

        let vector = lexical && embedded == records.len();
        self.store.set_file_readiness(&file_id, lexical, lexical, vector)?;
        Ok(FileIndexOutcome::Indexed {
            chunks: records.len(),
            embedded,
            vector_available: self.store.vector_available(),
        })
    }

    pub fn remove_file(&self, rel_path: &str) -> anyhow::Result<bool> {
        Ok(self.store.delete_file(rel_path)?)
    }

    /// Record a seen-but-not-text-indexed file (audio waiting on transcription,
    /// or a format we cannot parse) so `scan_diff` does not report it as `added`
    /// on every sweep.
    fn remember_unindexed(&self, file: &ScannedFile) -> anyhow::Result<()> {
        let file_id = self.store.upsert_file(
            &file.rel_path,
            &file.content_hash,
            file.size,
            file.mtime_ms,
        )?;
        self.store
            .set_file_readiness(&file_id, false, false, false)?;
        Ok(())
    }

    /// Reconcile one root: cached scan (size+mtime unchanged → hash reused)
    /// → diff against the store's file state → index the delta. This is the
    /// periodic sweep subtexd runs, and the catch-up after a daemon restart.
    pub async fn reconcile(&self, root: &Path) -> anyhow::Result<DiffOutcome> {
        let previous = self
            .store
            .file_scan_state()
            .map_err(|e| anyhow::anyhow!("read scan state: {e}"))?;
        let files = subtex_core::scanner::scan_dir_cached(root, &previous)?;
        let previous_hashes: std::collections::HashMap<String, String> = previous
            .into_iter()
            .map(|(path, entry)| (path, entry.content_hash))
            .collect();
        let diff = subtex_core::scanner::scan_diff(&previous_hashes, &files);
        self.index_diff(root, &diff).await
    }
}

/// Heading outline from the parsed IR, mirroring the chunker's rolling
/// heading-chain rule (push heading text, cap at 6, explicit section_path
/// replaces the chain). Extracted per Heading block — chunk merging can fold
/// short headings away, so the outline must not depend on chunks.
fn extract_outline(ir: &ingestion::ir::DocumentIr) -> Vec<(i64, String)> {
    use ingestion::ir::BlockType;

    let mut chain: Vec<String> = Vec::new();
    let mut entries: Vec<(i64, String)> = Vec::new();
    for block in &ir.blocks {
        if block.block_type != BlockType::Heading {
            continue;
        }
        if !block.section_path.is_empty() {
            chain = block.section_path.clone();
        } else {
            let text = block.text.trim();
            if !text.is_empty() {
                chain.push(text.to_string());
                if chain.len() > 6 {
                    chain.remove(0);
                }
            }
        }
        entries.push((entries.len() as i64, chain.join(" > ")));
    }
    entries
}
