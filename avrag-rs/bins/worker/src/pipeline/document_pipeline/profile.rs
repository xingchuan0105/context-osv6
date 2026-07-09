use contracts::auth_runtime::AuthContext;
use ingestion::IngestionTask;
use tracing::info;
use uuid::Uuid;

use super::super::processor::PgTaskProcessor;
use crate::ingestion_guard::ensure_ingestion_side_effects_allowed;

pub(crate) async fn generate_document_summary(
    processor: &PgTaskProcessor,
    context: &AuthContext,
    task: &IngestionTask,
    document_id: Uuid,
    filename: &str,
    content: &str,
    title: &str,
) {
    let Some(ref summary_gen) = processor.llm.summary_generator else {
        return;
    };
    let user_uuid = task
        .requested_by
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let mut skip_llm_summary = false;

    if let (Some(svc), Some(user_id)) = (&processor.metering.usage_limit, user_uuid) {
        match svc.check_quota(context.org_id().into_uuid(), user_id).await {
            Ok(quota) => {
                if quota.blocked_5h || quota.blocked_7d {
                    info!(document_id = %document_id, user_id = %user_id, "skipping LLM summary — quota exhausted");
                    skip_llm_summary = true;
                }
            }
            Err(error) => {
                info!(document_id = %document_id, error = %error, "quota check failed; skipping LLM summary (fail-closed)");
                skip_llm_summary = true;
            }
        }
    }

    if skip_llm_summary {
        return;
    }

    let generated_summary = summary_gen
        .synthesize(&document_id.to_string(), title, filename, content)
        .await;

    let Ok((summary, llm_usage)) = generated_summary else {
        info!(document_id = %document_id, "Summary generation failed, keeping naive fallback");
        return;
    };

    if ensure_ingestion_side_effects_allowed(
        &processor.storage.repo,
        context,
        task,
        document_id,
        "summary update",
    )
    .await
    .is_ok()
    {
        if let Err(error) = processor.storage.repo
            .documents()
            .update_document_summary(
                context,
                document_id,
                &summary,
                Some(&task.task_id),
                task.lock_token.as_deref(),
            )
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to update document summary");
        }
    }

    if let (Some(svc), Some(user_id)) = (&processor.metering.usage_limit, user_uuid) {
        let ctx = avrag_billing::usage_limit::MeteringContext {
            user_id,
            org_id: context.org_id().into_uuid(),
            feature: avrag_billing::usage_limit::BillableFeature::Summary,
            stage: "worker_summary".to_string(),
            session_id: None,
            document_id: Some(document_id),
            request_id: None,
            trace_id: None,
        };
        if let Err(error) = svc
            .record_usage(
                &ctx,
                avrag_billing::usage_limit::UsageRecord {
                    provider: &llm_usage.provider,
                    model: &llm_usage.model,
                    prompt_tokens: llm_usage.prompt_tokens,
                    completion_tokens: llm_usage.completion_tokens,
                    total_tokens: llm_usage.total_tokens,
                    usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                },
            )
            .await
        {
            info!(document_id = %document_id, error = %error, "failed to record summary usage");
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
            usage_units: avrag_billing::usage_limit::compute_usage_units(
                &llm_usage.provider,
                &llm_usage.model,
                llm_usage.prompt_tokens,
                llm_usage.completion_tokens,
            ),
            storage_bytes_delta: 0,
            external_call_count: 0,
            source: "worker".to_string(),
            metadata: serde_json::json!({
                "task_id": task.task_id,
                "document_id": document_id,
                "filename": filename,
            }),
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            info!(document_id = %document_id, error = %error, "failed to record summary analytics event");
        }
    }
}
