#[derive(Clone)]
struct PreflightTask {
    state: AppState,
}

#[async_trait]
impl Task for PreflightTask {
    fn id(&self) -> &str {
        TASK_PREFLIGHT
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_PREFLIGHT, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let preflight = self
            .state
            .execute_chat_preflight(&request)
            .await
            .map_err(graph_app_error)?;
        flow.set_preflight(&preflight).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct SessionTask {
    state: AppState,
}

#[async_trait]
impl Task for SessionTask {
    fn id(&self) -> &str {
        TASK_SESSION
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_SESSION, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = self
            .state
            .resolve_chat_session(&request)
            .await
            .map_err(graph_app_error)?;
        flow.set_session(&session).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct ModeSelectTask {
    state: AppState,
}

#[async_trait]
impl Task for ModeSelectTask {
    fn id(&self) -> &str {
        TASK_MODE_SELECT
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_MODE_SELECT, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let decision = crate::main_agent::MainAgent::decide(&request);
        if let crate::main_agent::MainAgentDecision::Clarify { message } = &decision {
            let session = flow.session().await?;
            let execution = self
                .state
                .execute_clarify_mode_core(&request, &session, message)
                .await
                .map_err(graph_app_error)?;
            flow.set_execution(&execution).await;
            return Ok(TaskResult::new(
                None,
                NextAction::GoTo(TASK_OUTPUT_GUARD.to_string()),
            ));
        }
        if self.state.pg().is_none() {
            return Ok(TaskResult::new(
                None,
                NextAction::GoTo(TASK_MEMORY_MODE.to_string()),
            ));
        }
        match decision {
            crate::main_agent::MainAgentDecision::Clarify { .. } => unreachable!(),
            crate::main_agent::MainAgentDecision::DirectChat => Ok(TaskResult::new(
                None,
                NextAction::GoTo(TASK_GENERAL.to_string()),
            )),
            crate::main_agent::MainAgentDecision::ExternalSearch => Ok(TaskResult::new(
                None,
                NextAction::GoTo(TASK_SEARCH.to_string()),
            )),
            crate::main_agent::MainAgentDecision::ExecutePlan => Ok(TaskResult::new(
                None,
                NextAction::GoTo(TASK_RAG_PREPARE_PLANNER_INPUT.to_string()),
            )),
        }
    }
}

#[derive(Clone)]
struct MemoryModeTask {
    state: AppState,
}

#[async_trait]
impl Task for MemoryModeTask {
    fn id(&self) -> &str {
        TASK_MEMORY_MODE
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_MEMORY_MODE, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = flow.session().await?;
        let execution = self
            .state
            .execute_memory_chat_compat(&request, &session)
            .await
            .map_err(graph_app_error)?;
        flow.set_execution(&execution).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct GeneralModeTask {
    state: AppState,
}

#[async_trait]
impl Task for GeneralModeTask {
    fn id(&self) -> &str {
        TASK_GENERAL
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_GENERAL, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = flow.session().await?;
        let pg = self.state.pg().ok_or_else(|| {
            graph_app_error(AppError::internal("postgres backend is not configured"))
        })?;
        let execution = self
            .state
            .execute_general_mode_core(&request, &session, pg.as_ref())
            .await
            .map_err(graph_app_error)?;
        flow.set_execution(&execution).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct SearchModeTask {
    state: AppState,
}

#[async_trait]
impl Task for SearchModeTask {
    fn id(&self) -> &str {
        TASK_SEARCH
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_SEARCH, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = flow.session().await?;
        let execution = self
            .state
            .execute_search_mode_core(&request, &session)
            .await
            .map_err(graph_app_error)?;
        flow.set_execution(&execution).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct OutputGuardTask {
    state: AppState,
}

#[async_trait]
impl Task for OutputGuardTask {
    fn id(&self) -> &str {
        TASK_OUTPUT_GUARD
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_OUTPUT_GUARD, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let session = flow.session().await?;
        let preflight = flow.preflight().await?;
        let mut execution = flow.execution().await?;
        self.state
            .apply_output_guard_to_execution(
                &session,
                &mut execution,
                &preflight.trace_id,
                preflight.user_uuid,
                self.state.pg().as_deref(),
            )
            .await
            .map_err(graph_app_error)?;
        flow.set_execution(&execution).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct PersistTask {
    state: AppState,
}

#[async_trait]
impl Task for PersistTask {
    fn id(&self) -> &str {
        TASK_PERSIST
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_PERSIST, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        let session = flow.session().await?;
        let mut execution = flow.execution().await?;
        if request.source_type.as_deref() != Some("share")
            && let Some(pg) = self.state.pg()
        {
            self.state
                .persist_chat_execution(&request, &session, &mut execution, pg.as_ref())
                .await
                .map_err(graph_app_error)?;
        }
        flow.set_execution(&execution).await;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct RecordUsageTask {
    state: AppState,
}

#[async_trait]
impl Task for RecordUsageTask {
    fn id(&self) -> &str {
        TASK_USAGE
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_USAGE, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let execution = flow.execution().await?;
        self.state
            .record_usage_for_execution(&execution)
            .await
            .map_err(graph_app_error)?;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct EmitNotificationsTask {
    state: AppState,
}

#[async_trait]
impl Task for EmitNotificationsTask {
    fn id(&self) -> &str {
        TASK_NOTIFY
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_NOTIFY, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let request = flow.request().await?;
        if request.source_type.as_deref() == Some("share") {
            return Ok(TaskResult::move_to_next());
        }
        let session = flow.session().await?;
        let execution = flow.execution().await?;
        self.state
            .emit_notifications_for_execution(&session, &execution)
            .await
            .map_err(graph_app_error)?;
        Ok(TaskResult::move_to_next())
    }
}

#[derive(Clone)]
struct BuildResponseTask;

#[async_trait]
impl Task for BuildResponseTask {
    fn id(&self) -> &str {
        TASK_BUILD_RESPONSE
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!(node = TASK_BUILD_RESPONSE, "graphflow chat node start");
        let flow = ChatFlowContext::from(context);
        let execution = flow.execution().await?;
        let answer = execution.response.answer.clone();
        flow.set_response(&execution.response).await;
        Ok(TaskResult::new(Some(answer), NextAction::End))
    }
}
