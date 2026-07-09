use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;

use crate::events::{CostEvent, ProductEvent};

#[derive(Clone)]
pub struct AnalyticsService {
    pool: PgPool,
}

impl AnalyticsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn record_product_event(&self, event: &ProductEvent) -> Result<()> {
        sqlx::query(
            r#"
            insert into product_events (
                event_id, event_time, event_date, user_id, session_id, workspace_id,
                surface, event_name, result, request_id, trace_id, client_platform, metadata
            ) values ($1, $2, date($2), $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(event.event_id)
        .bind(event.event_time)
        .bind(event.user_id)
        .bind(event.session_id)
        .bind(event.workspace_id)
        .bind(enum_text(&event.surface)?)
        .bind(enum_text(&event.event_name)?)
        .bind(enum_text(&event.result)?)
        .bind(&event.request_id)
        .bind(&event.trace_id)
        .bind(&event.client_platform)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_cost_event(&self, event: &CostEvent) -> Result<()> {
        sqlx::query(
            r#"
            insert into cost_events (
                event_id, event_time, event_date, user_id, session_id, workspace_id,
                event_name, feature, provider, model, prompt_tokens, completion_tokens,
                embedding_tokens, usage_units, storage_bytes_delta, external_call_count,
                source, metadata
            ) values ($1, $2, date($2), $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
        )
        .bind(event.event_id)
        .bind(event.event_time)
        .bind(event.user_id)
        .bind(event.session_id)
        .bind(event.workspace_id)
        .bind(enum_text(&event.event_name)?)
        .bind(&event.feature)
        .bind(&event.provider)
        .bind(&event.model)
        .bind(event.prompt_tokens)
        .bind(event.completion_tokens)
        .bind(event.embedding_tokens)
        .bind(event.usage_units)
        .bind(event.storage_bytes_delta)
        .bind(event.external_call_count)
        .bind(&event.source)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn enum_text<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?.trim_matches('"').to_string())
}
