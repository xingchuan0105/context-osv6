pub(super) fn subscription_snapshot_from_event(
    payload: &serde_json::Value,
    config: &BillingConfig,
) -> Result<StripeSubscriptionSnapshot> {
    let subscription = payload
        .pointer("/data/object")
        .ok_or_else(|| anyhow!("subscription payload is required"))?;

    let stripe_subscription_id = subscription
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("subscription payload is missing id"))?
        .to_string();
    let stripe_customer_id = string_or_nested_id(subscription.get("customer")).unwrap_or_default();
    let stripe_price_id = subscription
        .pointer("/items/data/0/price/id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let mut user_id = subscription
        .pointer("/metadata/user_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let mut plan_id = subscription
        .pointer("/metadata/plan_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if plan_id.is_empty()
        && let Some(mapped_plan) = config.plan_id_by_price_id(&stripe_price_id) {
            plan_id = mapped_plan.to_string();
        }
    if user_id.is_empty() && stripe_customer_id.is_empty() {
        bail!("subscription metadata missing user_id and customer id");
    }
    if plan_id.is_empty() {
        bail!("subscription metadata missing plan_id");
    }

    Ok(StripeSubscriptionSnapshot {
        user_id: std::mem::take(&mut user_id),
        stripe_customer_id,
        stripe_subscription_id,
        stripe_price_id,
        plan_id,
        status: map_stripe_status_to_local(
            subscription
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or(STATUS_ACTIVE),
        ),
        current_period_start: unix_timestamp_to_utc(
            subscription
                .get("current_period_start")
                .and_then(|value| value.as_i64()),
        ),
        current_period_end: unix_timestamp_to_utc(
            subscription
                .get("current_period_end")
                .and_then(|value| value.as_i64()),
        ),
        cancel_at_period_end: subscription
            .get("cancel_at_period_end")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    })
}

fn invoice_subscription_id(payload: &serde_json::Value) -> Option<String> {
    let value = payload.pointer("/data/object/subscription")?;
    string_or_nested_id(Some(value))
}

async fn hydrate_subscription_snapshot(
    repo: Arc<PgAppRepository>,
    snapshot: &mut StripeSubscriptionSnapshot,
    config: &BillingConfig,
) -> Result<()> {
    if snapshot.user_id.is_empty() && !snapshot.stripe_customer_id.is_empty()
        && let Some(user_id) =
            find_user_id_by_customer_id(repo.clone(), &snapshot.stripe_customer_id).await?
        {
            snapshot.user_id = user_id;
        }

    if let Some(existing) =
        load_existing_subscription_fields(repo.clone(), &snapshot.stripe_subscription_id).await?
    {
        if snapshot.user_id.is_empty() {
            snapshot.user_id = existing.user_id;
        }
        if snapshot.stripe_price_id.is_empty() {
            if let Some(price_id) = existing.stripe_price_id {
                snapshot.stripe_price_id = price_id;
            }
        }
        if snapshot.plan_id.is_empty() {
            snapshot.plan_id = existing.plan_id;
        }
    }

    if snapshot.plan_id.is_empty() && !snapshot.stripe_price_id.is_empty()
        && let Some(mapped_plan) = config.plan_id_by_price_id(&snapshot.stripe_price_id) {
            snapshot.plan_id = mapped_plan.to_string();
        }
    if snapshot.stripe_price_id.is_empty() && !snapshot.plan_id.is_empty()
        && let Some(price_id) = config
            .creem_checkout_product_for_plan(&snapshot.plan_id)
            .or_else(|| config.creem_checkout_price_for_plan(&snapshot.plan_id))
        {
            snapshot.stripe_price_id = price_id.to_string();
        }

    if snapshot.user_id.is_empty()
        || snapshot.stripe_subscription_id.is_empty()
        || snapshot.plan_id.is_empty()
        || snapshot.stripe_price_id.is_empty()
    {
        bail!("subscription snapshot incomplete after hydration");
    }
    Ok(())
}

async fn find_user_id_by_customer_id(
    repo: Arc<PgAppRepository>,
    customer_id: &str,
) -> Result<Option<String>> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    let row = sqlx::query("select id from users where stripe_customer_id = $1")
        .bind(customer_id)
        .fetch_optional(tx.as_mut())
        .await?;
    tx.commit().await?;
    Ok(row
        .map(|row| row.try_get::<Uuid, _>("id"))
        .transpose()?
        .map(|id| id.to_string()))
}

async fn load_existing_subscription_fields(
    repo: Arc<PgAppRepository>,
    stripe_subscription_id: &str,
) -> Result<Option<ExistingSubscriptionFields>> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    let row = sqlx::query(
        r#"
        select user_id, stripe_price_id, plan_id
        from subscriptions
        where stripe_subscription_id = $1
        limit 1
        "#,
    )
    .bind(stripe_subscription_id)
    .fetch_optional(tx.as_mut())
    .await?;
    tx.commit().await?;

    if let Some(row) = row {
        return Ok(Some(ExistingSubscriptionFields {
            user_id: row.try_get::<Uuid, _>("user_id")?.to_string(),
            stripe_price_id: row.try_get::<Option<String>, _>("stripe_price_id").ok().flatten(),
            plan_id: row.try_get::<String, _>("plan_id")?,
        }));
    }
    Ok(None)
}

async fn upsert_subscription_snapshot(
    repo: Arc<PgAppRepository>,
    snapshot: &StripeSubscriptionSnapshot,
) -> Result<()> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    if !snapshot.stripe_customer_id.is_empty() {
        sqlx::query("update users set stripe_customer_id = $2 where id = $1")
            .bind(Uuid::parse_str(&snapshot.user_id)?)
            .bind(&snapshot.stripe_customer_id)
            .execute(tx.as_mut())
            .await?;
    }
    sqlx::query(
        r#"
        insert into subscriptions (
            user_id,
            stripe_subscription_id,
            stripe_price_id,
            billing_provider,
            provider_subscription_id,
            provider_price_id,
            plan_id,
            status,
            current_period_start,
            current_period_end,
            cancel_at_period_end
        )
        values ($1, $2, $3, 'stripe', $2, $3, $4, $5, $6, $7, $8)
        on conflict (stripe_subscription_id) do update
        set user_id = excluded.user_id,
            stripe_price_id = excluded.stripe_price_id,
            billing_provider = excluded.billing_provider,
            provider_subscription_id = excluded.provider_subscription_id,
            provider_price_id = excluded.provider_price_id,
            plan_id = excluded.plan_id,
            status = excluded.status,
            current_period_start = excluded.current_period_start,
            current_period_end = excluded.current_period_end,
            cancel_at_period_end = excluded.cancel_at_period_end,
            updated_at = now()
        "#,
    )
    .bind(Uuid::parse_str(&snapshot.user_id)?)
    .bind(&snapshot.stripe_subscription_id)
    .bind(&snapshot.stripe_price_id)
    .bind(&snapshot.plan_id)
    .bind(&snapshot.status)
    .bind(snapshot.current_period_start)
    .bind(snapshot.current_period_end)
    .bind(snapshot.cancel_at_period_end)
    .execute(tx.as_mut())
    .await?;

    let sub_id = sqlx::query_scalar::<_, Uuid>(
        "select id from subscriptions where stripe_subscription_id = $1"
    )
    .bind(&snapshot.stripe_subscription_id)
    .fetch_one(tx.as_mut())
    .await?;

    let period_end_str = snapshot.current_period_end.map(|dt| dt.to_rfc3339()).unwrap_or_default();
    let dedupe_key = format!("{}:expired:{}", sub_id, period_end_str);
    sqlx::query(
        r#"
        insert into billing_outbox (event_type, payload, status, dedupe_key)
        values ($1, $2, 'pending', $3)
        on conflict (dedupe_key) do nothing
        "#,
    )
    .bind("subscription.updated")
    .bind(serde_json::json!({
        "subscription_id": sub_id.to_string(),
        "user_id": snapshot.user_id,
        "plan_id": snapshot.plan_id,
        "period_end": period_end_str,
    }))
    .bind(&dedupe_key)
    .execute(tx.as_mut())
    .await?;

    tx.commit().await?;
    Ok(())
}
