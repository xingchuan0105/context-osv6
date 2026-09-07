use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use subtex_core::scanner::{scan_diff, scan_dir};
use subtex_core::RootHandle;
use subtex_index::{render_outline, FileIndexOutcome, Indexer, TextEmbedder};
use subtex_store_sqlite::SubtexStore;
use tempfile::TempDir;

/// Deterministic embedder: same text → same vector, so re-embedding a chunk's
/// exact content yields a distance-0 self-match in the vector channel.
struct HashEmbedder;

#[async_trait]
impl TextEmbedder for HashEmbedder {
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                let digest = Sha256::digest(text.as_bytes());
                let mut state = u64::from_le_bytes(digest[0..8].try_into().unwrap());
                let mut vector = Vec::with_capacity(1024);
                for _ in 0..1024 {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    vector.push(((state % 2000) as f32 - 1000.0) / 1000.0);
                }
                vector
            })
            .collect())
    }
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn setup() -> (TempDir, TempDir, PathBuf, Arc<SubtexStore>, Indexer) {
    // Root and data dir are separate trees: in production the index store
    // lives under the app-data dir, never inside a scanned project root.
    let root_dir = TempDir::new().unwrap();
    let data_dir = TempDir::new().unwrap();
    let root = std::fs::canonicalize(root_dir.path()).unwrap();
    let handle = RootHandle::with_data_dir(&root, data_dir.path().to_path_buf()).unwrap();
    let store = Arc::new(SubtexStore::open(&handle).unwrap());
    let indexer = Indexer::new(store.clone(), Some(Arc::new(HashEmbedder)));
    (root_dir, data_dir, root, store, indexer)
}

const NOTES_MD: &str = "# 项目笔记\n\n数据库连接池使用 pgvector 存储 embedding。\n\n## 部署\n\n部署在 WSL，jobs=2。\n";
const TRANSCRIPT_MD: &str = "# 会议转写 2026-09-01\n\n说话人 A 00:01:02 讨论了索引切片方案。\n说话人 B 00:02:10 确认了转写回写位置。\n";

#[tokio::test]
async fn index_diff_pipeline_end_to_end() {
    let (_root_dir, _data_dir, root, store, indexer) = setup();
    assert!(store.vector_available(), "bundled sqlite-vec must load on this platform");

    write_file(&root, "notes.md", NOTES_MD);
    write_file(&root, "transcript.md", TRANSCRIPT_MD);
    write_file(&root, "src/lib.rs", "fn add(a: i32, b: i32) -> i32 { a + b }\n");
    write_file(&root, "data.csv", "id,name\n1,alpha\n2,beta\n");
    write_file(&root, "readme.txt", "plain text notes about deployment\n");

    let files = scan_dir(&root).unwrap();
    assert_eq!(files.len(), 5);
    let added = scan_diff(&HashMap::new(), &files);
    let outcome = indexer.index_diff(&root, &added).await.unwrap();
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    assert_eq!(outcome.indexed.len(), 5);
    assert!(outcome
        .indexed
        .iter()
        .all(|(_, o)| matches!(o, FileIndexOutcome::Indexed { .. })));

    let readiness = store.readiness_summary().unwrap();
    assert_eq!(
        readiness,
        subtex_store_sqlite::ReadinessSummary {
            files: 5,
            lexical_ready: 5,
            outline_ready: 5,
            vector_ready: 5,
        }
    );

    let lexical = store.search_lexical("数据库", 5).unwrap();
    assert!(!lexical.is_empty());
    assert_eq!(lexical[0].path, "notes.md");
    assert!(lexical[0].line_start.is_some());

    let query_vector = HashEmbedder.embed(&[lexical[0].content.clone()]).await.unwrap().remove(0);
    let vector = store.search_vector(&query_vector, 5).unwrap();
    assert_eq!(vector[0].chunk_id, lexical[0].chunk_id, "exact text self-match ranks first");
    assert!((vector[0].score - 1.0).abs() < 1e-6);

    let hybrid = store.search_hybrid("数据库", Some(&query_vector), 5).unwrap();
    assert!(hybrid.lexical_used && hybrid.vector_used);
    assert_eq!(hybrid.hits[0].hit.chunk_id, lexical[0].chunk_id);
    assert_eq!(hybrid.hits[0].lexical_rank, Some(1));
    assert_eq!(hybrid.hits[0].vector_rank, Some(1));
    assert_eq!(hybrid.readiness.files, 5);

    let outline = render_outline(&store, 2000).unwrap();
    assert!(!outline.truncated);
    assert!(outline.text.contains("notes.md"));
    assert!(outline.text.contains("项目笔记"));
    assert!(outline.text.contains("部署"));
    assert!(outline.token_count <= 2000);
    let tight = render_outline(&store, 1).unwrap();
    assert!(tight.truncated && tight.files == 0);

    // Modify: new content becomes searchable, readiness stays complete.
    write_file(&root, "notes.md", &format!("{NOTES_MD}\n备份策略：每晚 rsync 到 NAS。\n"));
    let files = scan_dir(&root).unwrap();
    let changed = scan_diff(&store.file_hashes().unwrap(), &files);
    assert_eq!(changed.modified.len(), 1);
    assert_eq!(changed.modified[0].rel_path, "notes.md");
    let outcome = indexer.index_diff(&root, &changed).await.unwrap();
    assert_eq!(outcome.indexed.len(), 1);
    assert!(!store.search_lexical("rsync", 5).unwrap().is_empty());

    // Remove: derived rows disappear with the file.
    std::fs::remove_file(root.join("readme.txt")).unwrap();
    let files = scan_dir(&root).unwrap();
    let removed = scan_diff(&store.file_hashes().unwrap(), &files);
    assert_eq!(removed.removed, vec!["readme.txt"]);
    let outcome = indexer.index_diff(&root, &removed).await.unwrap();
    assert_eq!(outcome.removed, vec!["readme.txt"]);
    assert_eq!(store.readiness_summary().unwrap().files, 4);
    assert!(store.search_lexical("deployment", 5).unwrap().is_empty());
}

#[tokio::test]
async fn audio_is_deferred_and_garbage_heavy_format_is_unsupported() {
    let (_root_dir, _data_dir, root, store, indexer) = setup();
    std::fs::write(root.join("meeting.m4a"), b"\x00\x01audio").unwrap();
    write_file(&root, "broken.docx", "this is not a real docx");

    let files = scan_dir(&root).unwrap();
    let diff = scan_diff(&HashMap::new(), &files);
    let outcome = indexer.index_diff(&root, &diff).await.unwrap();

    let by_path: HashMap<&str, &FileIndexOutcome> = outcome
        .indexed
        .iter()
        .map(|(path, o)| (path.as_str(), o))
        .collect();
    assert_eq!(by_path["meeting.m4a"], &FileIndexOutcome::AudioDeferred);
    assert!(
        matches!(by_path["broken.docx"], FileIndexOutcome::Unsupported { .. }),
        "garbage docx must degrade to Unsupported, got {:?}",
        by_path["broken.docx"]
    );
    // Deferred / unsupported files never enter the store.
    assert_eq!(store.readiness_summary().unwrap().files, 0);
}

#[tokio::test]
async fn pdf_indexed_when_liteparse_available() {
    if !Command::new("lit").arg("--version").output().is_ok_and(|o| o.status.success()) {
        eprintln!("lit binary not available; skipping pdf heavy-parse test");
        return;
    }
    let (_root_dir, _data_dir, root, store, indexer) = setup();
    let pdf = minimal_pdf("Subtex PDF pipeline works");
    write_file(&root, "doc.pdf", &String::from_utf8_lossy(&pdf));

    let files = scan_dir(&root).unwrap();
    let diff = scan_diff(&HashMap::new(), &files);
    let outcome = indexer.index_diff(&root, &diff).await.unwrap();
    assert!(
        matches!(&outcome.indexed[0].1, FileIndexOutcome::Indexed { .. }),
        "pdf outcome {:?}",
        outcome.indexed[0].1
    );
    assert!(!store.search_lexical("pipeline", 5).unwrap().is_empty());
}

fn minimal_pdf(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 24 Tf 72 700 Td ({text}) Tj ET");
    let objects = [
        "<</Type/Catalog/Pages 2 0 R>>".to_string(),
        "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_string(),
        "<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>".to_string(),
        format!("<</Length {}>>stream\n{stream}\nendstream", stream.len()),
        "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (idx, obj) in objects.iter().enumerate() {
        offsets.push(out.len() as u32);
        out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", idx + 1).as_bytes());
    }
    let xref_pos = out.len() as u32;
    let size = objects.len() + 1;
    let mut xref = format!("xref\n0 {size}\n0000000000 65535 f \n");
    for offset in &offsets {
        xref.push_str(&format!("{offset:010} 00000 n \n"));
    }
    xref.push_str(&format!("trailer<</Size {size}/Root 1 0 R>>\nstartxref\n{xref_pos}\n%%EOF"));
    out.extend_from_slice(xref.as_bytes());
    out
}
