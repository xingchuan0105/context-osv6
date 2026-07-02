use async_trait::async_trait;
use avrag_auth::AuthContext;
use common::{
    ContentStore, ContentStoreError, Document, DocumentMetadata, IndexedChunk, SummaryMetadata,
    TocEntry,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// 本地文件系统内容存储
///
/// 替代 MinIO/对象存储，将文档和 chunk 存储在本地文件系统中
#[derive(Clone)]
pub struct LocalContentStore {
    base_dir: PathBuf,
}

impl LocalContentStore {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    fn chunks_dir(&self) -> PathBuf {
        self.base_dir.join("chunks")
    }

    fn docs_dir(&self) -> PathBuf {
        self.base_dir.join("docs")
    }

    async fn ensure_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.chunks_dir()).await?;
        fs::create_dir_all(self.docs_dir()).await?;
        Ok(())
    }

    fn chunk_path(&self, chunk_id: &Uuid) -> PathBuf {
        self.chunks_dir().join(format!("{}.json", chunk_id))
    }

    fn doc_path(&self, doc_id: &Uuid) -> PathBuf {
        self.docs_dir().join(format!("{}.json", doc_id))
    }
}

#[async_trait]
impl ContentStore for LocalContentStore {
    async fn get_chunks_by_ids(
        &self,
        _auth: &AuthContext,
        chunk_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, IndexedChunk>, ContentStoreError> {
        self.ensure_dirs()
            .await
            .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

        let mut result = HashMap::new();

        for chunk_id in chunk_ids {
            let path = self.chunk_path(chunk_id);
            if path.exists() {
                let content = fs::read_to_string(&path)
                    .await
                    .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

                if let Ok(chunk) = serde_json::from_str::<IndexedChunk>(&content) {
                    result.insert(*chunk_id, chunk);
                }
            }
        }

        Ok(result)
    }

    async fn get_document_metadata_by_ids(
        &self,
        _auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<DocumentMetadata>, ContentStoreError> {
        self.ensure_dirs()
            .await
            .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

        let mut result = Vec::new();

        for doc_id in doc_ids {
            let path = self.doc_path(doc_id);
            if path.exists() {
                let content = fs::read_to_string(&path)
                    .await
                    .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

                if let Ok(doc) = serde_json::from_str::<DocumentMetadata>(&content) {
                    result.push(doc);
                }
            }
        }

        Ok(result)
    }

    async fn get_summary_metadata(
        &self,
        _auth: &AuthContext,
        _doc_ids: &[Uuid],
    ) -> Result<Vec<SummaryMetadata>, ContentStoreError> {
        // 本地存储暂不支持摘要元数据
        Ok(Vec::new())
    }

    async fn get_document_toc_entries(
        &self,
        _auth: &AuthContext,
        _doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, TocEntry)>, ContentStoreError> {
        // 本地存储暂不支持目录
        Ok(Vec::new())
    }

    async fn get_summary_chunks(
        &self,
        _auth: &AuthContext,
        _doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, String)>, ContentStoreError> {
        // 本地存储暂不支持摘要 chunk
        Ok(Vec::new())
    }

    async fn list_documents(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
        _document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, ContentStoreError> {
        self.ensure_dirs()
            .await
            .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

        let mut result = Vec::new();
        let docs_dir = self.docs_dir();

        if docs_dir.exists() {
            let mut entries = fs::read_dir(&docs_dir)
                .await
                .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| ContentStoreError::Internal(e.to_string()))?
            {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    let content = fs::read_to_string(&path)
                        .await
                        .map_err(|e| ContentStoreError::Internal(e.to_string()))?;

                    if let Ok(doc) = serde_json::from_str::<Document>(&content) {
                        result.push(doc);
                    }
                }
            }
        }

        Ok(result)
    }

    async fn get_document_names(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, String>, ContentStoreError> {
        let docs = self.get_document_metadata_by_ids(auth, doc_ids).await?;
        let mut result = HashMap::new();

        for doc in docs {
            if let Ok(uuid) = Uuid::parse_str(&doc.doc_id) {
                result.insert(uuid, doc.name.clone());
            }
        }

        Ok(result)
    }
}
