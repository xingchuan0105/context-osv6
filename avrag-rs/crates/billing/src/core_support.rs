fn string_or_nested_id(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(value) = value.as_str() {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    value
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn unix_timestamp_to_utc(timestamp: Option<i64>) -> Option<DateTime<Utc>> {
    timestamp.and_then(|value| Utc.timestamp_opt(value, 0).single())
}

fn map_stripe_status_to_local(status: &str) -> String {
    match status.trim() {
        "active" => STATUS_ACTIVE.to_string(),
        "canceled" | "cancelled" => STATUS_CANCELED.to_string(),
        "past_due" => STATUS_PAST_DUE.to_string(),
        "unpaid" => STATUS_UNPAID.to_string(),
        other => other.to_string(),
    }
}

async fn set_current_org(conn: &mut sqlx::PgConnection, org_id: &str) -> Result<()> {
    sqlx::query("select set_config('app.current_org', $1, true)")
        .bind(org_id)
        .execute(conn)
        .await?;
    Ok(())
}

async fn set_current_role(conn: &mut sqlx::PgConnection, role: &str) -> Result<()> {
    sqlx::query("select set_config('app.current_role', $1, true)")
        .bind(role)
        .execute(conn)
        .await?;
    Ok(())
}

fn map_subscription(row: sqlx::postgres::PgRow) -> Result<Subscription> {
    let created_at: Option<DateTime<Utc>> = row.try_get("created_at").ok();
    let updated_at: Option<DateTime<Utc>> = row.try_get("updated_at").ok();
    let period_start: Option<DateTime<Utc>> = row.try_get("current_period_start").ok();
    let period_end: Option<DateTime<Utc>> = row.try_get("current_period_end").ok();
    Ok(Subscription {
        id: row.try_get::<Uuid, _>("id")?.to_string(),
        org_id: row.try_get::<Uuid, _>("org_id")?.to_string(),
        stripe_subscription_id: row.try_get("stripe_subscription_id").ok(),
        stripe_price_id: row.try_get("stripe_price_id").ok(),
        plan_id: row.try_get("plan_id")?,
        status: row.try_get("status")?,
        current_period_start: period_start.map(|dt| dt.to_rfc3339()),
        current_period_end: period_end.map(|dt| dt.to_rfc3339()),
        cancel_at_period_end: row.try_get("cancel_at_period_end")?,
        created_at: created_at.map(|dt| dt.to_rfc3339()),
        updated_at: updated_at.map(|dt| dt.to_rfc3339()),
    })
}

fn month_start() -> DateTime<Utc> {
    let now = Utc::now();
    Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .expect("valid month start")
}

pub(crate) fn seconds_until_next_month() -> u64 {
    let now = Utc::now();
    let (year, month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let next = Utc
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .expect("valid next month start");
    next.signed_duration_since(now).num_seconds().max(1) as u64
}
