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

    // ADR-0010: release stale usage_hold rows left by crashes / failed release.
    release_stale_usage_holds(repo.clone()).await?;
    Ok(())
}

/// Default: holds older than 15 minutes without a matching release are returned.
///
/// Invoked from [`expire_subscriptions`] (worker maintenance tick). Each hold is
/// released in its own transaction so one bad row / concurrent reaper cannot
/// roll back the batch; release is idempotent on `usage_hold_release:{hold_id}`.
pub(super) async fn release_stale_usage_holds(repo: Arc<PgAppRepository>) -> Result<()> {
    let max_age_secs: i64 = std::env::var("WALLET_HOLD_MAX_AGE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900);

    // Holds whose release row is missing and older than max age.
    // `usage_hold:` is 11 chars; substring from 12 is the hold uuid suffix.
    let stale = {
        let mut tx = repo.raw().begin().await?;
        set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;
        let rows = sqlx::query(
            r#"
            select h.user_id, h.amount_fen, h.idempotency_key
            from wallet_ledger h
            where h.kind = 'usage_hold'
              and h.created_at < now() - make_interval(secs => $1)
              and not exists (
                select 1 from wallet_ledger r
                where r.kind = 'usage_hold_release'
                  and r.idempotency_key = 'usage_hold_release:' || substring(h.idempotency_key from 12)
              )
            limit 200
            "#,
        )
        .bind(max_age_secs as f64)
        .fetch_all(tx.as_mut())
        .await?;
        tx.commit().await?;
        rows
    };

    for row in stale {
        let user_id = match row.try_get::<Uuid, _>("user_id") {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "stale hold row missing user_id; skip");
                continue;
            }
        };
        let amount_fen = match row.try_get::<i64, _>("amount_fen") {
            Ok(v) => v, // negative
            Err(e) => {
                tracing::warn!(error = %e, "stale hold row missing amount_fen; skip");
                continue;
            }
        };
        let hold_fen = -amount_fen;
        if hold_fen <= 0 {
            continue;
        }
        let idem = match row.try_get::<String, _>("idempotency_key") {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "stale hold row missing idempotency_key; skip");
                continue;
            }
        };
        let hold_id_str = idem.strip_prefix("usage_hold:").unwrap_or("");
        let Ok(hold_id) = Uuid::parse_str(hold_id_str) else {
            tracing::warn!(%idem, "stale hold with unparseable id; skip");
            continue;
        };
        let release_key = format!("usage_hold_release:{hold_id}");

        let release_res: Result<()> = async {
            let mut tx = repo.raw().begin().await?;
            set_current_role(tx.as_mut(), ADMIN_ROLE_SUPER).await?;

            let existing = sqlx::query("select id from wallet_ledger where idempotency_key = $1")
                .bind(&release_key)
                .fetch_optional(tx.as_mut())
                .await?;
            if existing.is_some() {
                tx.commit().await?;
                return Ok(());
            }

            sqlx::query(
                "INSERT INTO wallets (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(user_id)
            .execute(tx.as_mut())
            .await?;
            let wallet_row = sqlx::query(
                "SELECT balance_fen FROM wallets WHERE user_id = $1 FOR UPDATE",
            )
            .bind(user_id)
            .fetch_one(tx.as_mut())
            .await?;
            let balance: i64 = wallet_row.try_get("balance_fen")?;
            let new_balance = balance + hold_fen;
            sqlx::query(
                "UPDATE wallets SET balance_fen = $2, updated_at = now() WHERE user_id = $1",
            )
            .bind(user_id)
            .bind(new_balance)
            .execute(tx.as_mut())
            .await?;
            // ON CONFLICT: concurrent reaper / late normal release already wrote this key.
            let inserted = sqlx::query(
                r#"
                INSERT INTO wallet_ledger
                  (id, user_id, kind, amount_fen, balance_after_fen, idempotency_key, metadata)
                VALUES ($1, $2, 'usage_hold_release', $3, $4, $5, $6)
                ON CONFLICT (idempotency_key) DO NOTHING
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(user_id)
            .bind(hold_fen)
            .bind(new_balance)
            .bind(&release_key)
            .bind(serde_json::json!({
                "hold_id": hold_id,
                "reaper": true,
                "max_age_secs": max_age_secs,
            }))
            .execute(tx.as_mut())
            .await?;
            if inserted.rows_affected() == 0 {
                // Lost race — do not keep the balance credit from this attempt.
                tx.rollback().await?;
                return Ok(());
            }
            tx.commit().await?;
            tracing::info!(
                %user_id,
                %hold_id,
                hold_fen,
                "released stale wallet usage_hold"
            );
            Ok(())
        }
        .await;

        if let Err(e) = release_res {
            tracing::warn!(
                %user_id,
                %hold_id,
                error = %e,
                "failed to release stale wallet usage_hold"
            );
        }
    }

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

        let copy = common::notification_copy::render(
            common::notification_copy::NotifyKind::from_billing_outbox(&event_type),
            common::notification_copy::NotifyLocale::product_default(),
        );

        let notify_res = if !user_id.is_empty() {
            emit_billing_notification(
                repo.clone(),
                user_id,
                &format!("billing.{}", event_type),
                &copy.title,
                &copy.body,
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
