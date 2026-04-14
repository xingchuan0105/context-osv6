pub(crate) async fn claim_webhook(
    repo: Arc<PgAppRepository>,
    payload: &serde_json::Value,
) -> Result<WebhookClaim> {
    let event_id = payload
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("missing stripe event id"))?;
    let event_type = payload
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");

    let mut tx = repo.raw().begin().await?;
    let existing = sqlx::query(
        r#"
        select status
        from stripe_webhook_events
        where event_id = $1
        for update
        "#,
    )
    .bind(event_id)
    .fetch_optional(tx.as_mut())
    .await?;

    if let Some(row) = existing {
        let status = row.try_get::<String, _>("status")?;
        if status == "processed" {
            tx.commit().await?;
            return Ok(WebhookClaim {
                event_id: event_id.to_string(),
                duplicate_processed: true,
            });
        }
        sqlx::query(
            r#"
            update stripe_webhook_events
            set event_type = $2,
                status = 'processing',
                payload = $3,
                error = null,
                updated_at = now()
            where event_id = $1
            "#,
        )
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .execute(tx.as_mut())
        .await?;
    } else {
        sqlx::query(
            r#"
            insert into stripe_webhook_events (event_id, event_type, status, payload)
            values ($1, $2, 'processing', $3)
            "#,
        )
        .bind(event_id)
        .bind(event_type)
        .bind(payload)
        .execute(tx.as_mut())
        .await?;
    }
    tx.commit().await?;

    Ok(WebhookClaim {
        event_id: event_id.to_string(),
        duplicate_processed: false,
    })
}

pub(crate) async fn update_webhook_status(
    repo: Arc<PgAppRepository>,
    event_id: &str,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        update stripe_webhook_events
        set status = $2,
            error = $3,
            processed_at = case when $2 = 'processed' then now() else processed_at end,
            updated_at = now()
        where event_id = $1
        "#,
    )
    .bind(event_id)
    .bind(status)
    .bind(error)
    .execute(repo.raw())
    .await?;
    Ok(())
}

pub(crate) async fn process_webhook_event(
    repo: Arc<PgAppRepository>,
    payload: &serde_json::Value,
    config: &BillingConfig,
) -> Result<()> {
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
                &snapshot.org_id,
                "billing.subscription.updated",
                "Billing subscription updated",
                "Your organization billing subscription status changed.",
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
                        &existing.org_id,
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
    let mut org_id = subscription
        .pointer("/metadata/org_id")
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
    if plan_id.is_empty() {
        if let Some(mapped_plan) = config.plan_id_by_price_id(&stripe_price_id) {
            plan_id = mapped_plan.to_string();
        }
    }
    if org_id.is_empty() && stripe_customer_id.is_empty() {
        bail!("subscription metadata missing org_id and customer id");
    }
    if plan_id.is_empty() {
        bail!("subscription metadata missing plan_id");
    }

    Ok(StripeSubscriptionSnapshot {
        org_id: std::mem::take(&mut org_id),
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
    if snapshot.org_id.is_empty() && !snapshot.stripe_customer_id.is_empty() {
        if let Some(org_id) =
            find_org_id_by_customer_id(repo.clone(), &snapshot.stripe_customer_id).await?
        {
            snapshot.org_id = org_id;
        }
    }

    if let Some(existing) =
        load_existing_subscription_fields(repo.clone(), &snapshot.stripe_subscription_id).await?
    {
        if snapshot.org_id.is_empty() {
            snapshot.org_id = existing.org_id;
        }
        if snapshot.stripe_price_id.is_empty() {
            snapshot.stripe_price_id = existing.stripe_price_id;
        }
        if snapshot.plan_id.is_empty() {
            snapshot.plan_id = existing.plan_id;
        }
    }

    if snapshot.plan_id.is_empty() && !snapshot.stripe_price_id.is_empty() {
        if let Some(mapped_plan) = config.plan_id_by_price_id(&snapshot.stripe_price_id) {
            snapshot.plan_id = mapped_plan.to_string();
        }
    }
    if snapshot.stripe_price_id.is_empty() && !snapshot.plan_id.is_empty() {
        if let Some(price_id) = config.checkout_price_for_plan(&snapshot.plan_id) {
            snapshot.stripe_price_id = price_id.to_string();
        }
    }

    if snapshot.org_id.is_empty()
        || snapshot.stripe_subscription_id.is_empty()
        || snapshot.plan_id.is_empty()
        || snapshot.stripe_price_id.is_empty()
    {
        bail!("subscription snapshot incomplete after hydration");
    }
    Ok(())
}

async fn find_org_id_by_customer_id(
    repo: Arc<PgAppRepository>,
    customer_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query("select id from organizations where stripe_customer_id = $1")
        .bind(customer_id)
        .fetch_optional(repo.raw())
        .await?;
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
        select org_id, stripe_price_id, plan_id
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
            org_id: row.try_get::<Uuid, _>("org_id")?.to_string(),
            stripe_price_id: row.try_get::<String, _>("stripe_price_id")?,
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
        sqlx::query("update organizations set stripe_customer_id = $2 where id = $1")
            .bind(Uuid::parse_str(&snapshot.org_id)?)
            .bind(&snapshot.stripe_customer_id)
            .execute(tx.as_mut())
            .await?;
    }
    sqlx::query(
        r#"
        insert into subscriptions (
            org_id,
            stripe_subscription_id,
            stripe_price_id,
            plan_id,
            status,
            current_period_start,
            current_period_end,
            cancel_at_period_end
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8)
        on conflict (stripe_subscription_id) do update
        set org_id = excluded.org_id,
            stripe_price_id = excluded.stripe_price_id,
            plan_id = excluded.plan_id,
            status = excluded.status,
            current_period_start = excluded.current_period_start,
            current_period_end = excluded.current_period_end,
            cancel_at_period_end = excluded.cancel_at_period_end,
            updated_at = now()
        "#,
    )
    .bind(Uuid::parse_str(&snapshot.org_id)?)
    .bind(&snapshot.stripe_subscription_id)
    .bind(&snapshot.stripe_price_id)
    .bind(&snapshot.plan_id)
    .bind(&snapshot.status)
    .bind(snapshot.current_period_start)
    .bind(snapshot.current_period_end)
    .bind(snapshot.cancel_at_period_end)
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
    org_id: &str,
    event_type: &str,
    title: &str,
    body: &str,
    data: serde_json::Value,
) -> Result<()> {
    let org_id = OrgId::from(Uuid::parse_str(org_id)?);
    let _ = repo
        .create_notifications_for_all_users(org_id, event_type, title, body, data)
        .await?;
    Ok(())
}
