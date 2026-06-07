
pub(crate) async fn process_webhook_event(
    repo: Arc<PgAppRepository>,
    provider: BillingProvider,
    payload: &serde_json::Value,
    config: &BillingConfig,
) -> Result<()> {
    match provider {
        BillingProvider::Stripe => {
            let event_type = payload
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");

            match event_type {
                "customer.subscription.created"
                | "customer.subscription.updated"
                | "customer.subscription.deleted" => {
                    let mut snapshot = subscription_snapshot_from_event(payload, config)?;
                    hydrate_subscription_snapshot(repo.clone(), &mut snapshot, config).await?;
                    if event_type == "customer.subscription.deleted" {
                        snapshot.status = STATUS_CANCELED.to_string();
                    }
                    upsert_subscription_snapshot(repo.clone(), &snapshot).await?;
                    let _ = emit_billing_notification(
                        repo.clone(),
                        &snapshot.user_id,
                        "billing.subscription.updated",
                        "Billing subscription updated",
                        "Your billing subscription status changed.",
                        serde_json::json!({
                            "plan_id": snapshot.plan_id,
                            "status": snapshot.status,
                            "stripe_subscription_id": snapshot.stripe_subscription_id,
                        }),
                    )
                    .await;
                }
                "invoice.payment_failed" => {
                    if let Some(subscription_id) = invoice_subscription_id(payload) {
                        update_subscription_status(repo.clone(), &subscription_id, STATUS_PAST_DUE).await?;
                        if let Some(existing) =
                            load_existing_subscription_fields(repo.clone(), &subscription_id).await?
                        {
                            let _ = emit_billing_notification(
                                repo.clone(),
                                &existing.user_id,
                                "billing.payment_failed",
                                "Billing payment failed",
                                "A subscription invoice payment failed and needs attention.",
                                serde_json::json!({
                                    "plan_id": existing.plan_id,
                                    "stripe_subscription_id": subscription_id,
                                    "status": STATUS_PAST_DUE,
                                }),
                            )
                            .await;
                        }
                    }
                }
                _ => {}
            }
        }
        BillingProvider::Creem => {
            let event_type = payload
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");

            match event_type {
                "subscription.paid" => {
                    let data = payload.get("data").ok_or_else(|| anyhow!("missing data field"))?;
                    let subscription_id = data.get("id").or_else(|| data.get("subscription_id")).and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing subscription id"))?.to_string();
                    let user_id = data.get("user_id").or_else(|| data.pointer("/metadata/user_id")).and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing user_id"))?.to_string();
                    let plan_id = data.get("plan_id").or_else(|| data.pointer("/metadata/plan_id")).and_then(|v| v.as_str()).unwrap_or("pro").to_string();
                    let price_id = data.get("price_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let amount_cents = data.get("amount").or_else(|| data.get("amount_cents")).and_then(|v| v.as_i64()).unwrap_or(2000) as i32;
                    let currency = data.get("currency").and_then(|v| v.as_str()).unwrap_or("usd").to_string();
                    
                    let current_period_start = data.get("current_period_start").and_then(|v| v.as_i64()).and_then(|ts| Utc.timestamp_opt(ts, 0).single());
                    let current_period_end = data.get("current_period_end").and_then(|v| v.as_i64()).and_then(|ts| Utc.timestamp_opt(ts, 0).single());

                    let mut tx = repo.raw().begin().await?;
                    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

                    sqlx::query(
                        r#"
                        insert into subscriptions (
                            user_id,
                            billing_provider,
                            provider_subscription_id,
                            provider_price_id,
                            plan_id,
                            status,
                            current_period_start,
                            current_period_end,
                            cancel_at_period_end
                        )
                        values ($1, 'creem', $2, $3, $4, 'active', $5, $6, false)
                        on conflict (billing_provider, provider_subscription_id) where provider_subscription_id is not null do update
                        set user_id = excluded.user_id,
                            provider_price_id = excluded.provider_price_id,
                            plan_id = excluded.plan_id,
                            status = excluded.status,
                            current_period_start = excluded.current_period_start,
                            current_period_end = excluded.current_period_end,
                            cancel_at_period_end = excluded.cancel_at_period_end,
                            updated_at = now()
                        "#,
                    )
                    .bind(Uuid::parse_str(&user_id)?)
                    .bind(&subscription_id)
                    .bind(&price_id)
                    .bind(&plan_id)
                    .bind(current_period_start)
                    .bind(current_period_end)
                    .execute(tx.as_mut())
                    .await?;

                    let sub_id = sqlx::query_scalar::<_, Uuid>(
                        "select id from subscriptions where billing_provider = 'creem' and provider_subscription_id = $1"
                    )
                    .bind(&subscription_id)
                    .fetch_one(tx.as_mut())
                    .await?;

                    let period_end_str = current_period_end.map(|dt| dt.to_rfc3339()).unwrap_or_default();
                    let dedupe_key = format!("{}:expired:{}", sub_id, period_end_str);
                    sqlx::query(
                        r#"
                        insert into billing_outbox (event_type, payload, status, dedupe_key)
                        values ($1, $2, 'pending', $3)
                        on conflict (dedupe_key) do nothing
                        "#,
                    )
                    .bind("subscription.paid")
                    .bind(serde_json::json!({
                        "subscription_id": sub_id.to_string(),
                        "user_id": user_id,
                        "plan_id": plan_id,
                        "period_end": period_end_str,
                    }))
                    .bind(&dedupe_key)
                    .execute(tx.as_mut())
                    .await?;

                    sqlx::query(
                        r#"
                        insert into billing_orders (user_id, provider, provider_order_id, plan_id, status, amount_cents, currency)
                        values ($1, 'creem', $2, $3, 'paid', $4, $5)
                        on conflict do nothing
                        "#,
                    )
                    .bind(Uuid::parse_str(&user_id)?)
                    .bind(&subscription_id)
                    .bind(&plan_id)
                    .bind(amount_cents)
                    .bind(&currency)
                    .execute(tx.as_mut())
                    .await?;

                    tx.commit().await?;

                    let _ = emit_billing_notification(
                        repo.clone(),
                        &user_id,
                        "billing.subscription.updated",
                        "Billing subscription updated",
                        "Your billing subscription status changed.",
                        serde_json::json!({
                            "plan_id": plan_id,
                            "status": "active",
                            "provider_subscription_id": subscription_id,
                        }),
                    )
                    .await;
                }
                "subscription.canceled" => {
                    let data = payload.get("data").ok_or_else(|| anyhow!("missing data field"))?;
                    let subscription_id = data.get("id").or_else(|| data.get("subscription_id")).and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing subscription id"))?.to_string();
                    
                    let mut tx = repo.raw().begin().await?;
                    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

                    sqlx::query(
                        r#"
                        update subscriptions
                        set status = 'canceled',
                            updated_at = now()
                        where billing_provider = 'creem' and provider_subscription_id = $1
                        "#,
                    )
                    .bind(&subscription_id)
                    .execute(tx.as_mut())
                    .await?;

                    let user_id = sqlx::query_scalar::<_, Uuid>(
                        "select user_id from subscriptions where billing_provider = 'creem' and provider_subscription_id = $1"
                    )
                    .bind(&subscription_id)
                    .fetch_optional(tx.as_mut())
                    .await?;

                    tx.commit().await?;

                    if let Some(uid) = user_id {
                        let _ = emit_billing_notification(
                            repo.clone(),
                            &uid.to_string(),
                            "billing.subscription.updated",
                            "Billing subscription updated",
                            "Your billing subscription status changed.",
                            serde_json::json!({
                                "status": "canceled",
                                "provider_subscription_id": subscription_id,
                            }),
                        )
                        .await;
                    }
                }
                _ => {}
            }
        }
        BillingProvider::Alipay => {
            let trade_status = payload.get("trade_status").and_then(|v| v.as_str()).unwrap_or("");
            if trade_status == "TRADE_SUCCESS" || trade_status == "TRADE_FINISHED" {
                let out_trade_no = payload.get("out_trade_no").and_then(|v| v.as_str()).unwrap_or("");
                if out_trade_no.is_empty() {
                    bail!("Alipay payload missing out_trade_no");
                }

                let mut tx = repo.raw().begin().await?;
                set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

                let row = sqlx::query("SELECT user_id, plan_id FROM billing_orders WHERE provider = 'alipay' AND provider_order_id = $1")
                    .bind(out_trade_no)
                    .fetch_one(tx.as_mut())
                    .await?;

                let user_id = row.try_get::<Uuid, _>("user_id")?;
                let plan_id = row.try_get::<String, _>("plan_id")?;

                sqlx::query(
                    r#"
                    update billing_orders
                    set status = 'paid',
                        updated_at = now()
                    where provider = 'alipay' and provider_order_id = $1
                    "#,
                )
                .bind(out_trade_no)
                .execute(tx.as_mut())
                .await?;

                sqlx::query(
                    r#"
                    insert into subscriptions (
                        user_id,
                        billing_provider,
                        provider_subscription_id,
                        plan_id,
                        status,
                        current_period_start,
                        current_period_end,
                        cancel_at_period_end
                    )
                    values ($1, 'alipay', $2, $3, 'active', now(), now() + interval '30 days', false)
                    on conflict (billing_provider, provider_subscription_id) where provider_subscription_id is not null do update
                    set plan_id = excluded.plan_id,
                        status = excluded.status,
                        current_period_start = excluded.current_period_start,
                        current_period_end = excluded.current_period_end,
                        cancel_at_period_end = excluded.cancel_at_period_end,
                        updated_at = now()
                    "#,
                )
                .bind(user_id)
                .bind(out_trade_no)
                .bind(&plan_id)
                .execute(tx.as_mut())
                .await?;

                let row_sub = sqlx::query(
                    "select id, current_period_end from subscriptions where billing_provider = 'alipay' and provider_subscription_id = $1"
                )
                .bind(out_trade_no)
                .fetch_one(tx.as_mut())
                .await?;
                let sub_id = row_sub.try_get::<Uuid, _>("id")?;
                let current_period_end = row_sub.try_get::<DateTime<Utc>, _>("current_period_end")?;
                let period_end_str = current_period_end.to_rfc3339();

                let dedupe_key = format!("{}:expired:{}", sub_id, period_end_str);
                sqlx::query(
                    r#"
                    insert into billing_outbox (event_type, payload, status, dedupe_key)
                    values ($1, $2, 'pending', $3)
                    on conflict (dedupe_key) do nothing
                    "#,
                )
                .bind("subscription.paid")
                .bind(serde_json::json!({
                    "subscription_id": sub_id.to_string(),
                    "user_id": user_id.to_string(),
                    "plan_id": plan_id,
                    "period_end": period_end_str,
                }))
                .bind(&dedupe_key)
                .execute(tx.as_mut())
                .await?;

                tx.commit().await?;

                let _ = emit_billing_notification(
                    repo.clone(),
                    &user_id.to_string(),
                    "billing.subscription.updated",
                    "Billing subscription updated",
                    "Your billing subscription status changed.",
                    serde_json::json!({
                        "plan_id": plan_id,
                        "status": "active",
                        "provider_subscription_id": out_trade_no,
                    }),
                )
                .await;
            }
        }
    }

    Ok(())
}

pub(crate) fn subscription_snapshot_from_event(
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
        && let Some(price_id) = config.checkout_price_for_plan(&snapshot.plan_id) {
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

pub(crate) async fn claim_webhook_with_lease(
    repo: Arc<PgAppRepository>,
    provider: BillingProvider,
    event_id: &str,
) -> Result<WebhookClaim> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

    sqlx::query(
        r#"
        INSERT INTO webhook_events (provider, event_id, status, created_at, updated_at)
        VALUES ($1, $2, 'pending', NOW(), NOW())
        ON CONFLICT (provider, event_id) DO NOTHING
        "#,
    )
    .bind(provider.to_string())
    .bind(event_id)
    .execute(tx.as_mut())
    .await?;

    let result = sqlx::query(
        r#"
        UPDATE webhook_events
        SET status = 'processing',
            claimed_at = NOW(),
            lease_expires_at = NOW() + INTERVAL '5 minutes',
            updated_at = NOW()
        WHERE provider = $1 AND event_id = $2
          AND (status = 'pending'
               OR (status = 'processing' AND lease_expires_at < NOW())
               OR status = 'failed')
        "#,
    )
    .bind(provider.to_string())
    .bind(event_id)
    .execute(tx.as_mut())
    .await?;

    if result.rows_affected() == 1 {
        tx.commit().await?;
        return Ok(WebhookClaim {
            event_id: event_id.to_string(),
            duplicate_processed: false,
        });
    }

    let row = sqlx::query("SELECT status FROM webhook_events WHERE provider = $1 AND event_id = $2")
        .bind(provider.to_string())
        .bind(event_id)
        .fetch_one(tx.as_mut())
        .await?;
    
    let status = row.try_get::<String, _>("status")?;
    tx.commit().await?;

    if status == "processed" {
        Ok(WebhookClaim {
            event_id: event_id.to_string(),
            duplicate_processed: true,
        })
    } else {
        anyhow::bail!("webhook event {} is currently in-flight or locked", event_id)
    }
}

pub(crate) async fn update_webhook_lease_status(
    repo: Arc<PgAppRepository>,
    provider: BillingProvider,
    event_id: &str,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    sqlx::query(
        r#"
        UPDATE webhook_events
        SET status = $3,
            error = $4,
            processed_at = CASE WHEN $3 = 'processed' THEN NOW() ELSE processed_at END,
            updated_at = NOW()
        WHERE provider = $1 AND event_id = $2
        "#,
    )
    .bind(provider.to_string())
    .bind(event_id)
    .bind(status)
    .bind(error)
    .execute(tx.as_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn update_subscription_status(
    repo: Arc<PgAppRepository>,
    stripe_subscription_id: &str,
    status: &str,
) -> Result<()> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    sqlx::query(
        r#"
        update subscriptions
        set status = $2,
            updated_at = now()
        where stripe_subscription_id = $1
        "#,
    )
    .bind(stripe_subscription_id)
    .bind(status)
    .execute(tx.as_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn emit_billing_notification(
    repo: Arc<PgAppRepository>,
    user_id: &str,
    event_type: &str,
    title: &str,
    body: &str,
    data: serde_json::Value,
) -> Result<()> {
    let user_uuid = Uuid::parse_str(user_id)?;
    let org_id = get_org_id_by_user_id(repo.clone(), user_uuid).await?;
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    sqlx::query(
        r#"
        insert into notifications (org_id, user_id, event_type, title, body, data)
        values ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(org_id)
    .bind(user_uuid)
    .bind(event_type)
    .bind(title)
    .bind(body)
    .bind(data)
    .execute(tx.as_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn expire_subscriptions(repo: Arc<PgAppRepository>) -> Result<()> {
    let mut tx = repo.raw().begin().await?;
    sqlx::query("select set_config('app.current_role', 'super_admin', true)").execute(tx.as_mut()).await?;

    let expired_subs = sqlx::query(
        r#"
        select id, user_id, current_period_end
        from subscriptions
        where status = 'active' and current_period_end < now()
        "#
    )
    .fetch_all(tx.as_mut())
    .await?;

    for row in expired_subs {
        let sub_id = row.try_get::<Uuid, _>("id")?;
        let user_id = row.try_get::<Uuid, _>("user_id")?;
        let current_period_end = row.try_get::<DateTime<Utc>, _>("current_period_end")?;
        let period_end_str = current_period_end.to_rfc3339();

        sqlx::query(
            r#"
            update subscriptions
            set status = 'expired',
                updated_at = now()
            where id = $1
            "#
        )
        .bind(sub_id)
        .execute(tx.as_mut())
        .await?;

        let dedupe_key = format!("{}:expired:{}", sub_id, period_end_str);
        sqlx::query(
            r#"
            insert into billing_outbox (event_type, payload, status, dedupe_key)
            values ($1, $2, 'pending', $3)
            on conflict (dedupe_key) do nothing
            "#
        )
        .bind("subscription.expired")
        .bind(serde_json::json!({
            "subscription_id": sub_id.to_string(),
            "user_id": user_id.to_string(),
            "period_end": period_end_str,
        }))
        .bind(&dedupe_key)
        .execute(tx.as_mut())
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn process_outbox(repo: Arc<PgAppRepository>) -> Result<()> {
    let pending = sqlx::query(
        r#"
        select id, event_type, payload, retry_count
        from billing_outbox
        where status = 'pending'
        limit 50
        "#
    )
    .fetch_all(repo.raw())
    .await?;

    for row in pending {
        let id = row.try_get::<Uuid, _>("id")?;
        let event_type = row.try_get::<String, _>("event_type")?;
        let payload = row.try_get::<serde_json::Value, _>("payload")?;
        let retry_count = row.try_get::<i32, _>("retry_count")?;

        let user_id = payload.get("user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let title = match event_type.as_str() {
            "subscription.paid" => "Subscription payment successful",
            "subscription.expired" => "Subscription expired",
            _ => "Billing update",
        };
        let body = match event_type.as_str() {
            "subscription.paid" => "Your subscription was successfully paid and is now active.",
            "subscription.expired" => "Your subscription has expired and was downgraded to the free plan.",
            _ => "Your billing details have been updated.",
        };

        let notify_res = if !user_id.is_empty() {
            emit_billing_notification(
                repo.clone(),
                user_id,
                &format!("billing.{}", event_type),
                title,
                body,
                payload.clone(),
            )
            .await
        } else {
            Err(anyhow!("missing user_id in outbox payload"))
        };

        let mut tx = repo.raw().begin().await?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)").execute(tx.as_mut()).await?;

        match notify_res {
            Ok(()) => {
                sqlx::query(
                    r#"
                    update billing_outbox
                    set status = 'sent',
                        processed_at = now(),
                        updated_at = now()
                    where id = $1
                    "#
                )
                .bind(id)
                .execute(tx.as_mut())
                .await?;
            }
            Err(error) => {
                let next_retry = retry_count + 1;
                let next_status = if next_retry > 3 { "failed" } else { "pending" };
                sqlx::query(
                    r#"
                    update billing_outbox
                    set status = $2,
                        retry_count = $3,
                        error = $4,
                        processed_at = case when $2 = 'failed' then now() else processed_at end,
                        updated_at = now()
                    where id = $1
                    "#
                )
                .bind(id)
                .bind(next_status)
                .bind(next_retry)
                .bind(error.to_string())
                .execute(tx.as_mut())
                .await?;
            }
        }
        tx.commit().await?;
    }

    Ok(())
}
