#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMatch {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QdrantFilter {
    pub must: Vec<FieldMatch>,
}

/// A search hit with doc_id, chunk_id, score
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub score: f32,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantSearchRequest {
    pub collection: String,
    pub vector: Vec<f32>,
    pub limit: u64,
    pub filter: QdrantFilter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantPoint {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub page: Option<i64>,
    pub score: f32,
    pub org_id: OrgId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantPointUpsert {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub org_id: OrgId,
    pub page: Option<i64>,
    pub vector: Vec<f32>,
    pub doc_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalQdrantPointUpsert {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub asset_id: Uuid,
    pub org_id: OrgId,
    pub page: Option<i64>,
    pub vector: Vec<f32>,
    pub caption: Option<String>,
    pub parser_backend: String,
    pub doc_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QdrantDistance {
    Cosine,
    Dot,
    Euclid,
}

impl QdrantDistance {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Cosine => "Cosine",
            Self::Dot => "Dot",
            Self::Euclid => "Euclid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QdrantCollectionConfig {
    pub name: String,
    pub vector_size: u64,
    pub distance: QdrantDistance,
}

pub struct SecureQdrantFilterBuilder;

impl SecureQdrantFilterBuilder {
    pub fn for_context(context: &AuthContext) -> Result<QdrantFilter, QdrantStorageError> {
        Ok(QdrantFilter {
            must: vec![FieldMatch {
                key: "org_id".to_owned(),
                value: context.org_id().to_string(),
            }],
        })
    }

    pub fn with_doc_filter(
        context: &AuthContext,
        doc_id: Uuid,
    ) -> Result<QdrantFilter, QdrantStorageError> {
        let mut filter = Self::for_context(context)?;
        filter.must.push(FieldMatch {
            key: "doc_id".to_owned(),
            value: doc_id.to_string(),
        });
        Ok(filter)
    }
}

#[async_trait]
pub trait VectorSearchBackend: Send + Sync {
    async fn ensure_collection(
        &self,
        config: &QdrantCollectionConfig,
    ) -> Result<(), QdrantStorageError>;

    async fn upsert_points(
        &self,
        collection: &str,
        points: &[QdrantPointUpsert],
    ) -> Result<(), QdrantStorageError>;

    async fn delete_points_by_filter(
        &self,
        collection: &str,
        filter: &QdrantFilter,
    ) -> Result<(), QdrantStorageError>;

    async fn search(
        &self,
        request: QdrantSearchRequest,
    ) -> Result<Vec<QdrantPoint>, QdrantStorageError>;

    async fn delete_points_by_doc_id(
        &self,
        collection: &str,
        doc_id: Uuid,
    ) -> Result<(), QdrantStorageError>;

    async fn upsert_points_with_version(
        &self,
        collection: &str,
        points: Vec<QdrantPointUpsert>,
        doc_version: u32,
    ) -> Result<(), QdrantStorageError>;
}

pub async fn search_candidates<B: VectorSearchBackend>(
    backend: &B,
    context: &AuthContext,
    collection: impl Into<String>,
    vector: Vec<f32>,
    limit: u64,
) -> Result<Vec<QdrantPoint>, QdrantStorageError> {
    if vector.is_empty() {
        return Err(QdrantStorageError::EmptyVector);
    }

    let request = QdrantSearchRequest {
        collection: collection.into(),
        vector,
        limit,
        filter: SecureQdrantFilterBuilder::for_context(context)?,
    };

    backend.search(request).await
}

#[derive(Debug, Error)]
pub enum QdrantStorageError {
    #[error("empty embedding vector")]
    EmptyVector,
    #[error("authorization failure: {0}")]
    Auth(#[from] AuthError),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("backend error: {message}")]
    Backend { message: String },
}
