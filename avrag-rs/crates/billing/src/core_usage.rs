pub(crate) fn build_plan_payloads(
    config: &BillingConfig,
    current_plan_id: &str,
    quotas: &HashMap<String, Vec<serde_json::Value>>,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "plan_id": PLAN_FREE,
            "name": "Free",
            "description": "Starter plan for smaller personal notebooks and trial usage.",
            "price_label": config.price_label_for_plan(PLAN_FREE),
            "interval": "month",
            "checkout_available": false,
            "current": current_plan_id == PLAN_FREE,
            "quotas": quotas.get(PLAN_FREE).cloned().unwrap_or_default(),
        }),
        serde_json::json!({
            "plan_id": PLAN_PRO,
            "name": "Pro",
            "description": "Higher monthly quotas for active document ingestion and chat workflows.",
            "price_label": config.price_label_for_plan(PLAN_PRO),
            "interval": "month",
            "checkout_available": config.checkout_available(PLAN_PRO),
            "current": current_plan_id == PLAN_PRO,
            "quotas": quotas.get(PLAN_PRO).cloned().unwrap_or_default(),
        }),
        serde_json::json!({
            "plan_id": PLAN_ENTERPRISE,
            "name": "Enterprise",
            "description": "Unlimited quota posture for larger teams and heavier workloads.",
            "price_label": config.price_label_for_plan(PLAN_ENTERPRISE),
            "interval": "month",
            "checkout_available": config.checkout_available(PLAN_ENTERPRISE),
            "current": current_plan_id == PLAN_ENTERPRISE,
            "quotas": quotas.get(PLAN_ENTERPRISE).cloned().unwrap_or_default(),
        }),
    ]
}

pub(crate) async fn get_current_subscription(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> Result<Subscription> {
    let mut tx = repo.raw().begin().await?;
    set_current_org(tx.as_mut(), &org_id.to_string()).await?;
    let row = sqlx::query(
        r#"
        select id, org_id, stripe_subscription_id, stripe_price_id, plan_id, status,
               current_period_start, current_period_end, cancel_at_period_end, created_at, updated_at
        from subscriptions
        where org_id = $1
        order by updated_at desc, created_at desc
        limit 1
        "#,
    )
    .bind(org_id.into_uuid())
    .fetch_optional(tx.as_mut())
    .await?;
    tx.commit().await?;

    if let Some(row) = row {
        return map_subscription(row);
    }

    Ok(Subscription {
        id: String::new(),
        org_id: org_id.to_string(),
        stripe_subscription_id: None,
        stripe_price_id: None,
        plan_id: PLAN_FREE.to_string(),
        status: STATUS_ACTIVE.to_string(),
        current_period_start: None,
        current_period_end: None,
        cancel_at_period_end: false,
        created_at: None,
        updated_at: None,
    })
}

pub(crate) async fn load_plan_quotas(
    repo: Arc<PgAppRepository>,
) -> Result<HashMap<String, Vec<serde_json::Value>>> {
    let rows = sqlx::query(
        "select plan_id, metric_type, soft_limit, hard_limit from quota_limits order by plan_id, metric_type",
    )
    .fetch_all(repo.raw())
    .await?;
    let mut quotas = HashMap::<String, Vec<serde_json::Value>>::new();
    for row in rows {
        quotas
            .entry(row.try_get::<String, _>("plan_id")?)
            .or_default()
            .push(serde_json::json!({
                "metric_type": row.try_get::<String, _>("metric_type")?,
                "soft_limit": row.try_get::<Option<i64>, _>("soft_limit")?,
                "hard_limit": row.try_get::<Option<i64>, _>("hard_limit")?,
            }));
    }
    Ok(quotas)
}

pub(crate) async fn load_usage(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> Result<HashMap<String, i64>> {
    let mut tx = repo.raw().begin().await?;
    set_current_org(tx.as_mut(), &org_id.to_string()).await?;
    let since = month_start();
    let rows = sqlx::query(
        r#"
        select metric_type, coalesce(sum(quantity), 0)::bigint as quantity
        from usage_events
        where org_id = $1 and created_at >= $2
        group by metric_type
        "#,
    )
    .bind(org_id.into_uuid())
    .bind(since)
    .fetch_all(tx.as_mut())
    .await?;

    let mut usage = HashMap::from([
        ("pages_processed".to_string(), 0),
        ("embedding_tokens".to_string(), 0),
        ("llm_input_tokens".to_string(), 0),
        ("llm_output_tokens".to_string(), 0),
        ("storage_bytes".to_string(), 0),
    ]);
    for row in rows {
        usage.insert(
            row.try_get::<String, _>("metric_type")?,
            row.try_get::<i64, _>("quantity")?,
        );
    }
    let storage_bytes = sqlx::query(
        r#"
        select coalesce(sum(file_size), 0)::bigint as storage_bytes
        from documents
        where org_id = $1
        "#,
    )
    .bind(org_id.into_uuid())
    .fetch_one(tx.as_mut())
    .await?
    .try_get::<i64, _>("storage_bytes")?;
    tx.commit().await?;
    usage.insert("storage_bytes".to_string(), storage_bytes);
    Ok(usage)
}

pub(crate) async fn current_metric_usage(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
    metric_type: &str,
) -> Result<i64> {
    if metric_type == "storage_bytes" {
        let mut tx = repo.raw().begin().await?;
        set_current_org(tx.as_mut(), &org_id.to_string()).await?;
        let row = sqlx::query(
            r#"
            select coalesce(sum(file_size), 0)::bigint as quantity
            from documents
            where org_id = $1
            "#,
        )
        .bind(org_id.into_uuid())
        .fetch_one(tx.as_mut())
        .await?;
        tx.commit().await?;
        return Ok(row.try_get::<i64, _>("quantity")?);
    }

    let mut tx = repo.raw().begin().await?;
    set_current_org(tx.as_mut(), &org_id.to_string()).await?;
    let since = month_start();
    let row = sqlx::query(
        r#"
        select coalesce(sum(quantity), 0)::bigint as quantity
        from usage_events
        where org_id = $1
          and metric_type = $2
          and created_at >= $3
        "#,
    )
    .bind(org_id.into_uuid())
    .bind(metric_type)
    .bind(since)
    .fetch_one(tx.as_mut())
    .await?;
    tx.commit().await?;
    Ok(row.try_get::<i64, _>("quantity")?)
}

pub(crate) async fn load_quota_limit(
    repo: Arc<PgAppRepository>,
    plan_id: &str,
    metric_type: &str,
) -> Result<Option<(Option<i64>, Option<i64>)>> {
    let row = sqlx::query(
        r#"
        select soft_limit, hard_limit
        from quota_limits
        where plan_id = $1 and metric_type = $2
        limit 1
        "#,
    )
    .bind(plan_id)
    .bind(metric_type)
    .fetch_optional(repo.raw())
    .await?;
    Ok(row.map(|row| {
        (
            row.try_get::<Option<i64>, _>("soft_limit").ok().flatten(),
            row.try_get::<Option<i64>, _>("hard_limit").ok().flatten(),
        )
    }))
}

pub(crate) async fn ensure_customer(
    repo: Arc<PgAppRepository>,
    client: &StripeClient,
    org_id: OrgId,
    user_id: UserId,
) -> Result<String> {
    if let Some(customer_id) = load_customer_id(repo.clone(), org_id).await? {
        return Ok(customer_id);
    }

    let mut tx = repo.raw().begin().await?;
    set_current_org(tx.as_mut(), &org_id.to_string()).await?;
    let row = sqlx::query(
        r#"
        select o.name, u.email
        from organizations o
        left join users u on u.id = $2 and u.org_id = o.id
        where o.id = $1
        "#,
    )
    .bind(org_id.into_uuid())
    .bind(user_id.into_uuid())
    .fetch_one(tx.as_mut())
    .await?;

    let org_name = row.try_get::<String, _>("name")?;
    let email = row
        .try_get::<Option<String>, _>("email")?
        .unwrap_or_else(|| "billing@context.local".to_string());
    let customer_id = client.create_customer(org_id, &org_name, &email).await?;
    sqlx::query("update organizations set stripe_customer_id = $2 where id = $1")
        .bind(org_id.into_uuid())
        .bind(&customer_id)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;
    Ok(customer_id)
}

pub(crate) async fn load_customer_id(
    repo: Arc<PgAppRepository>,
    org_id: OrgId,
) -> Result<Option<String>> {
    let row = sqlx::query("select stripe_customer_id from organizations where id = $1")
        .bind(org_id.into_uuid())
        .fetch_optional(repo.raw())
        .await?;
    Ok(row.and_then(|row| {
        row.try_get::<Option<String>, _>("stripe_customer_id")
            .ok()
            .flatten()
    }))
}
