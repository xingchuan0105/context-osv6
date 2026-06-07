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
            "price_label_cny": config.price_label_cny_for_plan(PLAN_FREE),
            "price_label_usd": config.price_label_usd_for_plan(PLAN_FREE),
            "interval": "month",
            "checkout_available": false,
            "current": current_plan_id == PLAN_FREE,
            "quotas": quotas.get(PLAN_FREE).cloned().unwrap_or_default(),
        }),
        serde_json::json!({
            "plan_id": PLAN_PLUS,
            "name": "Plus",
            "description": "Daily quotas for active document ingestion and chat workflows.",
            "price_label": config.price_label_for_plan(PLAN_PLUS),
            "price_label_cny": config.price_label_cny_for_plan(PLAN_PLUS),
            "price_label_usd": config.price_label_usd_for_plan(PLAN_PLUS),
            "interval": "month",
            "checkout_available": config.checkout_available(PLAN_PLUS),
            "current": current_plan_id == PLAN_PLUS,
            "quotas": quotas.get(PLAN_PLUS).cloned().unwrap_or_default(),
        }),
        serde_json::json!({
            "plan_id": PLAN_PRO,
            "name": "Pro",
            "description": "Unlimited quota posture for heavier workloads.",
            "price_label": config.price_label_for_plan(PLAN_PRO),
            "price_label_cny": config.price_label_cny_for_plan(PLAN_PRO),
            "price_label_usd": config.price_label_usd_for_plan(PLAN_PRO),
            "interval": "month",
            "checkout_available": config.checkout_available(PLAN_PRO),
            "current": current_plan_id == PLAN_PRO,
            "quotas": quotas.get(PLAN_PRO).cloned().unwrap_or_default(),
        }),
    ]
}

pub(crate) async fn get_current_subscription(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> Result<Subscription> {
    let mut tx = repo.raw().begin().await?;
    set_current_user(tx.as_mut(), &user_id.to_string()).await?;
    let row = sqlx::query(
        r#"
        select id, user_id, stripe_subscription_id, stripe_price_id, billing_provider,
               provider_subscription_id, provider_price_id, plan_id, status,
               current_period_start, current_period_end, cancel_at_period_end, created_at, updated_at
        from subscriptions
        where user_id = $1
        order by updated_at desc, created_at desc
        limit 1
        "#,
    )
    .bind(user_id.into_uuid())
    .fetch_optional(tx.as_mut())
    .await?;
    tx.commit().await?;

    if let Some(row) = row {
        return map_subscription(row);
    }

    Ok(Subscription {
        id: String::new(),
        user_id: user_id.to_string(),
        stripe_subscription_id: None,
        stripe_price_id: None,
        billing_provider: crate::types::BillingProvider::Stripe,
        provider_subscription_id: None,
        provider_price_id: None,
        plan_id: PLAN_FREE.to_string(),
        status: SubscriptionStatus::Active,
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
    user_id: UserId,
) -> Result<HashMap<String, i64>> {
    let mut tx = repo.raw().begin().await?;
    set_current_user(tx.as_mut(), &user_id.to_string()).await?;
    let since = month_start();
    let rows = sqlx::query(
        r#"
        select metric_type, coalesce(sum(quantity), 0)::bigint as quantity
        from usage_events
        where user_id = $1 and created_at >= $2
        group by metric_type
        "#,
    )
    .bind(user_id.into_uuid())
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
        where user_id = $1
        "#,
    )
    .bind(user_id.into_uuid())
    .fetch_one(tx.as_mut())
    .await?
    .try_get::<i64, _>("storage_bytes")?;
    tx.commit().await?;
    usage.insert("storage_bytes".to_string(), storage_bytes);
    Ok(usage)
}

pub(crate) async fn current_metric_usage(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
    metric_type: &str,
) -> Result<i64> {
    if metric_type == "storage_bytes" {
        let mut tx = repo.raw().begin().await?;
        set_current_user(tx.as_mut(), &user_id.to_string()).await?;
        let row = sqlx::query(
            r#"
            select coalesce(sum(file_size), 0)::bigint as quantity
            from documents
            where user_id = $1
            "#,
        )
        .bind(user_id.into_uuid())
        .fetch_one(tx.as_mut())
        .await?;
        tx.commit().await?;
        return Ok(row.try_get::<i64, _>("quantity")?);
    }

    let mut tx = repo.raw().begin().await?;
    set_current_user(tx.as_mut(), &user_id.to_string()).await?;
    let since = month_start();
    let row = sqlx::query(
        r#"
        select coalesce(sum(quantity), 0)::bigint as quantity
        from usage_events
        where user_id = $1
          and metric_type = $2
          and created_at >= $3
        "#,
    )
    .bind(user_id.into_uuid())
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
    user_id: UserId,
) -> Result<String> {
    if let Some(customer_id) = load_customer_id(repo.clone(), user_id).await? {
        return Ok(customer_id);
    }

    let mut tx = repo.raw().begin().await?;
    set_current_user(tx.as_mut(), &user_id.to_string()).await?;
    let row = sqlx::query(
        r#"
        select name, email
        from users
        where id = $1
        "#,
    )
    .bind(user_id.into_uuid())
    .fetch_one(tx.as_mut())
    .await?;

    let name = row.try_get::<String, _>("name")?;
    let email = row
        .try_get::<Option<String>, _>("email")?
        .unwrap_or_else(|| "billing@context.local".to_string());
    let customer_id = client.create_customer(user_id, &name, &email).await?;
    sqlx::query("update users set stripe_customer_id = $2 where id = $1")
        .bind(user_id.into_uuid())
        .bind(&customer_id)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;
    Ok(customer_id)
}

pub(crate) async fn load_customer_id(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> Result<Option<String>> {
    let row = sqlx::query("select stripe_customer_id from users where id = $1")
        .bind(user_id.into_uuid())
        .fetch_optional(repo.raw())
        .await?;
    Ok(row.and_then(|row| {
        row.try_get::<Option<String>, _>("stripe_customer_id")
            .ok()
            .flatten()
    }))
}

pub(crate) async fn load_user_plan_id(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> Result<String> {
    // Most recent active subscription wins; fall back to "free" so the meter
    // always renders something (matches get_current_subscription semantics).
    let row = sqlx::query(
        r#"
        select plan_id
        from subscriptions
        where user_id = $1 and status = 'active'
        order by updated_at desc, created_at desc
        limit 1
        "#,
    )
    .bind(user_id.into_uuid())
    .fetch_optional(repo.raw())
    .await?;
    Ok(row
        .and_then(|row| row.try_get::<String, _>("plan_id").ok())
        .unwrap_or_else(|| PLAN_FREE.to_string()))
}

pub(crate) async fn load_usage_window(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> Result<UsageWindowResponse> {
    let user_uuid = user_id.into_uuid();
    let plan_id = load_user_plan_id(repo.clone(), user_id).await?;

    // Plan-level 5h/7d caps (0/0 = unlimited, same convention as usage_limit/service.rs).
    let policy_row = sqlx::query(
        r#"
        select rolling_5h_limit_units, rolling_7d_limit_units
        from usage_limit_plan_policies
        where plan_id = $1
        "#,
    )
    .bind(&plan_id)
    .fetch_optional(repo.raw())
    .await?;
    let (limit_5h, limit_7d) = policy_row
        .map(|row| {
            (
                row.try_get::<i64, _>("rolling_5h_limit_units").unwrap_or(0),
                row.try_get::<i64, _>("rolling_7d_limit_units").unwrap_or(0),
            )
        })
        .unwrap_or((0, 0));

    let now = Utc::now();
    let cutoff_5h = now - Duration::hours(5);
    let cutoff_7d = now - Duration::days(7);

    let used_5h = sum_usage_units_window(repo.clone(), user_uuid, cutoff_5h).await?;
    let used_7d = sum_usage_units_window(repo.clone(), user_uuid, cutoff_7d).await?;
    let oldest_5h = oldest_event_in_window(repo.clone(), user_uuid, cutoff_5h).await?;
    let oldest_7d = oldest_event_in_window(repo.clone(), user_uuid, cutoff_7d).await?;

    let reset_5h = oldest_5h
        .map(|t| t + Duration::hours(5))
        .unwrap_or(now);
    let reset_7d = oldest_7d
        .map(|t| t + Duration::days(7))
        .unwrap_or(now);

    let bucket_5h = build_bucket(used_5h, limit_5h, reset_5h);
    let bucket_7d = build_bucket(used_7d, limit_7d, reset_7d);

    Ok(UsageWindowResponse {
        plan_id,
        rolling_5h: bucket_5h.clone(),
        rolling_7d: bucket_7d.clone(),
        soft_limit_hit: LimitHits {
            rolling_5h: bucket_5h.percentage >= 80,
            rolling_7d: bucket_7d.percentage >= 80,
        },
        hard_limit_hit: LimitHits {
            rolling_5h: bucket_5h.percentage >= 100,
            rolling_7d: bucket_7d.percentage >= 100,
        },
    })
}

fn build_bucket(used: i64, limit: i64, reset_at: DateTime<Utc>) -> UsageWindowBucket {
    let raw = if limit > 0 {
        (used as f64 / limit as f64) * 100.0
    } else {
        0.0
    };
    let percentage = raw.round().clamp(0.0, 100.0) as i32;
    UsageWindowBucket {
        used,
        limit,
        percentage,
        reset_at,
    }
}

async fn sum_usage_units_window(
    repo: Arc<PgAppRepository>,
    user_id: uuid::Uuid,
    since: DateTime<Utc>,
) -> Result<i64> {
    let row = sqlx::query(
        r#"
        select coalesce(sum(usage_units), 0)::bigint as total
        from llm_usage_events
        where user_id = $1 and created_at >= $2
        "#,
    )
    .bind(user_id)
    .bind(since)
    .fetch_one(repo.raw())
    .await?;
    Ok(row.try_get::<i64, _>("total")?)
}

async fn oldest_event_in_window(
    repo: Arc<PgAppRepository>,
    user_id: uuid::Uuid,
    since: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>> {
    let row = sqlx::query(
        r#"
        select min(created_at) as oldest
        from llm_usage_events
        where user_id = $1 and created_at >= $2
        "#,
    )
    .bind(user_id)
    .bind(since)
    .fetch_one(repo.raw())
    .await?;
    Ok(row.try_get::<Option<DateTime<Utc>>, _>("oldest")?)
}

pub(crate) async fn load_usage_history(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
    days: i32,
) -> Result<UsageHistoryResponse> {
    // Bucket llm_usage_events by day for the last N days. Returns one row
    // per day that had at least one event. Days with zero events are
    // omitted — the frontend fills gaps client-side.
    let user_uuid = user_id.into_uuid();
    let days = days.clamp(1, 90);
    let since = Utc::now() - Duration::days(days as i64);

    let rows = sqlx::query(
        r#"
        select date_trunc('day', created_at)::date as day,
               coalesce(sum(usage_units), 0)::bigint as tokens
        from llm_usage_events
        where user_id = $1 and created_at >= $2
        group by day
        order by day asc
        "#,
    )
    .bind(user_uuid)
    .bind(since)
    .fetch_all(repo.raw())
    .await?;

    let daily = rows
        .into_iter()
        .map(|row| {
            let day: chrono::NaiveDate = row.try_get("day")?;
            Ok::<_, anyhow::Error>(DailyUsage {
                date: day.format("%Y-%m-%d").to_string(),
                tokens: row.try_get::<i64, _>("tokens")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(UsageHistoryResponse { daily })
}

pub(crate) async fn load_usage_forecast(
    repo: Arc<PgAppRepository>,
    user_id: UserId,
) -> Result<UsageForecastResponse> {
    // Project a 30-day average against the user's current 7d plan limit.
    // The "avg" field is total / 30 (per day) so the frontend can render
    // either "you use ~8K tokens/day" or "you used 240K this month".
    let user_uuid = user_id.into_uuid();
    let plan_id = load_user_plan_id(repo.clone(), user_id).await?;

    // Plan-level 7d cap (0 = unlimited, same convention as usage_limit/service.rs).
    let policy_row = sqlx::query(
        r#"
        select rolling_7d_limit_units
        from usage_limit_plan_policies
        where plan_id = $1
        "#,
    )
    .bind(&plan_id)
    .fetch_optional(repo.raw())
    .await?;
    let current_limit_7d = policy_row
        .map(|row| row.try_get::<i64, _>("rolling_7d_limit_units").unwrap_or(0))
        .unwrap_or(0);

    let now = Utc::now();
    let cutoff_30d = now - Duration::days(30);
    let total_30d = sum_usage_units_window(repo, user_uuid, cutoff_30d).await?;
    let avg_30d_tokens = total_30d / 30;
    let projected_30d_tokens = avg_30d_tokens * 30;

    let usage_pct_of_limit = if current_limit_7d > 0 {
        (projected_30d_tokens as f64 / current_limit_7d as f64) * 100.0
    } else {
        0.0
    };
    let upgrade_recommended = usage_pct_of_limit >= 80.0;

    let suggestion_zh = if upgrade_recommended {
        "按当前用量，本月建议升级到 Plus（7d 限额 4M）".to_string()
    } else {
        "按当前用量，本月无需升级".to_string()
    };
    let suggestion_en = if upgrade_recommended {
        "Based on current usage, upgrading to Plus is recommended this month (7d limit: 4M)"
            .to_string()
    } else {
        "Based on current usage, no upgrade needed this month".to_string()
    };

    Ok(UsageForecastResponse {
        current_plan: plan_id,
        avg_30d_tokens,
        projected_30d_tokens,
        current_limit_7d,
        upgrade_recommended,
        suggestion_zh,
        suggestion_en,
    })
}
