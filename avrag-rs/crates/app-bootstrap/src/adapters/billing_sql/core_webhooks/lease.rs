pub(super) async fn claim_webhook_with_lease(
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

pub(super) async fn update_webhook_lease_status(
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
