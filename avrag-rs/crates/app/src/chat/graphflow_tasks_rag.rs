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
        let request = flow.request().await?;
        let mut execute_request = flow.rag_execute_request().await?;
        execute_request.ensure_original_query_text_dense_item(request.query.trim());
        execute_request
            .validate()
            .map_err(|error| graph_app_error(AppError::validation("invalid_rag_plan", error.to_string())))?;
        let rag_plan = execute_request.to_rag_plan_compat();
        info!(
            node = TASK_RAG_NORMALIZE_PLAN,
            summary_mode = %rag_plan_summary_mode(&rag_plan),
            "main agent rag execute-plan request validated"
        );
        flow.set_rag_execute_request(&execute_request).await;
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
