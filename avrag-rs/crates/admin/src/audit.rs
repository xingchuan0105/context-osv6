use crate::models::{AuditLogEntry, AuditLogQuery, OrgInfo, UserInfo};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Postgres, QueryBuilder, Row};
use uuid::Uuid;

pub(super) fn map_org_info(row: sqlx::postgres::PgRow) -> Result<OrgInfo> {
    let id: Uuid = row.try_get("id")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    Ok(OrgInfo {
        id: avrag_auth::OrgId::from(id),
        name: row.try_get("name")?,
        created_at: created_at.timestamp(),
        blocked: row.try_get("blocked")?,
        user_count: row.try_get("user_count")?,
        document_count: row.try_get("document_count")?,
        query_count: row.try_get("query_count")?,
    })
}

pub(super) fn map_user_info(row: sqlx::postgres::PgRow) -> Result<UserInfo> {
    let id: Uuid = row.try_get("id")?;
    let org_id: Uuid = row.try_get("org_id")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    Ok(UserInfo {
        id: common::UserId::from(id),
        email: row.try_get("email")?,
        org_id: avrag_auth::OrgId::from(org_id),
        role: row.try_get("role")?,
        created_at: created_at.timestamp(),
    })
}

pub(super) fn usage_period_start(period: &str) -> DateTime<Utc> {
    let days = match period {
        "7d" => 7,
        "90d" => 90,
        _ => 30,
    };
    Utc::now() - chrono::TimeDelta::days(days)
}

pub(super) fn clamp_audit_per_page(value: usize) -> usize {
    value.clamp(1, 200)
}

pub(super) fn audit_window_start(window: Option<&str>) -> Option<DateTime<Utc>> {
    let duration = match window {
        Some("24h") => Some(chrono::TimeDelta::hours(24)),
        Some("7d") => Some(chrono::TimeDelta::days(7)),
        Some("30d") => Some(chrono::TimeDelta::days(30)),
        Some("90d") => Some(chrono::TimeDelta::days(90)),
        _ => None,
    }?;
    Some(Utc::now() - duration)
}

pub(super) fn build_audit_log_base_query(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &AuditLogQuery,
    count_only: bool,
) {
    if count_only {
        builder.push("select count(*) as total from audit_log where 1 = 1");
    } else {
        builder.push(
            "select id, actor_id, action, resource_type, resource_id, org_id, created_at from audit_log where 1 = 1",
        );
    }

    if let Some(window_start) = audit_window_start(query.window.as_deref()) {
        builder.push(" and created_at >= ").push_bind(window_start);
    }
    if let Some(action) = query.action.as_deref() {
        builder
            .push(" and action = ")
            .push_bind(action.trim().to_string());
    }
    if let Some(resource_type) = query.resource_type.as_deref() {
        builder
            .push(" and resource_type = ")
            .push_bind(resource_type.trim().to_string());
    }
    if let Some(actor) = query.actor.as_deref() {
        builder
            .push(" and coalesce(actor_id::text, '') ilike ")
            .push_bind(format!("%{}%", actor.trim()));
    }
    if let Some(search) = query.query.as_deref() {
        let pattern = format!("%{}%", search.trim());
        builder.push(" and (action ilike ");
        builder.push_bind(pattern.clone());
        builder.push(" or resource_type ilike ");
        builder.push_bind(pattern.clone());
        builder.push(" or resource_id ilike ");
        builder.push_bind(pattern.clone());
        builder.push(" or coalesce(actor_id::text, '') ilike ");
        builder.push_bind(pattern);
        builder.push(")");
    }
}

pub(super) async fn audit_log_total(
    conn: &mut sqlx::PgConnection,
    query: &AuditLogQuery,
) -> Result<usize> {
    let mut builder = QueryBuilder::<Postgres>::new("");
    build_audit_log_base_query(&mut builder, query, true);
    let row = builder.build().fetch_one(conn).await?;
    Ok(row.try_get::<i64, _>("total")?.max(0) as usize)
}

pub(super) async fn audit_log_rows(
    conn: &mut sqlx::PgConnection,
    query: &AuditLogQuery,
) -> Result<Vec<sqlx::postgres::PgRow>> {
    let per_page = clamp_audit_per_page(query.per_page);
    let page = query.page.max(1);
    let offset = (page - 1) * per_page;
    let mut builder = QueryBuilder::<Postgres>::new("");
    build_audit_log_base_query(&mut builder, query, false);
    builder.push(" order by created_at desc, id desc limit ");
    builder.push_bind(per_page as i64);
    builder.push(" offset ");
    builder.push_bind(offset as i64);
    Ok(builder.build().fetch_all(conn).await?)
}

pub(super) fn map_audit_log_entry(row: sqlx::postgres::PgRow) -> Result<AuditLogEntry> {
    let actor_id = row.try_get::<Option<Uuid>, _>("actor_id")?;
    let org_id = row.try_get::<Option<Uuid>, _>("org_id")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    Ok(AuditLogEntry {
        id: row.try_get("id")?,
        actor_id: actor_id.map(|value| value.to_string()),
        action: row.try_get("action")?,
        resource_type: row.try_get("resource_type")?,
        resource_id: row.try_get("resource_id")?,
        org_id: org_id.map(|value| value.to_string()),
        created_at: created_at.timestamp(),
    })
}

pub(super) fn audit_logs_to_csv(items: &[AuditLogEntry]) -> String {
    let mut lines =
        vec!["id,action,resource_type,resource_id,actor_id,org_id,created_at".to_string()];
    for item in items {
        lines.push(format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            item.id,
            item.action.replace('"', "\"\""),
            item.resource_type.replace('"', "\"\""),
            item.resource_id.replace('"', "\"\""),
            item.actor_id
                .clone()
                .unwrap_or_default()
                .replace('"', "\"\""),
            item.org_id.clone().unwrap_or_default().replace('"', "\"\""),
            item.created_at
        ));
    }
    lines.join("\n")
}

pub(super) async fn set_current_org(conn: &mut sqlx::PgConnection, org_id: &str) -> Result<()> {
    sqlx::query("select set_config('app.current_org', $1, true)")
        .bind(org_id)
        .execute(conn)
        .await?;
    Ok(())
}
