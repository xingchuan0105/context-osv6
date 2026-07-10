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
    let owner_user_id = owner_user_id_for_user(repo.clone(), user_uuid).await?;
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
    sqlx::query(
        r#"
        insert into notifications (owner_user_id, user_id, event_type, title, body, data)
        values ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(owner_user_id)
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

pub(super) async fn expire_subscriptions(repo: Arc<PgAppRepository>) -> Result<()> {
    let mut tx = repo.raw().begin().await?;
    set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

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

pub(super) async fn process_outbox(repo: Arc<PgAppRepository>) -> Result<()> {
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
        set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

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
