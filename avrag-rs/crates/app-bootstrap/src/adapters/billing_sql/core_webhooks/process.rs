pub(super) async fn process_webhook_event(
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
                    let amount_cents = data
                        .get("amount")
                        .or_else(|| data.get("amount_cents"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(2000);
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

                    if app_core::billing_domain::is_desktop_license_plan(&plan_id) {
                        if let Ok(license) =
                            avrag_licensing::fulfill_desktop_license(&user_id, &plan_id).await
                        {
                            let _ = emit_billing_notification(
                                repo.clone(),
                                &user_id,
                                "desktop.license.issued",
                                "Desktop license issued",
                                "Your AVRag Desktop license key is ready.",
                                serde_json::json!({
                                    "plan_id": plan_id,
                                    "license_key": license.key,
                                    "deep_link": format!("avrag-desktop://activate?key={}", license.key),
                                }),
                            )
                            .await;
                        }
                    }

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

                if app_core::billing_domain::is_desktop_license_plan(&plan_id) {
                    if let Ok(license) =
                        avrag_licensing::fulfill_desktop_license(&user_id.to_string(), &plan_id).await
                    {
                        let _ = emit_billing_notification(
                            repo.clone(),
                            &user_id.to_string(),
                            "desktop.license.issued",
                            "Desktop license issued",
                            "Your AVRag Desktop license key is ready.",
                            serde_json::json!({
                                "plan_id": plan_id,
                                "license_key": license.key,
                                "deep_link": format!("avrag-desktop://activate?key={}", license.key),
                            }),
                        )
                        .await;
                    }
                }

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
