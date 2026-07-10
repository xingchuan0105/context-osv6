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

fn map_subscription(row: sqlx::postgres::PgRow) -> Result<Subscription> {
    let created_at: Option<DateTime<Utc>> = row.try_get("created_at").ok();
    let updated_at: Option<DateTime<Utc>> = row.try_get("updated_at").ok();
    let period_start: Option<DateTime<Utc>> = row.try_get("current_period_start").ok();
    let period_end: Option<DateTime<Utc>> = row.try_get("current_period_end").ok();

    let billing_provider_str: String = row.try_get("billing_provider")?;
    let billing_provider: BillingProvider = billing_provider_str.parse()?;

    let status_str: String = row.try_get("status")?;
    let status: SubscriptionStatus = status_str.parse()?;

    Ok(Subscription {
        id: row.try_get::<Uuid, _>("id")?.to_string(),
        user_id: row.try_get::<Uuid, _>("user_id")?.to_string(),
        stripe_subscription_id: row.try_get("stripe_subscription_id").ok(),
        stripe_price_id: row.try_get("stripe_price_id").ok(),
        billing_provider,
        provider_subscription_id: row.try_get("provider_subscription_id").ok(),
        provider_price_id: row.try_get("provider_price_id").ok(),
        plan_id: row.try_get("plan_id")?,
        status,
        current_period_start: period_start,
        current_period_end: period_end,
        cancel_at_period_end: row.try_get("cancel_at_period_end")?,
        created_at,
        updated_at,
    })
}

fn month_start() -> DateTime<Utc> {
    let now = Utc::now();
    Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .expect("valid month start")
}

#[allow(dead_code)]
pub(super) fn seconds_until_next_month() -> u64 {
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

/// Personal B2C: account owner is the user id itself (no org table).
async fn owner_user_id_for_user(
    _repo: Arc<PgAppRepository>,
    user_id: Uuid,
) -> Result<Uuid> {
    Ok(user_id)
}

#[allow(dead_code)]
async fn get_stripe_customer_id_by_user_id(
    repo: Arc<PgAppRepository>,
    user_id: Uuid,
) -> Result<Option<String>> {
    if user_id.is_nil() {
        return Ok(None);
    }
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    let row = sqlx::query("select stripe_customer_id from users where id = $1")
        .bind(user_id)
        .fetch_optional(tx.as_mut())
        .await?;
    tx.commit().await?;
    Ok(row.and_then(|r| r.try_get::<Option<String>, _>("stripe_customer_id").ok().flatten()))
}

