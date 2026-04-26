impl AppState {
    pub async fn execute_rag_execute_plan(
        &self,
        req: common::ExecutePlanRequest,
    ) -> Result<common::ExecutePlanResponse, AppError> {
        req.validate()
            .map_err(|error| AppError::validation("invalid_execute_plan", error.to_string()))?;
        self.validate_execute_plan_doc_scope(&req).await?;

        if let Some(rag_runtime) = &self.rag_runtime {
            return rag_runtime
                .execute_plan(&req, &self.auth)
                .await
                .map_err(map_anyhow_error);
        }

        self.execute_rag_execute_plan_memory_compat(req).await
    }

    async fn validate_execute_plan_doc_scope(
        &self,
        req: &common::ExecutePlanRequest,
    ) -> Result<(), AppError> {
        if req.doc_scope.is_empty() {
            return Err(AppError::validation(
                "invalid_doc_scope",
                "doc_scope must not be empty",
            ));
        }

        let doc_ids = req
            .doc_scope
            .iter()
            .map(|id| {
                Uuid::parse_str(id).map_err(|_| {
                    AppError::validation(
                        "invalid_doc_scope",
                        format!("doc_scope contains an invalid document id: {id}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unique_doc_ids = doc_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();

        if let Some(pg) = &self.pg {
            let unique_doc_ids = unique_doc_ids.iter().copied().collect::<Vec<_>>();
            let states = pg
                .get_document_scope_states(&self.auth, &unique_doc_ids)
                .await
                .map_err(map_pg_error)?;
            if states.len() != unique_doc_ids.len() {
                return Err(AppError::validation(
                    "invalid_doc_scope",
                    "doc_scope contains a document that does not exist or is not accessible",
                ));
            }
            if let Some(DocumentScopeState {
                document_id,
                status,
            }) = states
                .into_iter()
                .find(|state| !matches!(state.status, DocumentStatus::Completed))
            {
                return Err(AppError::validation(
                    "invalid_doc_scope",
                    format!("document {document_id} is not ready for RAG execution: {status:?}"),
                ));
            }
            return Ok(());
        }

        let state = self.inner.read().await;
        for doc_id in &req.doc_scope {
            let Some(stored) = state.documents.get(doc_id) else {
                return Err(AppError::validation(
                    "invalid_doc_scope",
                    format!("document {doc_id} does not exist"),
                ));
            };
            if stored.document.org_id != self.current_org_id() {
                return Err(AppError::validation(
                    "invalid_doc_scope",
                    format!("document {doc_id} is not accessible"),
                ));
            }
            if !matches!(stored.document.status, DocumentStatus::Completed) {
                return Err(AppError::validation(
                    "invalid_doc_scope",
                    format!("document {doc_id} is not ready for RAG execution"),
                ));
            }
        }

        Ok(())
    }

    async fn execute_rag_execute_plan_memory_compat(
        &self,
        req: common::ExecutePlanRequest,
    ) -> Result<common::ExecutePlanResponse, AppError> {
        let final_chunk_budget = req
            .budget
            .as_ref()
            .and_then(|budget| budget.final_chunk_budget)
            .unwrap_or(8);
        let total_candidate_budget = req
            .budget
            .as_ref()
            .and_then(|budget| budget.total_candidate_budget)
            .unwrap_or(final_chunk_budget);
        let state = self.inner.read().await;
        let mut documents = state
            .documents
            .values()
            .filter(|stored| stored.document.org_id == self.current_org_id())
            .filter(|stored| matches!(stored.document.status, DocumentStatus::Completed))
            .filter(|stored| req.doc_scope.contains(&stored.document.id))
            .cloned()
            .collect::<Vec<_>>();

        documents.sort_by(|left, right| {
            let left_score = memory_execute_plan_score(&left.content, &req.items);
            let right_score = memory_execute_plan_score(&right.content, &req.items);
            right_score
                .partial_cmp(&left_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let chunks = documents
            .iter()
            .take(final_chunk_budget)
            .map(|stored| common::RetrievedChunk {
                chunk_id: stored.document.id.clone(),
                doc_id: stored.document.id.clone(),
                chunk_type: "text".to_string(),
                page: Some(1),
                text: stored.content.chars().take(600).collect(),
                score: memory_execute_plan_score(&stored.content, &req.items),
                retrieval_channel: "memory_compat".to_string(),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
            })
            .collect::<Vec<_>>();

        let citations = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| common::Citation {
                citation_id: (index + 1) as i64,
                doc_id: chunk.doc_id.clone(),
                chunk_id: Some(chunk.chunk_id.clone()),
                page: Some(1),
                doc_name: documents
                    .iter()
                    .find(|stored| stored.document.id == chunk.doc_id)
                    .map(|stored| stored.document.file_name.clone())
                    .unwrap_or_else(|| format!("Document {}", chunk.doc_id)),
                preview: Some(chunk.text.chars().take(100).collect()),
                content: Some(chunk.text.clone()),
                score: chunk.score,
                layer: Some(chunk.retrieval_channel.clone()),
                chunk_type: Some(chunk.chunk_type.clone()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
            })
            .collect::<Vec<_>>();

        let remaining_summary_budget = final_chunk_budget.saturating_sub(chunks.len());
        let summary_chunks = if req.summary_mode == common::ExecutePlanSummaryMode::None
            || remaining_summary_budget == 0
        {
            Vec::new()
        } else {
            documents
                .iter()
                .take(remaining_summary_budget)
                .map(|stored| common::AnswerContextChunk {
                    chunk_id: format!("summary-{}", stored.document.id),
                    doc_id: Some(stored.document.id.clone()),
                    chunk_type: "summary".to_string(),
                    page: None,
                    text: format!(
                        "[Document Summary] {}",
                        stored
                            .summary
                            .clone()
                            .filter(|summary| !summary.trim().is_empty())
                            .unwrap_or_else(|| build_summary(&stored.content))
                    ),
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                })
                .collect::<Vec<_>>()
        };

        let matched_doc_count = chunks
            .iter()
            .map(|chunk| chunk.doc_id.clone())
            .chain(
                summary_chunks
                    .iter()
                    .filter_map(|chunk| chunk.doc_id.clone()),
            )
            .collect::<std::collections::HashSet<_>>()
            .len();

        Ok(common::ExecutePlanResponse {
            bundle: common::RetrievalBundle {
                chunks: chunks.clone(),
                citations,
                summary_chunks: summary_chunks.clone(),
            },
            coverage: common::Coverage {
                requested_doc_count: req.doc_scope.len(),
                matched_doc_count,
                retrieved_chunk_count: chunks.len(),
                summary_chunk_count: summary_chunks.len(),
            },
            degrade_trace: vec![common::DegradeTraceItem {
                stage: "rag_execute_plan".to_string(),
                reason: "rag_runtime_not_configured".to_string(),
                impact: "Used memory compatibility retrieval instead of the configured backend."
                    .to_string(),
            }],
            backend_trace: common::BackendTrace {
                trace: req.trace.clone(),
                item_trace: Vec::new(),
                retrieval_trace: common::RagTraceSummary {
                    item_count: req.items.len(),
                    total_candidate_budget,
                    max_rerank_docs: chunks.len(),
                    max_final_chunks: final_chunk_budget,
                    top_k_returned: chunks.len(),
                    summary_mode: req.summary_mode.as_str().to_string(),
                    items: Vec::new(),
                },
            },
        })
    }
}

fn memory_execute_plan_score(content: &str, items: &[common::ExecutePlanItem]) -> f32 {
    let haystack = content.to_ascii_lowercase();
    let mut best_score = 0.1f32;

    for item in items {
        if let Some(query) = item.query.as_deref() {
            let tokens = query
                .split_whitespace()
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>();
            let matched = tokens
                .iter()
                .filter(|token| haystack.contains(token.as_str()))
                .count();
            if !tokens.is_empty() {
                best_score = best_score.max(item.priority * (matched as f32 / tokens.len() as f32));
            }
        }
        if let Some(terms) = item.bm25_terms.as_ref() {
            let matched = terms
                .iter()
                .map(|term| term.trim().to_ascii_lowercase())
                .filter(|term| !term.is_empty() && haystack.contains(term.as_str()))
                .count();
            if !terms.is_empty() {
                best_score = best_score.max(item.priority * (matched as f32 / terms.len() as f32));
            }
        }
    }

    best_score
}
