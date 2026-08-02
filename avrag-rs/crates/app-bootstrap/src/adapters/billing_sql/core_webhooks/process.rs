/// Store-side webhook application: the provider adapters have already verified
/// signatures and parsed the wire payload into [`app_core::ProviderEvent`], so
/// this module only maps typed events onto subscriptions / orders / outbox.
///
/// (Formerly the second dispatcher: it re-parsed raw `serde_json::Value` with
/// silent defaults like `plan_id.unwrap_or("pro")`, `amount.unwrap_or(2000)`,
/// `currency.unwrap_or("usd")`. Those defaults are gone — missing fields now
/// fail in the adapter, and [`ProviderEvent::Ignored`] acks events with no
/// product effect explicitly.)
pub(super) async fn process_webhook_event(
    repo: Arc<PgAppRepository>,
    provider: BillingProvider,
    event: &ProviderEvent,
) -> Result<()> {
    match provider {
        // Stripe payment stack removed 2026-07-13 — do not process residual webhooks.
        BillingProvider::Stripe => {
            bail!("billing_provider_removed: Stripe is not a product payment provider");
        }
        BillingProvider::Creem => match event {
            ProviderEvent::SubscriptionPaid {
                subscription_id,
                user_id,
                plan_id,
                price_id,
                amount_cents,
                currency,
                current_period_start,
                current_period_end,
            } => {
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
                .bind(Uuid::parse_str(user_id)?)
                .bind(subscription_id)
                .bind(price_id)
                .bind(plan_id)
                .bind(current_period_start)
                .bind(current_period_end)
                .execute(tx.as_mut())
                .await?;

                let sub_id = sqlx::query_scalar::<_, Uuid>(
                    "select id from subscriptions where billing_provider = 'creem' and provider_subscription_id = $1",
                )
                .bind(subscription_id)
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
                .bind(Uuid::parse_str(user_id)?)
                .bind(subscription_id)
                .bind(plan_id)
                .bind(amount_cents)
                .bind(currency)
                .execute(tx.as_mut())
                .await?;

                tx.commit().await?;

                if app_core::billing_domain::is_desktop_license_plan(plan_id) {
                    if let Ok(license) =
                        avrag_licensing::fulfill_desktop_license(user_id, plan_id).await
                    {
                        let _ = emit_billing_notification(
                            repo.clone(),
                            user_id,
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
                    user_id,
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
            ProviderEvent::SubscriptionCanceled { subscription_id } => {
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
                .bind(subscription_id)
                .execute(tx.as_mut())
                .await?;

                let user_id = sqlx::query_scalar::<_, Uuid>(
                    "select user_id from subscriptions where billing_provider = 'creem' and provider_subscription_id = $1",
                )
                .bind(subscription_id)
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
            ProviderEvent::AlipayOrderPaid { .. } => {
                bail!("alipay notify cannot arrive for provider creem");
            }
            ProviderEvent::Ignored => {}
        },
        BillingProvider::Alipay => match event {
            ProviderEvent::AlipayOrderPaid {
                out_trade_no,
                paid_cents,
            } => {
                let mut tx = repo.raw().begin().await?;
                set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

                let row = sqlx::query(
                    "SELECT user_id, plan_id, amount_cents FROM billing_orders WHERE provider = 'alipay' AND provider_order_id = $1",
                )
                .bind(out_trade_no)
                .fetch_one(tx.as_mut())
                .await?;

                let user_id = row.try_get::<Uuid, _>("user_id")?;
                let plan_id = row.try_get::<String, _>("plan_id")?;

                // Anti-forgery: the paid amount must match the pending order we created.
                let expected_cents = i64::from(row.try_get::<i32, _>("amount_cents")?);
                if *paid_cents != expected_cents {
                    bail!(
                        "alipay amount mismatch for {}: notify total_amount={} cents vs order amount_cents={}",
                        out_trade_no,
                        paid_cents,
                        expected_cents
                    );
                }

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
                    "select id, current_period_end from subscriptions where billing_provider = 'alipay' and provider_subscription_id = $1",
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
            ProviderEvent::SubscriptionPaid { .. } | ProviderEvent::SubscriptionCanceled { .. } => {
                bail!("creem event cannot arrive for provider alipay");
            }
            ProviderEvent::Ignored => {}
        },
    }

    Ok(())
}
