#[derive(Clone)]
struct RagLoadSessionContextTask {
    state: AppState,
}

#[async_trait]
impl Task for RagLoadSessionContextTask {
    fn id(&self) -> &str {
        TASK_RAG_LOAD_SESSION_CONTEXT
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(
            node = TASK_RAG_LOAD_SESSION_CONTEXT,
            "graphflow chat node start"
        );
        let flow = ChatFlowContext::from(context);
        let session = flow.session().await?;
        let session_context = self
            .state
            .build_session_context(&session)
            .await
            .map_err(graph_app_error)?;
        if let Some(session_context) = session_context {
            flow.set_rag_session_context(&session_context).await;
        }

        Ok(TaskResult::move_to_next())
    }
}

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
        let rag_runtime = require_rag_runtime(&self.state)?;
        let mut degrade_trace = flow.degrade_trace().await.unwrap_or_default();

        let (rag_plan, planner_usage) = rag_runtime
            .plan(
                &request,
                flow.rag_session_context().await.as_ref(),
                flow.docscope_metadata().await.as_ref(),
                &mut degrade_trace,
            )
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
        if let Some(usage) = planner_usage.as_ref() {
            self.state
                .record_llm_usage_if_available(
                    avrag_usage_limit::BillableFeature::Planner,
                    "rag_planner",
                    usage,
                    "graphflow",
                )
                .await;
        }

        flow.set_rag_plan(&rag_plan).await;
        flow.set_degrade_trace(&degrade_trace).await;

        if rag_plan.clarify_needed {
            return Ok(TaskResult::new(
                None,
                NextAction::GoTo(TASK_RAG_ANSWER_SYNTHESIZE.to_string()),
            ));
        }

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
        let request = flow.request().await?;
        let mut rag_plan = flow.rag_plan().await?;
        let rag_runtime = require_rag_runtime(&self.state)?;
        let item_trace = rag_runtime.normalize_plan(&request, &mut rag_plan);
        flow.set_rag_plan(&rag_plan).await;
        flow.set_item_trace(&item_trace).await;
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
        let summaries = rag_runtime
            .apply_summary_policy(
                &request,
                &self.state.auth,
                &rag_plan,
                &flow.retrieved_chunks().await.unwrap_or_default(),
            )
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;
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
        let context_chunks = rag_runtime.build_answer_context_chunks(
            &flow.summary_chunks().await.unwrap_or_default(),
            &flow.retrieved_chunks().await.unwrap_or_default(),
        );
        flow.set_answer_context(&context_chunks).await;
        Ok(TaskResult::move_to_next())
    }
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
        let rag_plan = flow.rag_plan().await?;
        let chunks = flow.retrieved_chunks().await.unwrap_or_default();
        let item_trace = flow.item_trace().await.unwrap_or_default();
        let mut degrade_trace = flow.degrade_trace().await.unwrap_or_default();
        let rag_runtime = require_rag_runtime(&self.state)?;

        let synthesis_output = if rag_plan.clarify_needed {
            avrag_llm::SynthesisOutput {
                answer_text: rag_plan.clarify_message.clone(),
                answer_blocks: common::plain_text_answer_blocks(&rag_plan.clarify_message),
                cited_chunk_ids: Vec::new(),
                llm_usage: None,
            }
        } else {
            rag_runtime
                .synthesize_answer_text(
                    &request,
                    flow.rag_session_context().await.as_ref(),
                    &rag_plan,
                    &item_trace,
                    &flow.answer_context().await.unwrap_or_default(),
                    &mut degrade_trace,
                )
                .await
        };

        let rag_llm_usage = synthesis_output.llm_usage.clone();
        let session = flow.session().await?;
        let response = rag_runtime
            .build_rag_chat_response(
                &request,
                Some(&session.id),
                &self.state.auth,
                &rag_plan,
                &chunks,
                &item_trace,
                flow.summary_chunks().await.unwrap_or_default().len(),
                synthesis_output,
                degrade_trace,
            )
            .await
            .map_err(|error| graph_app_error(crate::map_anyhow_error(error)))?;

        flow.set_execution(&ChatGraphExecution {
            mode: "rag".to_string(),
            input_usage_text: request.query.trim().to_string(),
            apply_output_guard: true,
            response,
            llm_usage: rag_llm_usage,
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
            .retrieved_chunks()
            .await
            .unwrap_or_default()
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
