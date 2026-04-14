mod analytics_jobs;

use anyhow::Result;
use app::{AppConfig, AppState};
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_cache_redis::DocumentLock;
use avrag_llm::{MultiModalEmbeddingInput, SummaryGenerator};
use avrag_storage_pg::{
    NotificationCreateParams, ObjectStoreHandle, PgAppRepository, S3ObjectStore,
};
use avrag_storage_qdrant::{
    HttpQdrantBackend, QdrantCollectionConfig, QdrantDistance, QdrantPointUpsert,
    SecureQdrantFilterBuilder, VectorSearchBackend,
};
use ingestion::chunker::ChunkPolicy;
use ingestion::parser::{
    MineruClient, MineruConfig, NormalizedDocument, ParseRoute, ParseRouter, ParsedUnit,
    ParserFactory,
};
use ingestion::{
    AuditRecord, AuditSink, IngestionError, IngestionTask, NoopAuditSink, NoopStateSink,
    NoopTaskProcessor, NoopTaskSource, StateSink, TaskProcessor, TaskSource, Transition,
    WorkerRuntime, WorkerTick,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::time::{Duration, interval};
use tracing::info;
use uuid::Uuid;

struct PgTaskSource {
    repo: PgAppRepository,
    worker_id: String,
}

#[async_trait::async_trait]
impl TaskSource for PgTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        self.repo
            .claim_next_ingestion_task(&self.worker_id)
            .await
            .map_err(|error| IngestionError::TaskSource(error.to_string()))
    }

    async fn complete(&mut self, task: &IngestionTask) -> Result<(), IngestionError> {
        self.repo
            .complete_ingestion_task(&task.task_id)
            .await
            .map_err(|error| IngestionError::TaskSource(error.to_string()))
    }

    async fn fail(&mut self, task: &IngestionTask, error: &str) -> Result<(), IngestionError> {
        self.repo
            .fail_ingestion_task(&task.task_id, error)
            .await
            .map_err(|err| IngestionError::TaskSource(err.to_string()))
    }
}

struct PgAuditSink {
    repo: PgAppRepository,
}

#[async_trait::async_trait]
impl AuditSink for PgAuditSink {
    async fn record(&mut self, record: AuditRecord) -> Result<(), IngestionError> {
        self.repo
            .append_audit_record(&record)
            .await
            .map_err(|error| IngestionError::AuditSink(error.to_string()))
    }
}

struct PgStateSink {
    repo: PgAppRepository,
}

#[async_trait::async_trait]
impl StateSink for PgStateSink {
    async fn transition(
        &mut self,
        task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        ingestion::DocumentStateMachine::validate(&transition)?;
        let context = task_context(task);
        let document_id = Uuid::parse_str(&task.document_id)
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        self.repo
            .set_document_status(&context, document_id, transition.to.clone())
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        self.repo
            .append_audit_record(&AuditRecord {
                audit_id: common::new_id(),
                org_id: task.org_id.clone(),
                actor_id: task.requested_by.clone(),
                action: ingestion::AuditAction::StateTransition,
                resource_type: "document".to_string(),
                resource_id: task.document_id.clone(),
                payload: serde_json::json!({
                    "from": transition.from,
                    "to": transition.to,
                    "task_id": task.task_id,
                }),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        if matches!(
            transition.to,
            common::DocumentStatus::Completed | common::DocumentStatus::Failed
        ) {
            if matches!(transition.to, common::DocumentStatus::Failed) {
                if let Some(user_id) = task
                    .requested_by
                    .as_deref()
                    .and_then(|value| Uuid::parse_str(value).ok())
                {
                    let analytics = analytics::AnalyticsService::new(self.repo.raw().clone());
                    let event = analytics::ProductEvent {
                        event_id: Uuid::new_v4(),
                        event_time: chrono::Utc::now(),
                        user_id,
                        session_id: None,
                        notebook_id: Uuid::parse_str(&task.notebook_id).ok(),
                        surface: analytics::Surface::Workspace,
                        event_name: analytics::ProductEventName::DocumentUploadFailed,
                        result: analytics::ResultTag::Failure,
                        request_id: None,
                        trace_id: None,
                        client_platform: "worker".to_string(),
                        metadata: serde_json::json!({
                            "document_id": task.document_id.clone(),
                            "task_id": task.task_id.clone(),
                        }),
                    };
                    if let Err(error) = analytics.record_product_event(&event).await {
                        info!(error = %error, document_id = task.document_id, "failed to record document upload failure event");
                    }
                }
            }
            if let Some(user_id) = task
                .requested_by
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
            {
                let title = if matches!(transition.to, common::DocumentStatus::Completed) {
                    "Document ingestion completed"
                } else {
                    "Document ingestion failed"
                };
                let body = if matches!(transition.to, common::DocumentStatus::Completed) {
                    "A document finished ingestion and is ready for retrieval."
                } else {
                    "A document failed ingestion and needs attention."
                };
                let _ = self
                    .repo
                    .create_notification(
                        &context,
                        NotificationCreateParams {
                            user_id,
                            event_type: if matches!(
                                transition.to,
                                common::DocumentStatus::Completed
                            ) {
                                "ingestion.success".to_string()
                            } else {
                                "ingestion.failed".to_string()
                            },
                            title: title.to_string(),
                            body: body.to_string(),
                            data: serde_json::json!({
                                "document_id": task.document_id.clone(),
                                "task_id": task.task_id.clone(),
                                "status": format!("{:?}", transition.to),
                            }),
                            channels: vec!["in_app".to_string()],
                        },
                    )
                    .await;
            }
        }

        Ok(())
    }
}

struct PgTaskProcessor {
    repo: PgAppRepository,
    object_store: ObjectStoreHandle,
    qdrant: Option<HttpQdrantBackend>,
    qdrant_collection: String,
    embedding_dim: usize,
    embedding_client: Option<avrag_llm::EmbeddingClient>,
    mm_embedding_client: Option<avrag_llm::EmbeddingClient>,
    asset_url_ttl_secs: u64,
    redis_lock: Option<DocumentLock>,
    summary_generator: Option<avrag_llm::SummaryGenerator>,
    analytics: Option<analytics::AnalyticsService>,
    usage_limit: Option<avrag_usage_limit::UsageLimitService>,
    mineru_client: Option<MineruClient>,
}

#[async_trait::async_trait]
impl TaskProcessor for PgTaskProcessor {
    async fn process(&mut self, task: &IngestionTask) -> Result<(), IngestionError> {
        let task_kind = worker_task_kind(task);
        telemetry::prometheus::observe_worker_task_started(task_kind);
        let started_at = std::time::Instant::now();
        let result = async {
            let context = task_context(task);
            let document_id = Uuid::parse_str(&task.document_id)
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            let _lock_guard = if let Some(ref lock) = self.redis_lock {
                match lock.try_acquire(document_id).await {
                    Ok(Some(guard)) => Some(guard),
                    Ok(None) => {
                        info!(%document_id, "skipping document — lock held by another worker");
                        return Ok(());
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            let object_path = match &task.payload {
                ingestion::IngestionTaskPayload::IngestDocument(payload) => payload.object_path.clone(),
                ingestion::IngestionTaskPayload::ReindexDocument(_) => {
                    let seed = self
                        .repo
                        .get_document_task_seed(&context, document_id)
                        .await
                        .map_err(|error| IngestionError::StateSink(error.to_string()))?
                        .ok_or_else(|| {
                            IngestionError::StateSink("document seed not found".to_string())
                        })?;
                    seed.object_path
                }
            };

        let bytes = self
            .object_store
            .get(&object_path)
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;
        let filename = Path::new(&object_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| object_path.clone());

        let route_decision = ParseRouter::route(&bytes, &filename)
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        info!(
            filename = %filename,
            route = ?route_decision.route,
            reason = %route_decision.reason,
            "Document routing decision"
        );

        let parse_start = std::time::Instant::now();
        let normalized_doc = match route_decision.route {
            ParseRoute::Local => {
                if let Some(parser) = ParserFactory::create_parser(&filename) {
                    match parser.parse(&bytes, &filename).await {
                        Ok(doc) => ingestion::parser::normalize_parsed_document(&doc, "local"),
                        Err(_) => fallback_normalized_doc(&filename, &bytes),
                    }
                } else {
                    fallback_normalized_doc(&filename, &bytes)
                }
            }
            ParseRoute::MineruPrecise => {
                let mineru = self.mineru_client.as_ref().ok_or_else(|| {
                    IngestionError::StateSink(format!(
                        "MinerU route selected for {filename} ({}) but MINERU is not configured",
                        route_decision.reason
                    ))
                })?;
                mineru.parse(&bytes, &filename).await.map_err(|error| {
                    IngestionError::StateSink(format!(
                        "MinerU precise parse failed for {filename}: {error}"
                    ))
                })?
            }
        };

        let parse_duration = parse_start.elapsed();
        let text_unit_count = normalized_doc
            .units
            .iter()
            .filter(|u| u.kind == ingestion::parser::ParsedUnitKind::Text)
            .count();
        let image_unit_count = normalized_doc
            .units
            .iter()
            .filter(|u| u.kind == ingestion::parser::ParsedUnitKind::ImageWithContext)
            .count();

        info!(
            filename = %filename,
            route = ?route_decision.route,
            parse_duration_ms = parse_duration.as_millis(),
            text_units = text_unit_count,
            image_units = image_unit_count,
            "Document parsing completed"
        );

        let content = normalized_doc
            .units
            .iter()
            .filter(|u| u.kind == ingestion::parser::ParsedUnitKind::Text)
            .map(|u| u.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let chunk_plan = ingestion::chunker::build_chunk_plan(
            &normalized_doc,
            &filename,
            &ChunkPolicy::default(),
        );
        let body_items: Vec<common::ParsedPreviewItem> = chunk_plan
            .text_chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| common::ParsedPreviewItem {
                kind: "paragraph".to_string(),
                text: chunk.text.clone(),
                page: chunk.page as usize,
                cursor: i,
            })
            .collect();

        info!(
            filename = %filename,
            text_chunks = chunk_plan.text_chunks.len(),
            multimodal_chunks = chunk_plan.multimodal_chunks.len(),
            "Chunk plan generated"
        );

        let chunks = self
            .repo
            .store_document_body_items(&context, document_id, &content, &body_items)
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;
        let processed_chunk_count = chunks.len().max(1);

        #[derive(Clone)]
        struct StoredMultimodalChunk {
            chunk_id: Uuid,
            asset_id: Uuid,
            image_path: String,
            caption: Option<String>,
            context_text: String,
            page: i64,
            parser_backend: String,
        }

        let mut stored_multimodal_chunks = Vec::new();
        for multimodal_chunk in &chunk_plan.multimodal_chunks {
            let asset_id = Uuid::parse_str(&multimodal_chunk.asset_id).map_err(|error| {
                IngestionError::StateSink(format!("invalid multimodal asset id: {error}"))
            })?;
            let chunk_id = Uuid::parse_str(&multimodal_chunk.chunk_id).map_err(|error| {
                IngestionError::StateSink(format!("invalid multimodal chunk id: {error}"))
            })?;
            let parser_backend = normalized_doc
                .units
                .iter()
                .find(|u| u.unit_id == multimodal_chunk.asset_id)
                .map(|u| u.parser_backend.clone())
                .unwrap_or_else(|| "unknown".to_string());

            let object_key = build_asset_object_key(
                &context,
                &task.notebook_id,
                &task.document_id,
                asset_id,
                &multimodal_chunk.image_path,
            );
            let stored_image_path = if is_remote_media_reference(&multimodal_chunk.image_path) {
                match mirror_remote_asset(
                    &self.object_store,
                    &multimodal_chunk.image_path,
                    &object_key,
                    self.asset_url_ttl_secs,
                )
                .await
                {
                    Ok(path) => path,
                    Err(error) => {
                        info!(
                            asset_id = %asset_id,
                            error = %error,
                            "Failed to mirror multimodal asset; using original URL"
                        );
                        multimodal_chunk.image_path.clone()
                    }
                }
            } else {
                multimodal_chunk.image_path.clone()
            };

            self.repo
                .store_document_asset(
                    &context,
                    avrag_storage_pg::StoreDocumentAssetParams {
                        asset_id,
                        notebook_id: Uuid::parse_str(&task.notebook_id)
                            .unwrap_or_else(|_| Uuid::nil()),
                        document_id,
                        page: Some(multimodal_chunk.page as i32),
                        asset_kind: "image".to_string(),
                        storage_path: Some(stored_image_path.clone()),
                        mime_type: None,
                        width: None,
                        height: None,
                        caption: multimodal_chunk.caption.clone(),
                        parser_backend: parser_backend.clone(),
                    },
                )
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            self.repo
                .store_multimodal_chunk(
                    &context,
                    avrag_storage_pg::StoreMultimodalChunkParams {
                        chunk_id,
                        notebook_id: Uuid::parse_str(&task.notebook_id)
                            .unwrap_or_else(|_| Uuid::nil()),
                        document_id,
                        asset_id: Some(asset_id),
                        page: Some(multimodal_chunk.page as i32),
                        context_text: Some(multimodal_chunk.context_text.clone()),
                        caption: multimodal_chunk.caption.clone(),
                        normalized_text: multimodal_chunk.context_text.clone(),
                        parser_backend: parser_backend.clone(),
                        metadata: serde_json::json!({
                            "source_image_path": multimodal_chunk.image_path,
                            "mirrored_object_key": object_key,
                        }),
                    },
                )
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            stored_multimodal_chunks.push(StoredMultimodalChunk {
                chunk_id,
                asset_id,
                image_path: stored_image_path,
                caption: multimodal_chunk.caption.clone(),
                context_text: multimodal_chunk.context_text.clone(),
                page: i64::from(multimodal_chunk.page),
                parser_backend,
            });
        }

        if let Some(ref summary_gen) = self.summary_generator {
            let user_uuid = task
                .requested_by
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            let mut skip_llm_summary = false;

            if let (Some(svc), Some(user_id)) = (&self.usage_limit, user_uuid) {
                match svc.check_quota(context.org_id().into_uuid(), user_id).await {
                    Ok(quota) => {
                        if quota.blocked_5h || quota.blocked_7d {
                            info!(
                                document_id = %document_id,
                                user_id = %user_id,
                                blocked_5h = quota.blocked_5h,
                                blocked_7d = quota.blocked_7d,
                                "skipping LLM summary generation because user quota is exhausted"
                            );
                            skip_llm_summary = true;
                        }
                    }
                    Err(error) => {
                        info!(
                            document_id = %document_id,
                            error = %error,
                            "quota check failed for summary generation; skipping LLM summary (fail-closed)"
                        );
                        skip_llm_summary = true;
                    }
                }
            }

            if !skip_llm_summary {
                let title = normalized_doc.title.clone();
                let generated_summary = summary_gen
                    .synthesize(&document_id.to_string(), &title, &filename, &content)
                    .await;
                match generated_summary {
                    Ok((summary, llm_usage)) => {
                        if let Err(e) = self
                            .repo
                            .update_document_summary(&context, document_id, &summary)
                            .await
                        {
                            info!(document_id = %document_id, error = %e, "failed to update document summary with LLM result");
                        } else {
                            info!(document_id = %document_id, "successfully updated document summary with LLM result");
                        }
                        if let (Some(svc), Some(user_id)) = (&self.usage_limit, user_uuid) {
                            let ctx = avrag_usage_limit::MeteringContext {
                                user_id,
                                org_id: context.org_id().into_uuid(),
                                feature: avrag_usage_limit::BillableFeature::Summary,
                                stage: "worker_summary".to_string(),
                                session_id: None,
                                document_id: Some(document_id),
                                request_id: None,
                                trace_id: None,
                            };
                            if let Err(e) = svc
                                .record_usage(
                                    &ctx,
                                    &llm_usage.provider,
                                    &llm_usage.model,
                                    llm_usage.prompt_tokens,
                                    llm_usage.completion_tokens,
                                    llm_usage.total_tokens,
                                    avrag_usage_limit::UsageSource::Actual,
                                )
                                .await
                            {
                                info!(document_id = %document_id, error = %e, "failed to record summary usage");
                            }
                        }
                        if let (Some(analytics), Some(user_id)) = (&self.analytics, user_uuid) {
                            let event = analytics::CostEvent {
                                event_id: Uuid::new_v4(),
                                event_time: chrono::Utc::now(),
                                user_id,
                                session_id: None,
                                notebook_id: None,
                                event_name: analytics::CostEventName::SummaryUsageMetered,
                                feature: "summary".to_string(),
                                provider: if llm_usage.provider.trim().is_empty() {
                                    "unknown".to_string()
                                } else {
                                    llm_usage.provider.clone()
                                },
                                model: if llm_usage.model.trim().is_empty() {
                                    "unknown".to_string()
                                } else {
                                    llm_usage.model.clone()
                                },
                                prompt_tokens: i64::from(llm_usage.prompt_tokens),
                                completion_tokens: i64::from(llm_usage.completion_tokens),
                                embedding_tokens: 0,
                                usage_units: avrag_usage_limit::compute_usage_units(
                                    &llm_usage.provider,
                                    &llm_usage.model,
                                    llm_usage.prompt_tokens,
                                    llm_usage.completion_tokens,
                                ),
                                storage_bytes_delta: 0,
                                external_call_count: 0,
                                source: "worker".to_string(),
                                metadata: serde_json::json!({
                                    "task_id": task.task_id.clone(),
                                    "document_id": document_id,
                                    "filename": filename,
                                }),
                            };
                            if let Err(error) = analytics.record_cost_event(&event).await {
                                info!(document_id = %document_id, error = %error, "failed to record summary analytics event");
                            }
                        }
                    }
                    Err(e) => {
                        info!(document_id = %document_id, error = %e, "Summary generation failed, keeping naive fallback");
                    }
                }
            }
        }

        if let Some(qdrant) = &self.qdrant {
            let collection = QdrantCollectionConfig {
                name: self.qdrant_collection.clone(),
                vector_size: self.embedding_dim as u64,
                distance: QdrantDistance::Cosine,
            };
            qdrant
                .ensure_collection(&collection)
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;
            let filter = SecureQdrantFilterBuilder::with_doc_filter(&context, document_id)
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;
            qdrant
                .delete_points_by_filter(&self.qdrant_collection, &filter)
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
            let embeddings = if let Some(ref client) = self.embedding_client {
                client
                    .embed(&texts)
                    .await
                    .map_err(|e| IngestionError::StateSink(format!("embedding failed: {e}")))?
            } else {
                vec![vec![0f32; self.embedding_dim]; texts.len()]
            };

            let mut points = Vec::with_capacity(chunks.len());
            for (chunk, vector) in chunks.into_iter().zip(embeddings) {
                let chunk_id = Uuid::parse_str(&chunk.chunk_id).map_err(|error| {
                    IngestionError::StateSink(format!("invalid chunk id: {error}"))
                })?;
                let doc_id = Uuid::parse_str(&chunk.doc_id).map_err(|error| {
                    IngestionError::StateSink(format!("invalid doc id: {error}"))
                })?;
                points.push(QdrantPointUpsert {
                    chunk_id,
                    doc_id,
                    org_id: context.org_id(),
                    page: chunk.page,
                    vector,
                    doc_version: 1,
                });
            }

            qdrant
                .upsert_points(&self.qdrant_collection, &points)
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            if !stored_multimodal_chunks.is_empty() {
                if let Some(ref client) = self.mm_embedding_client {
                    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
                    let client = client.clone();
                    let mut handles: Vec<
                        tokio::task::JoinHandle<anyhow::Result<(usize, Vec<f32>)>>,
                    > = Vec::with_capacity(stored_multimodal_chunks.len());

                    for (idx, chunk) in stored_multimodal_chunks.iter().enumerate() {
                        let input = if is_remote_media_reference(&chunk.image_path) {
                            MultiModalEmbeddingInput::text_image(
                                chunk.context_text.clone(),
                                chunk.image_path.clone(),
                            )
                        } else {
                            MultiModalEmbeddingInput::text(chunk.context_text.clone())
                        };
                        let sem = semaphore.clone();
                        let c = client.clone();
                        handles.push(tokio::spawn(async move {
                            let _permit =
                                sem.acquire().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                            let vector = c.embed_multimodal_fused(&input, None).await?;
                            Ok((idx, vector))
                        }));
                    }

                    let mut indexed_embeddings = Vec::with_capacity(handles.len());
                    for handle in handles {
                        let (idx, vector) = handle
                            .await
                            .map_err(|e| IngestionError::StateSink(format!("task join: {e}")))?
                            .map_err(|e| {
                                IngestionError::StateSink(format!("multimodal embedding: {e}"))
                            })?;
                        indexed_embeddings.push((idx, vector));
                    }
                    indexed_embeddings.sort_by_key(|(idx, _)| *idx);
                    let multimodal_embeddings: Vec<Vec<f32>> =
                        indexed_embeddings.into_iter().map(|(_, v)| v).collect();

                    if let Some(first_vector) = multimodal_embeddings.first() {
                        let multimodal_collection_name =
                            format!("{}_multimodal", self.qdrant_collection);
                        let multimodal_collection = QdrantCollectionConfig {
                            name: multimodal_collection_name.clone(),
                            vector_size: first_vector.len() as u64,
                            distance: QdrantDistance::Cosine,
                        };
                        qdrant
                            .ensure_collection(&multimodal_collection)
                            .await
                            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

                        let multimodal_filter =
                            SecureQdrantFilterBuilder::with_doc_filter(&context, document_id)
                                .map_err(|error| IngestionError::StateSink(error.to_string()))?;
                        qdrant
                            .delete_points_by_filter(
                                &multimodal_collection_name,
                                &multimodal_filter,
                            )
                            .await
                            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

                        let multimodal_points = stored_multimodal_chunks
                            .iter()
                            .zip(multimodal_embeddings)
                            .map(|(chunk, vector)| {
                                avrag_storage_qdrant::MultimodalQdrantPointUpsert {
                                    chunk_id: chunk.chunk_id,
                                    doc_id: document_id,
                                    asset_id: chunk.asset_id,
                                    org_id: context.org_id(),
                                    page: Some(chunk.page),
                                    vector,
                                    caption: chunk.caption.clone(),
                                    parser_backend: chunk.parser_backend.clone(),
                                    doc_version: 1,
                                }
                            })
                            .collect::<Vec<_>>();

                        qdrant
                            .upsert_multimodal_points(
                                &multimodal_collection_name,
                                &multimodal_points,
                            )
                            .await
                            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

                        info!(
                            document_id = %document_id,
                            multimodal_chunks = stored_multimodal_chunks.len(),
                            "Stored multimodal chunks"
                        );
                    }
                } else {
                    info!(
                        document_id = %document_id,
                        multimodal_chunks = stored_multimodal_chunks.len(),
                        "MM embedding not configured; skipping multimodal vector indexing"
                    );
                }
            }
        }

            let embedding_tokens = estimate_token_count(&content);
            let _ = self
                .repo
                .record_usage_event(
                    &context,
                    "pages_processed",
                    i64::try_from(processed_chunk_count).unwrap_or(1),
                    "worker_ingestion",
                )
                .await;
            let _ = self
                .repo
                .record_usage_event(
                    &context,
                    "embedding_tokens",
                    embedding_tokens,
                    "worker_ingestion",
                )
                .await;
            if let Some(ref analytics) = self.analytics {
                if let Some(user_id) = task
                    .requested_by
                    .as_deref()
                    .and_then(|value| Uuid::parse_str(value).ok())
                {
                    let event = analytics::CostEvent {
                        event_id: Uuid::new_v4(),
                        event_time: chrono::Utc::now(),
                        user_id,
                        session_id: None,
                        notebook_id: Uuid::parse_str(&task.notebook_id).ok(),
                        event_name: analytics::CostEventName::EmbeddingUsageMetered,
                        feature: "embedding".to_string(),
                        provider: "worker".to_string(),
                        model: "document_embedding".to_string(),
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        embedding_tokens,
                        usage_units: embedding_tokens,
                        storage_bytes_delta: 0,
                        external_call_count: 0,
                        source: "worker_ingestion".to_string(),
                        metadata: serde_json::json!({
                            "task_id": task.task_id.clone(),
                            "document_id": task.document_id.clone(),
                            "task_kind": task_kind,
                        }),
                    };
                    if let Err(error) = analytics.record_cost_event(&event).await {
                        info!(document_id = %document_id, error = %error, "failed to record embedding analytics event");
                    }
                }
            }
            Ok(())
        }
        .await;
        telemetry::prometheus::observe_worker_task_completed(
            task_kind,
            if result.is_ok() { "success" } else { "failure" },
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        result
    }
}

fn worker_task_kind(task: &IngestionTask) -> &'static str {
    match task.kind {
        ingestion::IngestionTaskKind::IngestDocument => "ingest_document",
        ingestion::IngestionTaskKind::ReindexDocument => "reindex_document",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    telemetry::init("avrag-worker")?;
    let config = AppConfig::from_env();
    let database_url = config.database_url.clone();
    let state = AppState::bootstrap(config).await?;
    let embedding_dim = state.config().embedding.dimensions.unwrap_or(64);
    let heartbeat_secs = std::env::var("AVRAG_WORKER_HEARTBEAT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30);
    let poll_secs = std::env::var("AVRAG_WORKER_POLL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5);
    let worker_id =
        std::env::var("AVRAG_WORKER_ID").unwrap_or_else(|_| format!("worker-{}", common::new_id()));
    let mut poll_interval = interval(Duration::from_secs(poll_secs));
    let mut heartbeat_interval = interval(Duration::from_secs(heartbeat_secs));

    info!(
        runtime_mode = state.runtime_mode(),
        heartbeat_secs, poll_secs, "avrag worker skeleton started"
    );

    if let Some(database_url) = database_url {
        let repo = PgAppRepository::connect(&database_url).await?;
        if state.config().auto_migrate {
            repo.migrate().await?;
        }
        let analytics_pool = repo.raw().clone();
        let mut analytics_job_runner = analytics_jobs::AnalyticsJobRunner::from_env(analytics_pool.clone());
        let usage_limit_pool = repo.raw().clone();
        let mut worker = WorkerRuntime::new(
            PgTaskSource {
                repo: repo.clone(),
                worker_id: worker_id.clone(),
            },
            PgAuditSink { repo: repo.clone() },
            PgStateSink { repo: repo.clone() },
            PgTaskProcessor {
                repo,
                object_store: build_worker_object_store(state.config()).await?,
                qdrant: Some(HttpQdrantBackend::new(state.config().qdrant.url.clone()))
                    .filter(|_| !state.config().qdrant.url.trim().is_empty()),
                qdrant_collection: state.config().qdrant.collection.clone(),
                embedding_dim,
                embedding_client: {
                    let ec = &state.config().embedding;
                    ec.to_llm_config().map(avrag_llm::EmbeddingClient::new)
                },
                asset_url_ttl_secs: state.config().object_storage.download_url_expire_sec,
                mm_embedding_client: {
                    let ec = &state.config().mm_embedding;
                    ec.to_llm_config().map(avrag_llm::EmbeddingClient::new)
                },
                redis_lock: {
                    let url = &state.config().redis.url;
                    if !url.trim().is_empty() {
                        DocumentLock::new(url).ok()
                    } else {
                        None
                    }
                },
                summary_generator: {
                    let sc = &state.config().summary_llm;
                    if let Some(config) = sc.to_llm_config() {
                        let mut generator = SummaryGenerator::new(config);
                        if let Some(template) =
                            load_prompt_template(state.config(), "summary_generation")
                        {
                            generator = generator.with_prompt_template(template);
                        }
                        if let Some(template) =
                            load_prompt_template(state.config(), "summary_generation_finalize")
                        {
                            generator = generator.with_finalize_prompt_template(template);
                        }
                        Some(generator)
                    } else {
                        None
                    }
                },
                analytics: Some(analytics::AnalyticsService::new(analytics_pool)),
                usage_limit: Some(avrag_usage_limit::UsageLimitService::new(usage_limit_pool)),
                mineru_client: MineruConfig::from_env().map(MineruClient::new),
            },
        );

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("worker shutdown signal received");
                    break;
                }
                _ = poll_interval.tick() => {
                    match worker.run_once().await {
                        Ok(WorkerTick::Idle) => info!("worker poll completed with no tasks"),
                        Ok(WorkerTick::Processed(task)) => {
                            info!(task_id = task.task_id, kind = ?task.kind, "worker processed task");
                        }
                        Err(error) => {
                            info!(error = %error, "worker poll failed");
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if let Some(runner) = analytics_job_runner.as_mut()
                        && let Err(error) = runner.maybe_run().await
                    {
                        telemetry::prometheus::record_dependency_failure("analytics_rollup");
                        info!(error = %error, worker_id, "analytics rollup job failed");
                    }
                    info!(runtime_mode = state.runtime_mode(), worker_id, "worker heartbeat");
                }
            }
        }
        return Ok(());
    }

    let mut worker = WorkerRuntime::new(
        NoopTaskSource,
        NoopAuditSink,
        NoopStateSink,
        NoopTaskProcessor,
    );
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("worker shutdown signal received");
                break;
            }
            _ = poll_interval.tick() => {
                match worker.run_once().await? {
                    WorkerTick::Idle => {
                        info!("worker poll completed with no tasks");
                    }
                    WorkerTick::Processed(task) => {
                        info!(task_id = task.task_id, kind = ?task.kind, "worker processed task");
                    }
                }
            }
            _ = heartbeat_interval.tick() => {
                info!(runtime_mode = state.runtime_mode(), "worker heartbeat");
            }
        }
    }

    Ok(())
}

fn task_context(task: &IngestionTask) -> AuthContext {
    let org_id = Uuid::parse_str(&task.org_id).unwrap_or_else(|_| Uuid::nil());
    let auth = AuthContext::new(OrgId::from(org_id), SubjectKind::System);
    if let Some(requested_by) = task
        .requested_by
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        return auth.with_actor_id(ActorId::new(requested_by));
    }
    auth
}

async fn build_worker_object_store(config: &AppConfig) -> Result<ObjectStoreHandle> {
    if !config.object_storage.endpoint.trim().is_empty()
        && !config.object_storage.bucket.trim().is_empty()
        && !config.object_storage.access_key.trim().is_empty()
        && !config.object_storage.secret_key.trim().is_empty()
    {
        let store = S3ObjectStore::new(
            config.object_storage.endpoint.clone(),
            config.object_storage.bucket.clone(),
            config.object_storage.region.clone(),
            config.object_storage.access_key.clone(),
            config.object_storage.secret_key.clone(),
            config.object_storage.use_path_style,
        )
        .await?;
        return Ok(ObjectStoreHandle::S3(store));
    }
    Ok(ObjectStoreHandle::local(PathBuf::from(
        config.object_root.clone(),
    )))
}

fn estimate_token_count(text: &str) -> i64 {
    common::estimate_token_count(text)
}

fn is_remote_media_reference(path: &str) -> bool {
    common::is_remote_url(path)
}

fn fallback_normalized_doc(filename: &str, bytes: &[u8]) -> NormalizedDocument {
    let fallback_content = String::from_utf8_lossy(bytes).to_string();
    NormalizedDocument {
        title: filename.to_string(),
        units: vec![ParsedUnit::new_text(
            1,
            fallback_content,
            "local_fallback".to_string(),
        )],
        metadata: Default::default(),
    }
}

fn build_asset_object_key(
    context: &AuthContext,
    notebook_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
) -> String {
    let extension = infer_asset_extension(source_path).unwrap_or("bin");
    format!(
        "{}/{}/{}/assets/{}.{}",
        context.org_id(),
        notebook_id,
        document_id,
        asset_id,
        extension
    )
}

fn infer_asset_extension(path: &str) -> Option<&'static str> {
    common::infer_image_extension(path)
}

async fn mirror_remote_asset(
    object_store: &ObjectStoreHandle,
    source_url: &str,
    object_key: &str,
    ttl_secs: u64,
) -> Result<String> {
    let response = reqwest::Client::new()
        .get(source_url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    object_store.put(object_key, &bytes).await?;
    if object_store.is_remote() {
        object_store
            .presigned_get_url(object_key, ttl_secs.max(60))
            .await
    } else {
        Ok(source_url.to_string())
    }
}

fn load_prompt_template(config: &AppConfig, base_name: &str) -> Option<String> {
    let prompts_dir = PathBuf::from(config.prompts.dir.trim());
    let version = config.prompts.summary_version.trim();
    let mut candidates = Vec::new();
    if !version.is_empty() {
        candidates.push(prompts_dir.join(format!("{base_name}.{version}.tmpl")));
        candidates.push(prompts_dir.join(format!("{base_name}_{version}.tmpl")));
    }
    candidates.push(prompts_dir.join(format!("{base_name}.tmpl")));

    candidates.into_iter().find_map(|path| {
        fs::read_to_string(&path)
            .ok()
            .map(|template| template.trim().to_string())
            .filter(|template| !template.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};
    use uuid::Uuid;

    #[test]
    fn load_prompt_template_prefers_versioned_file() {
        let temp_dir = env::temp_dir().join(format!("summary-template-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(
            temp_dir.join("summary_generation.tmpl"),
            "default {{title}}",
        )
        .unwrap();
        fs::write(
            temp_dir.join("summary_generation.v2.tmpl"),
            "versioned {{title}}",
        )
        .unwrap();

        let mut config = AppConfig::default();
        config.prompts.dir = temp_dir.to_string_lossy().to_string();
        config.prompts.summary_version = "v2".to_string();

        let template = load_prompt_template(&config, "summary_generation").unwrap();
        assert_eq!(template, "versioned {{title}}");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
