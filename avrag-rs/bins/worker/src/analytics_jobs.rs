use analytics::detect_request_burst;
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use sqlx::{PgConnection, PgPool, Row};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tracing::info;
use uuid::Uuid;

const ANALYTICS_ROLLUP_LOCK_KEY: i64 = 60190019;

pub struct AnalyticsJobRunner {
    pool: PgPool,
    interval: Duration,
    last_run_at: Option<Instant>,
}

impl AnalyticsJobRunner {
    pub fn from_env(pool: PgPool) -> Option<Self> {
        if !env_bool("ANALYTICS_ROLLUP_ENABLED", false) {
            return None;
        }

        let interval_secs = std::env::var("ANALYTICS_ROLLUP_INTERVAL_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(300);

        Some(Self {
            pool,
            interval: Duration::from_secs(interval_secs),
            last_run_at: None,
        })
    }

    pub async fn maybe_run(&mut self) -> Result<()> {
        let now = Instant::now();
        if let Some(last_run_at) = self.last_run_at
            && now.duration_since(last_run_at) < self.interval
        {
            return Ok(());
        }
        // One transaction for lock + work: the session-scoped advisory lock
        // previously leaked whenever the pool handed lock/unlock to different
        // connections, silently disabling every later run. xact lock
        // self-releases at commit/rollback.
        let mut tx = self.pool.begin().await?;
        let acquired = sqlx::query_scalar::<_, bool>("select pg_try_advisory_xact_lock($1)")
            .bind(ANALYTICS_ROLLUP_LOCK_KEY)
            .fetch_one(&mut *tx)
            .await?;
        if !acquired {
            tx.rollback().await?;
            return Ok(());
        }
        // Rollups read FORCE-RLS tables (workspaces, share_access_logs,
        // documents); the migration-established escape hatch grants full reads.
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await?;
        self.last_run_at = Some(now);

        let target_date = Utc::now().date_naive();
        let result = async {
            run_daily_jobs(&mut tx, target_date).await?;
            if let Some(previous_date) = target_date.pred_opt() {
                run_daily_jobs(&mut tx, previous_date).await?;
            }
            detect_recent_request_bursts(&mut tx).await?;
            detect_failed_chat_loops(&mut tx, target_date).await?;

            info!(target_date = %target_date, "analytics rollup jobs completed");
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                tx.commit().await?;
                Ok(())
            }
            Err(error) => {
                tx.rollback().await?;
                Err(error)
            }
        }
    }
}

async fn run_daily_jobs(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    rollup_product_events(conn, target_date).await?;
    rollup_public_share_views(conn, target_date).await?;
    record_storage_snapshots(conn, target_date).await?;
    rollup_cost_events(conn, target_date).await?;
    derive_daily_product_metrics(conn, target_date).await?;
    Ok(())
}

async fn rollup_product_events(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    sqlx::query(
        r#"
        insert into daily_user_metrics (
            event_date,
            user_id,
            is_dau,
            is_new_user,
            is_first_answer,
            is_cited_activation,
            chat_count,
            search_count,
            upload_count,
            shared_kb_open_count
        )
        select
            $1::date as event_date,
            pe.user_id,
            true as is_dau,
            bool_or(pe.event_name = 'user_registered') as is_new_user,
            bool_or(pe.event_name = 'chat_completed')
                and not exists (
                    select 1
                    from product_events earlier
                    where earlier.user_id = pe.user_id
                      and earlier.event_name = 'chat_completed'
                      and earlier.event_date < $1
                ) as is_first_answer,
            bool_or(
                pe.event_name = 'chat_completed'
                    and coalesce((pe.metadata->>'citation_count')::int, 0) > 0
            )
                and not exists (
                    select 1
                    from product_events earlier
                    where earlier.user_id = pe.user_id
                      and earlier.event_name = 'chat_completed'
                      and coalesce((earlier.metadata->>'citation_count')::int, 0) > 0
                      and earlier.event_date < $1
                ) as is_cited_activation,
            count(*) filter (where pe.event_name = 'chat_completed')::bigint as chat_count,
            count(*) filter (where pe.event_name = 'search_completed')::bigint as search_count,
            count(*) filter (where pe.event_name in ('document_upload_completed', 'url_source_added'))::bigint as upload_count,
            0::bigint as shared_kb_open_count
        from product_events pe
        where pe.event_date = $1
        group by pe.user_id
        on conflict (event_date, user_id) do update
        set is_dau = excluded.is_dau,
            is_new_user = excluded.is_new_user,
            is_first_answer = excluded.is_first_answer,
            is_cited_activation = excluded.is_cited_activation,
            chat_count = excluded.chat_count,
            search_count = excluded.search_count,
            upload_count = excluded.upload_count,
            shared_kb_open_count = excluded.shared_kb_open_count
        "#,
    )
    .bind(target_date)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn rollup_public_share_views(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    sqlx::query(
        r#"
        insert into daily_user_metrics (
            event_date,
            user_id,
            shared_kb_open_count
        )
        select
            $1::date as event_date,
            n.owner_id as user_id,
            count(*)::bigint as shared_kb_open_count
        from share_access_logs sal
        join workspaces n on n.id = sal.workspace_id
        where date(sal.created_at) = $1
          and sal.action = 'view'
          and n.owner_id is not null
        group by n.owner_id
        on conflict (event_date, user_id) do update
        set shared_kb_open_count = excluded.shared_kb_open_count
        "#,
    )
    .bind(target_date)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn rollup_cost_events(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    sqlx::query(
        r#"
        insert into daily_user_metrics (
            event_date,
            user_id,
            llm_prompt_tokens,
            llm_completion_tokens,
            embedding_tokens,
            storage_bytes,
            usage_units,
            estimated_cost_cents
        )
        select
            $1::date as event_date,
            ce.user_id,
            coalesce(sum(ce.prompt_tokens), 0)::bigint as llm_prompt_tokens,
            coalesce(sum(ce.completion_tokens), 0)::bigint as llm_completion_tokens,
            coalesce(sum(ce.embedding_tokens), 0)::bigint as embedding_tokens,
            coalesce(max(case when ce.event_name = 'storage_snapshot_recorded' then greatest(ce.storage_bytes_delta, 0) else 0 end), 0)::bigint as storage_bytes,
            coalesce(sum(ce.usage_units), 0)::bigint as usage_units,
            coalesce(sum(ce.usage_units), 0)::bigint as estimated_cost_cents
        from cost_events ce
        where ce.event_date = $1
        group by ce.user_id
        on conflict (event_date, user_id) do update
        set llm_prompt_tokens = excluded.llm_prompt_tokens,
            llm_completion_tokens = excluded.llm_completion_tokens,
            embedding_tokens = excluded.embedding_tokens,
            storage_bytes = excluded.storage_bytes,
            usage_units = excluded.usage_units,
            estimated_cost_cents = excluded.estimated_cost_cents
        "#,
    )
    .bind(target_date)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn derive_daily_product_metrics(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    sqlx::query(
        r#"
        insert into daily_product_metrics (
            event_date,
            dau,
            new_users,
            first_answer_users,
            cited_activation_users,
            daily_chat_users,
            daily_search_users,
            daily_upload_users,
            daily_shared_kb_users,
            total_llm_prompt_tokens,
            total_llm_completion_tokens,
            total_embedding_tokens,
            total_upload_bytes,
            total_estimated_cost_cents,
            cost_per_dau_cents,
            cost_per_cited_activation_cents
        )
        select
            $1::date as event_date,
            count(*) filter (where dum.is_dau)::bigint as dau,
            count(*) filter (where dum.is_new_user)::bigint as new_users,
            count(*) filter (where dum.is_first_answer)::bigint as first_answer_users,
            count(*) filter (where dum.is_cited_activation)::bigint as cited_activation_users,
            count(*) filter (where dum.chat_count > 0)::bigint as daily_chat_users,
            count(*) filter (where dum.search_count > 0)::bigint as daily_search_users,
            count(*) filter (where dum.upload_count > 0)::bigint as daily_upload_users,
            count(*) filter (where dum.shared_kb_open_count > 0)::bigint as daily_shared_kb_users,
            coalesce(sum(dum.llm_prompt_tokens), 0)::bigint as total_llm_prompt_tokens,
            coalesce(sum(dum.llm_completion_tokens), 0)::bigint as total_llm_completion_tokens,
            coalesce(sum(dum.embedding_tokens), 0)::bigint as total_embedding_tokens,
            (
                select coalesce(sum(greatest(ce.storage_bytes_delta, 0)), 0)::bigint
                from cost_events ce
                where ce.event_date = $1
                  and ce.event_name = 'upload_bytes_metered'
            ) as total_upload_bytes,
            coalesce(sum(dum.estimated_cost_cents), 0)::bigint as total_estimated_cost_cents,
            case
                when count(*) filter (where dum.is_dau) = 0 then 0
                else (coalesce(sum(dum.estimated_cost_cents), 0) / (count(*) filter (where dum.is_dau)))::bigint
            end as cost_per_dau_cents,
            case
                when count(*) filter (where dum.is_cited_activation) = 0 then 0
                else (coalesce(sum(dum.estimated_cost_cents), 0) / (count(*) filter (where dum.is_cited_activation)))::bigint
            end as cost_per_cited_activation_cents
        from daily_user_metrics dum
        where dum.event_date = $1
        on conflict (event_date) do update
        set dau = excluded.dau,
            new_users = excluded.new_users,
            first_answer_users = excluded.first_answer_users,
            cited_activation_users = excluded.cited_activation_users,
            daily_chat_users = excluded.daily_chat_users,
            daily_search_users = excluded.daily_search_users,
            daily_upload_users = excluded.daily_upload_users,
            daily_shared_kb_users = excluded.daily_shared_kb_users,
            total_llm_prompt_tokens = excluded.total_llm_prompt_tokens,
            total_llm_completion_tokens = excluded.total_llm_completion_tokens,
            total_embedding_tokens = excluded.total_embedding_tokens,
            total_upload_bytes = excluded.total_upload_bytes,
            total_estimated_cost_cents = excluded.total_estimated_cost_cents,
            cost_per_dau_cents = excluded.cost_per_dau_cents,
            cost_per_cited_activation_cents = excluded.cost_per_cited_activation_cents
        "#,
    )
    .bind(target_date)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn record_storage_snapshots(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    if target_date != Utc::now().date_naive() {
        return Ok(());
    }

    sqlx::query(
        r#"
        insert into cost_events (
            event_id,
            event_time,
            event_date,
            user_id,
            session_id,
            workspace_id,
            event_name,
            feature,
            provider,
            model,
            prompt_tokens,
            completion_tokens,
            embedding_tokens,
            usage_units,
            storage_bytes_delta,
            external_call_count,
            source,
            metadata
        )
        select
            gen_random_uuid(),
            now(),
            $1::date,
            d.owner_user_id::text::uuid,
            null,
            null,
            'storage_snapshot_recorded',
            'storage',
            'internal',
            'storage_snapshot',
            0,
            0,
            0,
            0,
            coalesce(sum(d.file_size), 0)::bigint,
            0,
            'daily_snapshot',
            jsonb_build_object(
                'snapshot_date', $1::text,
                'document_count', count(*)::bigint
            )
        from documents d
        where d.owner_user_id is not null
        group by d.owner_user_id
        having not exists (
            select 1
            from cost_events ce
            where ce.event_date = $1
              and ce.event_name = 'storage_snapshot_recorded'
              and ce.source = 'daily_snapshot'
              and ce.user_id = d.owner_user_id
        )
        "#,
    )
    .bind(target_date)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn detect_recent_request_bursts(conn: &mut PgConnection) -> Result<()> {
    let rows = sqlx::query(
        r#"
        select user_id, event_name, extract(epoch from event_time)::bigint as ts
        from product_events
        where event_time >= now() - interval '5 minutes'
        order by user_id, event_name, event_time
        "#,
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut groups: BTreeMap<(Uuid, String), Vec<i64>> = BTreeMap::new();
    for row in rows {
        let user_id = row.try_get::<Uuid, _>("user_id")?;
        let event_name = row.try_get::<String, _>("event_name")?;
        let ts = row.try_get::<i64, _>("ts")?;
        groups.entry((user_id, event_name)).or_default().push(ts);
    }

    for ((user_id, event_name), timestamps) in groups {
        if detect_request_burst(&timestamps, 20, 60).is_some() {
            insert_anomaly_if_missing(
                &mut *conn,
                user_id,
                "request_burst",
                "medium",
                format!("request_burst:{user_id}:{event_name}"),
                serde_json::json!({
                    "event_name": event_name,
                    "window_sec": 60,
                    "threshold": 20,
                    "sample_size": timestamps.len(),
                }),
            )
            .await?;
        }
    }

    Ok(())
}

async fn detect_failed_chat_loops(conn: &mut PgConnection, target_date: NaiveDate) -> Result<()> {
    let rows = sqlx::query(
        r#"
        select user_id, count(*)::bigint as failure_count
        from product_events
        where event_date = $1
          and event_name = 'chat_failed'
        group by user_id
        having count(*) >= 5
        "#,
    )
    .bind(target_date)
    .fetch_all(&mut *conn)
    .await?;

    for row in rows {
        let user_id = row.try_get::<Uuid, _>("user_id")?;
        let failure_count = row.try_get::<i64, _>("failure_count")?;
        insert_anomaly_if_missing(
            &mut *conn,
            user_id,
            "failed_chat_loop",
            "high",
            format!("failed_chat_loop:{user_id}:{target_date}"),
            serde_json::json!({
                "event_name": "chat_failed",
                "failure_count": failure_count,
                "event_date": target_date,
            }),
        )
        .await?;
    }

    Ok(())
}

async fn insert_anomaly_if_missing(
    conn: &mut PgConnection,
    user_id: Uuid,
    anomaly_kind: &str,
    severity: &str,
    signature: String,
    metadata: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into user_anomalies (
            anomaly_id, detected_at, user_id, anomaly_kind, severity, signature, metadata
        )
        values ($1, now(), $2, $3, $4, $5, $6)
        on conflict (signature) do nothing
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(anomaly_kind)
    .bind(severity)
    .bind(signature)
    .bind(metadata)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}
