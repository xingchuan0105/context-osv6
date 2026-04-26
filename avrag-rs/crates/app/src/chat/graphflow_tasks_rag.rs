#[derive(Clone)]
struct RagPreparePlannerInputTask {
    state: AppState,
}

#[async_trait]
impl Task for RagPreparePlannerInputTask {
    fn id(&self) -> &str {
        TASK_RAG_PREPARE_PLANNER_INPUT
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_PREPARE_PLANNER_INPUT,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = flow.session().await?;

        self.state
            .get_notebook(&session.notebook_id)
            .await
            .ok_or_else(|| {
                graph_app_error(AppError::not_found(
                    "notebook_not_found",
                    "notebook not found",
                ))
            })?;

        if !request.doc_scope.is_empty() {
            let docscope_metadata = self
                .state
                .load_docscope_metadata(&request.doc_scope)
                .await
                .map_err(graph_app_error)?;
            info!(
                node = TASK_RAG_PREPARE_PLANNER_INPUT,
                doc_scope_count = request.doc_scope.len(),
                docscope_documents = docscope_metadata.documents.len(),
                docscope_languages = ?docscope_metadata.profile.languages,
                docscope_domains = ?docscope_metadata.profile.domains,
                "rag planner docscope metadata prepared"
            );
            flow.set_docscope_metadata(&docscope_metadata).await;
        }

        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagCallPlannerTask {
    state: AppState,
}

#[async_trait]
impl Task for RagCallPlannerTask {
    fn id(&self) -> &str {
        TASK_RAG_CALL_PLANNER
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_RAG_CALL_PLANNER, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = flow.session().await?;
        let mut degrade_trace = flow.degrade_trace().await.unwrap_or_default();
        let docscope_metadata = flow.docscope_metadata().await;
        let plan_result = self
            .state
            .plan_rag_with_main_agent(
                &request,
                Some(&session),
                docscope_metadata.as_ref(),
                &mut degrade_trace,
            )
            .await;
        if let Some(usage) = plan_result.llm_usage.as_ref() {
            self.state
                .record_llm_usage_if_available(
                    avrag_usage_limit::BillableFeature::Chat,
                    "main_agent_rag_plan",
                    usage,
                    "graphflow",
                )
                .await;
        }

        let execute_request = match plan_result.decision {
            crate::main_agent::MainAgentRagPlanDecision::Execute(execute_request) => {
                execute_request
            }
            crate::main_agent::MainAgentRagPlanDecision::Clarify(message) => {
                let execution = self
                    .state
                    .execute_clarify_mode_core(&request, &session, &message)
                    .await
                    .map_err(graph_app_error)?;
                flow.set_execution(&execution).await;
                flow.set_degrade_trace(&degrade_trace).await;
                return Ok(TaskResult::new(
                    None,
                    NextAction::GoTo(TASK_OUTPUT_GUARD.to_string()),
                ));
            }
        };

        let rag_plan = execute_request.to_rag_plan_compat();

        info!(
            node = TASK_RAG_CALL_PLANNER,
            summary_mode = %execute_request.summary_mode.as_str(),
            items = ?rag_plan_items_for_log(&rag_plan),
            "main agent rag execute-plan request ready"
        );
        flow.set_rag_plan(&rag_plan).await;
        flow.set_rag_execute_request(&execute_request).await;
        flow.set_degrade_trace(&degrade_trace).await;

        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagNormalizePlanTask {
    state: AppState,
}

#[async_trait]
impl Task for RagNormalizePlanTask {
    fn id(&self) -> &str {
        TASK_RAG_NORMALIZE_PLAN
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_RAG_NORMALIZE_PLAN, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let execute_request = flow.rag_execute_request().await?;
        let rag_plan = execute_request.to_rag_plan_compat();
        info!(
            node = TASK_RAG_NORMALIZE_PLAN,
            summary_mode = %rag_plan_summary_mode(&rag_plan),
            "main agent rag execute-plan request validated"
        );
        flow.set_rag_plan(&rag_plan).await;
        let _ = &self.state;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagExecutePlanTask {
    state: AppState,
}

#[async_trait]
impl Task for RagExecutePlanTask {
    fn id(&self) -> &str {
        TASK_RAG_EXECUTE_PLAN
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_RAG_EXECUTE_PLAN, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let execute_request = flow.rag_execute_request().await?;
        let execute_response = self
            .state
            .execute_rag_execute_plan(execute_request)
            .await
            .map_err(graph_app_error)?;
        let mut degrade_trace = flow.degrade_trace().await.unwrap_or_default();
        degrade_trace.extend(execute_response.degrade_trace.clone());
        flow.set_degrade_trace(&degrade_trace).await;
        info!(
            node = TASK_RAG_EXECUTE_PLAN,
            retrieved_chunk_count = execute_response.coverage.retrieved_chunk_count,
            summary_chunk_count = execute_response.coverage.summary_chunk_count,
            matched_doc_count = execute_response.coverage.matched_doc_count,
            "rag execute-plan completed"
        );
        flow.set_rag_execute_response(&execute_response).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagRetrieveTextDenseTask {
    state: AppState,
}

#[async_trait]
impl Task for RagRetrieveTextDenseTask {
    fn id(&self) -> &str {
        TASK_RAG_RETRIEVE_TEXT_DENSE
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_RETRIEVE_TEXT_DENSE,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let rag_plan = flow.rag_plan().await?;
        let rag_runtime = require_rag_runtime(&self.state)?;
        let (lists, degrade_trace) = rag_runtime
            .retrieve_text_dense_stage(&request, &self.state.auth, &rag_plan)
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
        info!(
            node = TASK_RAG_RETRIEVE_TEXT_DENSE,
            dense_list_count = lists.len(),
            dense_hit_count = lists.iter().map(|list| list.chunks.len()).sum::<usize>(),
            "rag text dense retrieval completed"
        );
        flow.set_text_dense_lists(&lists).await;
        append_degrade_trace(&flow, degrade_trace).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagRetrieveBm25Task {
    state: AppState,
}

#[async_trait]
impl Task for RagRetrieveBm25Task {
    fn id(&self) -> &str {
        TASK_RAG_RETRIEVE_BM25
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_RAG_RETRIEVE_BM25, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let rag_plan = flow.rag_plan().await?;
        let rag_runtime = require_rag_runtime(&self.state)?;
        let (lists, degrade_trace) = rag_runtime
            .retrieve_bm25_stage(&request, &self.state.auth, &rag_plan)
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
        info!(
            node = TASK_RAG_RETRIEVE_BM25,
            bm25_list_count = lists.len(),
            bm25_hit_count = lists.iter().map(|list| list.chunks.len()).sum::<usize>(),
            "rag bm25 retrieval completed"
        );
        flow.set_bm25_lists(&lists).await;
        append_degrade_trace(&flow, degrade_trace).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagRetrieveMultimodalDenseTask {
    state: AppState,
}

#[async_trait]
impl Task for RagRetrieveMultimodalDenseTask {
    fn id(&self) -> &str {
        TASK_RAG_RETRIEVE_MULTIMODAL_DENSE
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_RETRIEVE_MULTIMODAL_DENSE,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let rag_plan = flow.rag_plan().await?;
        let rag_runtime = require_rag_runtime(&self.state)?;
        let (chunks, degrade_trace) = rag_runtime
            .retrieve_multimodal_dense_stage(&request, &self.state.auth, &rag_plan)
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
        flow.set_multimodal_pool(&chunks).await;
        append_degrade_trace(&flow, degrade_trace).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagMergeTextRrfTask {
    state: AppState,
}

#[async_trait]
impl Task for RagMergeTextRrfTask {
    fn id(&self) -> &str {
        TASK_RAG_MERGE_TEXT_RRF
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_RAG_MERGE_TEXT_RRF, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let rag_runtime = require_rag_runtime(&self.state)?;
        let text_pool = rag_runtime.merge_text_stage(
            flow.text_dense_lists().await.unwrap_or_default(),
            flow.bm25_lists().await.unwrap_or_default(),
        );
        flow.set_text_pool(&text_pool).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagMultimodalRerankTask {
    state: AppState,
}

#[async_trait]
impl Task for RagMultimodalRerankTask {
    fn id(&self) -> &str {
        TASK_RAG_MULTIMODAL_RERANK
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_MULTIMODAL_RERANK,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let rag_runtime = require_rag_runtime(&self.state)?;
        let (chunks, degrade_trace) = rag_runtime
            .multimodal_rerank_stage(
                &request.query,
                flow.text_pool().await.unwrap_or_default(),
                flow.multimodal_pool().await.unwrap_or_default(),
            )
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
        flow.set_reranked_chunks(&chunks).await;
        append_degrade_trace(&flow, degrade_trace).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagCutFinalCandidatesTask {
    state: AppState,
}

#[async_trait]
impl Task for RagCutFinalCandidatesTask {
    fn id(&self) -> &str {
        TASK_RAG_CUT_FINAL_CANDIDATES
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_CUT_FINAL_CANDIDATES,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let rag_runtime = require_rag_runtime(&self.state)?;
        let final_chunks = rag_runtime
            .cut_final_candidates_stage(flow.reranked_chunks().await.unwrap_or_default());
        info!(
            node = TASK_RAG_CUT_FINAL_CANDIDATES,
            final_chunk_count = final_chunks.len(),
            final_chunks = ?scored_chunks_for_log(&final_chunks),
            "rag final candidate cut completed"
        );
        flow.set_retrieved_chunks(&final_chunks).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagApplySummaryPolicyTask {
    state: AppState,
}

#[async_trait]
impl Task for RagApplySummaryPolicyTask {
    fn id(&self) -> &str {
        TASK_RAG_APPLY_SUMMARY_POLICY
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_APPLY_SUMMARY_POLICY,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let rag_plan = flow.rag_plan().await?;
        let rag_runtime = require_rag_runtime(&self.state)?;
        let retrieved_chunks = flow.retrieved_chunks().await.unwrap_or_default();
        let summaries = rag_runtime
            .apply_summary_policy(&request, &self.state.auth, &rag_plan, &retrieved_chunks)
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
        info!(
            node = TASK_RAG_APPLY_SUMMARY_POLICY,
            summary_mode = %rag_plan_summary_mode(&rag_plan),
            retrieved_chunk_count = retrieved_chunks.len(),
            summary_chunk_count = summaries.len(),
            summary_doc_ids = ?summaries.iter().map(|(doc_id, _)| doc_id.to_string()).collect::<Vec<_>>(),
            "rag summary policy applied"
        );
        flow.set_summary_chunks(&summaries).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagBuildAnswerContextTask {
    state: AppState,
}

#[async_trait]
impl Task for RagBuildAnswerContextTask {
    fn id(&self) -> &str {
        TASK_RAG_BUILD_ANSWER_CONTEXT
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_BUILD_ANSWER_CONTEXT,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let rag_runtime = require_rag_runtime(&self.state)?;
        let summary_chunks = flow.summary_chunks().await.unwrap_or_default();
        let retrieved_chunks = flow.retrieved_chunks().await.unwrap_or_default();
        let context_chunks =
            rag_runtime.build_answer_context_chunks(&summary_chunks, &retrieved_chunks);
        let retrieval_context_count = context_chunks
            .iter()
            .filter(|chunk| chunk.chunk_type != "summary")
            .count();
        let summary_context_count = context_chunks.len().saturating_sub(retrieval_context_count);
        info!(
            node = TASK_RAG_BUILD_ANSWER_CONTEXT,
            retrieved_chunk_count = retrieved_chunks.len(),
            summary_chunk_count = summary_chunks.len(),
            answer_context_count = context_chunks.len(),
            retrieval_context_count,
            summary_context_count,
            "rag answer context assembled"
        );
        flow.set_answer_context(&context_chunks).await;
        Ok(TaskResult::move_to_next())
    }
}

fn rag_plan_summary_mode(plan: &common::RagPlan) -> &'static str {
    if plan
        .items
        .iter()
        .any(|item| item.summary.as_deref() == Some("all"))
    {
        "all"
    } else if plan
        .items
        .iter()
        .any(|item| item.summary.as_deref() == Some("related"))
    {
        "related"
    } else {
        "none"
    }
}

fn rag_plan_items_for_log(plan: &common::RagPlan) -> Vec<String> {
    plan.items
        .iter()
        .map(|item| {
            if let Some(query) = item
                .query
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                format!("query:{:.2}:{}", item.priority, query)
            } else if let Some(terms) = item.bm25_terms.as_ref().filter(|terms| !terms.is_empty()) {
                format!("bm25:{:.2}:{}", item.priority, terms.join("|"))
            } else if let Some(summary) = item.summary.as_deref() {
                format!("summary:{:.2}:{}", item.priority, summary)
            } else {
                format!("empty:{:.2}", item.priority)
            }
        })
        .collect()
}

fn item_trace_for_log(trace: &[common::RagTraceItem]) -> Vec<String> {
    trace
        .iter()
        .map(|item| {
            format!(
                "{}:priority={:.2}:dense_k={}:bm25_k={}:summary={}",
                item.payload_kind,
                item.priority,
                item.dense_k,
                item.bm25_k,
                item.summary.as_deref().unwrap_or("none")
            )
        })
        .collect()
}

fn scored_chunks_for_log(chunks: &[avrag_rag_core::retrieval::ScoredChunk]) -> Vec<String> {
    chunks
        .iter()
        .take(6)
        .map(|chunk| {
            format!(
                "{}:{}:{:.3}:page={}:type={}",
                chunk.doc_id,
                chunk.chunk_id,
                chunk.score,
                chunk.page.unwrap_or_default(),
                chunk.chunk_type
            )
        })
        .collect()
}

#[derive(Clone)]
struct RagAnswerSynthesizeTask {
    state: AppState,
}

#[async_trait]
impl Task for RagAnswerSynthesizeTask {
    fn id(&self) -> &str {
        TASK_RAG_ANSWER_SYNTHESIZE
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_ANSWER_SYNTHESIZE,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let execute_request = flow.rag_execute_request().await?;
        let execute_response = flow.rag_execute_response().await?;
        let mut degrade_trace = flow.degrade_trace().await.unwrap_or_default();
        let session = flow.session().await?;
        let answer_output = self
            .state
            .answer_rag_with_main_agent(
                &request,
                Some(&session),
                &execute_request,
                &execute_response,
                &mut degrade_trace,
            )
            .await;

        let rag_llm_usage = answer_output.llm_usage.clone();
        let response = crate::main_agent::MainAgent::build_rag_chat_response(
            &request,
            Some(&session.id),
            &execute_request,
            &execute_response,
            answer_output,
            degrade_trace,
        );

        flow.set_execution(&ChatGraphExecution {
            mode: "rag".to_string(),
            input_usage_text: request.query.trim().to_string(),
            apply_output_guard: true,
            response,
            llm_usage: rag_llm_usage,
            debug_metadata: Some(serde_json::json!({
                "rag_tool": {
                    "execute_plan_request": &execute_request,
                    "execute_plan_coverage": &execute_response.coverage,
                    "execute_plan_backend_trace": &execute_response.backend_trace,
                }
            })),
        })
        .await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RagValidateCitationsTask {
    state: AppState,
}

#[async_trait]
impl Task for RagValidateCitationsTask {
    fn id(&self) -> &str {
        TASK_RAG_VALIDATE_CITATIONS
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_VALIDATE_CITATIONS,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let mut execution = flow.execution().await?;
        let valid_chunk_ids = flow
            .rag_execute_response()
            .await?
            .bundle
            .chunks
            .into_iter()
            .map(|chunk| chunk.chunk_id.to_string())
            .collect::<std::collections::HashSet<_>>();
        let before = execution.response.citations.len();
        execution.response.citations.retain(|citation| {
            citation
                .chunk_id
                .as_ref()
                .is_none_or(|chunk_id| valid_chunk_ids.contains(chunk_id))
        });
        if execution.response.citations.len() != before {
            execution
                .response
                .degrade_trace
                .push(common::DegradeTraceItem {
                    stage: "rag_validate_citations".to_string(),
                    reason: "removed dangling citations not present in final candidate set"
                        .to_string(),
                    impact: "Dropped invalid citations before output guard".to_string(),
                });
        }
        let _ = &self.state;
        flow.set_execution(&execution).await;
        Ok(TaskResult::move_to_next())
    }
}
