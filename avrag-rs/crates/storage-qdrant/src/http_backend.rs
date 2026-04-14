#[derive(Debug, Clone)]
pub struct HttpQdrantBackend {
    base_url: String,
    client: Client,
}

impl HttpQdrantBackend {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    fn collection_url(&self, collection: &str) -> String {
        format!("{}/collections/{}", self.base_url, collection)
    }

    fn points_url(&self, collection: &str) -> String {
        format!("{}/collections/{}/points", self.base_url, collection)
    }

    fn search_url(&self, collection: &str) -> String {
        format!("{}/collections/{}/points/search", self.base_url, collection)
    }

    async fn ensure_success(
        response: reqwest::Response,
        action: &'static str,
    ) -> Result<reqwest::Response, QdrantStorageError> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(QdrantStorageError::Backend {
            message: format!("{action} failed with {status}: {body}"),
        })
    }

    pub async fn upsert_multimodal_points(
        &self,
        collection: &str,
        points: &[MultimodalQdrantPointUpsert],
    ) -> Result<(), QdrantStorageError> {
        if points.is_empty() {
            return Ok(());
        }

        let payload_points = points
            .iter()
            .map(|point| {
                let mut payload = json!({
                    "org_id": point.org_id.to_string(),
                    "chunk_id": point.chunk_id.to_string(),
                    "doc_id": point.doc_id.to_string(),
                    "asset_id": point.asset_id.to_string(),
                    "page": point.page,
                    "parser_backend": point.parser_backend,
                    "doc_version": point.doc_version,
                });

                if let Some(caption) = &point.caption {
                    payload["caption"] = json!(caption);
                }

                json!({
                    "id": point.chunk_id.to_string(),
                    "vector": point.vector,
                    "payload": payload,
                })
            })
            .collect::<Vec<_>>();

        let response = self
            .client
            .put(self.points_url(collection))
            .query(&[("wait", "true")])
            .json(&json!({ "points": payload_points }))
            .send()
            .await?;
        Self::ensure_success(response, "upsert multimodal points").await?;
        Ok(())
    }

    /// Dense vector search with optional doc_id filter
    pub async fn search_dense(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        org_id: OrgId,
        doc_ids: Option<&[Uuid]>,
        limit: usize,
    ) -> Result<Vec<SearchHit>, QdrantStorageError> {
        if query_vector.is_empty() {
            return Err(QdrantStorageError::EmptyVector);
        }

        let mut filter = QdrantFilter {
            must: vec![FieldMatch {
                key: "org_id".to_owned(),
                value: org_id.to_string(),
            }],
        };

        if let Some(ids) = doc_ids {
            for doc_id in ids {
                filter.must.push(FieldMatch {
                    key: "doc_id".to_owned(),
                    value: doc_id.to_string(),
                });
            }
        }

        let response = self
            .client
            .post(self.search_url(collection))
            .json(&json!({
                "vector": query_vector,
                "limit": limit,
                "with_payload": true,
                "filter": filter_to_json(&filter),
            }))
            .send()
            .await?;
        let response = Self::ensure_success(response, "search dense points").await?;
        let body: QdrantSearchResponse = response.json().await?;

        body.result
            .into_iter()
            .map(|point| {
                let payload = point.payload;
                let chunk_id = payload
                    .get("chunk_id")
                    .and_then(Value::as_str)
                    .or_else(|| point.id.as_deref())
                    .ok_or_else(|| QdrantStorageError::Backend {
                        message: "search result missing chunk_id".to_string(),
                    })?;
                let doc_id = payload
                    .get("doc_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| QdrantStorageError::Backend {
                        message: "search result missing doc_id payload".to_string(),
                    })?;

                Ok(SearchHit {
                    chunk_id: Uuid::parse_str(chunk_id).map_err(|error| {
                        QdrantStorageError::Backend {
                            message: format!("invalid chunk_id payload: {error}"),
                        }
                    })?,
                    doc_id: Uuid::parse_str(doc_id).map_err(|error| {
                        QdrantStorageError::Backend {
                            message: format!("invalid doc_id payload: {error}"),
                        }
                    })?,
                    page: payload
                        .get("page")
                        .and_then(Value::as_i64)
                        .map(|p| p as u32),
                    score: point.score,
                })
            })
            .collect()
    }
}

#[async_trait]
impl VectorSearchBackend for HttpQdrantBackend {
    async fn ensure_collection(
        &self,
        config: &QdrantCollectionConfig,
    ) -> Result<(), QdrantStorageError> {
        let url = self.collection_url(&config.name);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let body: Value = response.json().await?;
            if let Some(actual_size) = collection_vector_size(&body)
                && actual_size != config.vector_size
            {
                return Err(QdrantStorageError::Backend {
                    message: format!(
                        "collection {} already exists with vector size {}, expected {}; recreate the collection or use a new QDRANT_COLLECTION",
                        config.name, actual_size, config.vector_size
                    ),
                });
            }
            return Ok(());
        }

        if response.status() != StatusCode::NOT_FOUND {
            return Err(QdrantStorageError::Backend {
                message: format!(
                    "get collection failed with {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                ),
            });
        }

        let create_response = self
            .client
            .put(&url)
            .json(&json!({
                "vectors": {
                    "size": config.vector_size,
                    "distance": config.distance.as_str(),
                },
            }))
            .send()
            .await?;
        Self::ensure_success(create_response, "ensure collection").await?;
        Ok(())
    }

    async fn upsert_points(
        &self,
        collection: &str,
        points: &[QdrantPointUpsert],
    ) -> Result<(), QdrantStorageError> {
        if points.is_empty() {
            return Ok(());
        }

        let payload_points = points
            .iter()
            .map(|point| {
                json!({
                    "id": point.chunk_id.to_string(),
                    "vector": point.vector,
                    "payload": {
                        "org_id": point.org_id.to_string(),
                        "chunk_id": point.chunk_id.to_string(),
                        "doc_id": point.doc_id.to_string(),
                        "page": point.page,
                        "doc_version": point.doc_version,
                    }
                })
            })
            .collect::<Vec<_>>();

        let response = self
            .client
            .put(self.points_url(collection))
            .query(&[("wait", "true")])
            .json(&json!({ "points": payload_points }))
            .send()
            .await?;
        Self::ensure_success(response, "upsert points").await?;
        Ok(())
    }

    async fn delete_points_by_filter(
        &self,
        collection: &str,
        filter: &QdrantFilter,
    ) -> Result<(), QdrantStorageError> {
        let response = self
            .client
            .post(format!("{}/delete", self.points_url(collection)))
            .query(&[("wait", "true")])
            .json(&json!({
                "filter": filter_to_json(filter),
            }))
            .send()
            .await?;
        Self::ensure_success(response, "delete points").await?;
        Ok(())
    }

    async fn search(
        &self,
        request: QdrantSearchRequest,
    ) -> Result<Vec<QdrantPoint>, QdrantStorageError> {
        if request.vector.is_empty() {
            return Err(QdrantStorageError::EmptyVector);
        }

        let response = self
            .client
            .post(self.search_url(&request.collection))
            .json(&json!({
                "vector": request.vector,
                "limit": request.limit,
                "with_payload": true,
                "filter": filter_to_json(&request.filter),
            }))
            .send()
            .await?;
        let response = Self::ensure_success(response, "search points").await?;
        let body: QdrantSearchResponse = response.json().await?;

        body.result
            .into_iter()
            .map(|point| {
                let payload = point.payload;
                let org_id = payload
                    .get("org_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| QdrantStorageError::Backend {
                        message: "search result missing org_id payload".to_string(),
                    })?;
                let chunk_id = payload
                    .get("chunk_id")
                    .and_then(Value::as_str)
                    .or_else(|| point.id.as_deref())
                    .ok_or_else(|| QdrantStorageError::Backend {
                        message: "search result missing chunk_id".to_string(),
                    })?;
                let doc_id = payload
                    .get("doc_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| QdrantStorageError::Backend {
                        message: "search result missing doc_id payload".to_string(),
                    })?;

                Ok(QdrantPoint {
                    chunk_id: Uuid::parse_str(chunk_id).map_err(|error| {
                        QdrantStorageError::Backend {
                            message: format!("invalid chunk_id payload: {error}"),
                        }
                    })?,
                    doc_id: Uuid::parse_str(doc_id).map_err(|error| {
                        QdrantStorageError::Backend {
                            message: format!("invalid doc_id payload: {error}"),
                        }
                    })?,
                    page: payload.get("page").and_then(Value::as_i64),
                    score: point.score,
                    org_id: org_id.parse::<OrgId>().map_err(|error| {
                        QdrantStorageError::Backend {
                            message: format!("invalid org_id payload: {error}"),
                        }
                    })?,
                })
            })
            .collect()
    }

    async fn delete_points_by_doc_id(
        &self,
        collection: &str,
        doc_id: Uuid,
    ) -> Result<(), QdrantStorageError> {
        let filter = QdrantFilter {
            must: vec![FieldMatch {
                key: "doc_id".to_owned(),
                value: doc_id.to_string(),
            }],
        };

        let response = self
            .client
            .post(format!("{}/delete", self.points_url(collection)))
            .query(&[("wait", "true")])
            .json(&json!({
                "filter": filter_to_json(&filter),
            }))
            .send()
            .await?;
        Self::ensure_success(response, "delete points by doc_id").await?;
        Ok(())
    }

    async fn upsert_points_with_version(
        &self,
        collection: &str,
        points: Vec<QdrantPointUpsert>,
        doc_version: u32,
    ) -> Result<(), QdrantStorageError> {
        if points.is_empty() {
            return Ok(());
        }

        let payload_points = points
            .iter()
            .map(|point| {
                json!({
                    "id": point.chunk_id.to_string(),
                    "vector": point.vector,
                    "payload": {
                        "org_id": point.org_id.to_string(),
                        "chunk_id": point.chunk_id.to_string(),
                        "doc_id": point.doc_id.to_string(),
                        "page": point.page,
                        "doc_version": doc_version,
                    }
                })
            })
            .collect::<Vec<_>>();

        let response = self
            .client
            .put(self.points_url(collection))
            .query(&[("wait", "true")])
            .json(&json!({ "points": payload_points }))
            .send()
            .await?;
        Self::ensure_success(response, "upsert points with version").await?;
        Ok(())
    }
}

fn collection_vector_size(body: &Value) -> Option<u64> {
    let vectors = body
        .get("result")?
        .get("config")?
        .get("params")?
        .get("vectors")?;

    if let Some(size) = vectors.get("size").and_then(Value::as_u64) {
        return Some(size);
    }

    vectors
        .as_object()?
        .values()
        .find_map(|entry| entry.get("size").and_then(Value::as_u64))
}

fn filter_to_json(filter: &QdrantFilter) -> Value {
    let must = filter
        .must
        .iter()
        .map(|field| {
            json!({
                "key": field.key,
                "match": { "value": field.value }
            })
        })
        .collect::<Vec<_>>();

    json!({ "must": must })
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    result: Vec<QdrantSearchResultPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResultPoint {
    #[serde(default)]
    id: Option<String>,
    score: f32,
    #[serde(default)]
    payload: Value,
}
