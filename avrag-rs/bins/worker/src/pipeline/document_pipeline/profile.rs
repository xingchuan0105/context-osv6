use contracts::auth_runtime::AuthContext;
use ingestion::IngestionTask;
use tracing::info;
use uuid::Uuid;

use super::super::processor::PgTaskProcessor;
use super::super::windowed_llm::run_windowed_ps_and_triplets;
use super::ParseRunState;

/// Joint profile+summary+triplet windowed extraction (design 2026-08-06).
/// Writes TOC + summary; stores triplets on `parse_run_state` for the index stage.
pub(crate) async fn generate_document_summary(
    processor: &PgTaskProcessor,
    context: &AuthContext,
    task: &IngestionTask,
    document_id: Uuid,
    workspace_id: Uuid,
    filename: &str,
    title: &str,
    raw_text: &str,
    parse_run_state: &mut ParseRunState,
) {
    let user_uuid = task
        .requested_by
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    // ADR-0010: private indexing is gated by wallet at task start, not residual 5h/7d
    // plan unit walls. Do not skip windowed profile/summary LLM on rolling soft/hard.

    let result = run_windowed_ps_and_triplets(
        processor,
        context,
        task,
        document_id,
        workspace_id,
        filename,
        title,
        raw_text,
        parse_run_state,
    )
    .await;

    info!(
        document_id = %document_id,
        summary_chars = result.summary_text.len(),
        toc = result.toc_entries.len(),
        triplets = result.triplets.triplets.len(),
        prompt_tokens = result.prompt_tokens,
        completion_tokens = result.completion_tokens,
        total_tokens = result.triplets.total_tokens,
        "windowed profile+summary+triplet done"
    );

    if result.prompt_tokens == 0 && result.completion_tokens == 0 && result.triplets.total_tokens == 0
    {
        return;
    }

    let prompt_tokens = if result.prompt_tokens > 0 {
        result.prompt_tokens
    } else {
        // Fallback if provider omitted split: treat total as prompt-side.
        result.triplets.total_tokens
    };
    let completion_tokens = result.completion_tokens;
    let total_tokens = prompt_tokens.saturating_add(completion_tokens).max(result.triplets.total_tokens);

    let (provider, model) = processor
        .llm
        .ingestion_llm
        .as_ref()
        .map(|c| (c.config.provider_name(), c.config.model.clone()))
        .unwrap_or_else(|| ("unknown".into(), "unknown".into()));

    if let (Some(svc), Some(user_id)) = (&processor.metering.usage_limit, user_uuid) {
        let ctx = avrag_billing::usage_limit::MeteringContext {
            user_id,
            owner_user_id: context.user_id().into_uuid(),
            feature: avrag_billing::usage_limit::BillableFeature::Summary,
            stage: "worker_windowed_ingestion".to_string(),
            session_id: None,
            document_id: Some(document_id),
            request_id: None,
            trace_id: None,
        };
        if let Err(error) = svc
            .record_usage(
                &ctx,
                avrag_billing::usage_limit::UsageRecord {
                    provider: &provider,
                    model: &model,
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                    usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                },
            )
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to record windowed ingestion usage");
        }
    }

    if let (Some(analytics), Some(user_id)) = (&processor.metering.analytics, user_uuid) {
        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            workspace_id: None,
            event_name: analytics::CostEventName::SummaryUsageMetered,
            feature: "windowed_ingestion".to_string(),
            provider: if provider.trim().is_empty() {
                "unknown".to_string()
            } else {
                provider.clone()
            },
            model: if model.trim().is_empty() {
                "unknown".to_string()
            } else {
                model.clone()
            },
            prompt_tokens: i64::from(prompt_tokens),
            completion_tokens: i64::from(completion_tokens),
            embedding_tokens: 0,
            usage_units: avrag_billing::usage_limit::compute_usage_units(
                &provider,
                &model,
                prompt_tokens,
                completion_tokens,
            ),
            storage_bytes_delta: 0,
            external_call_count: 0,
            source: "worker".to_string(),
            metadata: serde_json::json!({
                "task_id": task.task_id,
                "document_id": document_id,
                "filename": filename,
                "stage": "worker_windowed_ingestion",
            }),
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            info!(document_id = %document_id, error = %error, "failed to record windowed ingestion analytics event");
        }
    }
}
