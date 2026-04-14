async fn ensure_org_and_actor(
    conn: &mut PgConnection,
    context: &AuthContext,
) -> Result<(), PgStorageError> {
    sqlx::query(
        r#"
        insert into organizations (id, name)
        values ($1, $2)
        on conflict (id) do nothing
        "#,
    )
    .bind(context.org_id().into_uuid())
    .bind(format!("Org {}", context.org_id()))
    .execute(&mut *conn)
    .await?;

    if let Some(actor_id) = context.actor_id() {
        let user_id = actor_id.into_uuid();
        sqlx::query(
            r#"
            insert into users (id, org_id, email, full_name)
            values ($1, $2, $3, $4)
            on conflict (id) do nothing
            "#,
        )
        .bind(user_id)
        .bind(context.org_id().into_uuid())
        .bind(format!("{user_id}@local.dev"))
        .bind("Local Dev User")
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}

async fn insert_notification_row(
    conn: &mut PgConnection,
    org_id: Uuid,
    params: NotificationCreateParams,
) -> Result<PgRow, PgStorageError> {
    let channels = if params.channels.is_empty() {
        vec!["in_app".to_string()]
    } else {
        params.channels
    };
    let row = sqlx::query(
        r#"
        insert into notifications (org_id, user_id, event_type, title, body, data)
        values ($1, $2, $3, $4, $5, $6)
        returning id, org_id, user_id, event_type, title, body, data, read_at, created_at, updated_at
        "#,
    )
    .bind(org_id)
    .bind(params.user_id)
    .bind(params.event_type)
    .bind(params.title)
    .bind(params.body)
    .bind(params.data.clone())
    .fetch_one(&mut *conn)
    .await?;

    let notification_id: Uuid = row.try_get("id")?;
    let payload = json!({
        "notification_id": notification_id,
        "user_id": params.user_id,
        "event_type": row.try_get::<String, _>("event_type")?,
        "title": row.try_get::<String, _>("title")?,
        "body": row.try_get::<String, _>("body")?,
        "data": row.try_get::<serde_json::Value, _>("data")?,
    });
    for channel in channels {
        sqlx::query(
            r#"
            insert into notification_outbox (org_id, notification_id, channel, status, payload, available_at)
            values ($1, $2, $3, 'pending', $4, now())
            "#,
        )
        .bind(org_id)
        .bind(notification_id)
        .bind(channel)
        .bind(payload.clone())
        .execute(&mut *conn)
        .await?;
    }
    Ok(row)
}

async fn set_current_role(conn: &mut PgConnection, role: &str) -> Result<(), PgStorageError> {
    sqlx::query("select set_config('app.current_role', $1, true)")
        .bind(role)
        .execute(conn)
        .await?;
    Ok(())
}

