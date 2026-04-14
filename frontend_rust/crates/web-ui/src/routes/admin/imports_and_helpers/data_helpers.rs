#[derive(Clone, Copy, PartialEq, Eq)]
enum OrgSort {
    NameAsc,
    UsersDesc,
    NotebooksDesc,
    QueriesDesc,
    CreatedDesc,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UserSort {
    CreatedDesc,
    EmailAsc,
    RoleAsc,
    LastActiveDesc,
}

fn parse_timestamp_like(value: &str) -> i64 {
    value.parse::<i64>().unwrap_or_default()
}

fn sort_org_rows(items: &[OrgRow], sort: OrgSort) -> Vec<OrgRow> {
    let mut rows = items.to_vec();
    match sort {
        OrgSort::NameAsc => rows.sort_by(|left, right| left.name.cmp(&right.name)),
        OrgSort::UsersDesc => rows.sort_by_key(|row| Reverse((row.user_count, row.name.clone()))),
        OrgSort::NotebooksDesc => {
            rows.sort_by_key(|row| Reverse((row.notebook_count, row.name.clone())))
        }
        OrgSort::QueriesDesc => {
            rows.sort_by_key(|row| Reverse((row.query_count, row.name.clone())))
        }
        OrgSort::CreatedDesc => rows
            .sort_by_key(|row| Reverse((parse_timestamp_like(&row.created_at), row.name.clone()))),
    }
    rows
}

fn sort_user_rows(items: &[UserRow], sort: UserSort) -> Vec<UserRow> {
    let mut rows = items.to_vec();
    match sort {
        UserSort::CreatedDesc => rows.sort_by_key(|row| {
            Reverse((
                parse_timestamp_like(&row.created_at),
                row.email.to_lowercase(),
            ))
        }),
        UserSort::EmailAsc => rows.sort_by_key(|row| row.email.to_lowercase()),
        UserSort::RoleAsc => rows.sort_by_key(|row| (row.role.clone(), row.email.to_lowercase())),
        UserSort::LastActiveDesc => rows.sort_by_key(|row| {
            Reverse((
                row.last_active_at
                    .as_deref()
                    .map(parse_timestamp_like)
                    .unwrap_or_default(),
                row.email.to_lowercase(),
            ))
        }),
    }
    rows
}

fn format_unix_timestamp(timestamp: i64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let millis = if timestamp.abs() >= 1_000_000_000_000 {
            timestamp as f64
        } else {
            (timestamp * 1000) as f64
        };
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(millis));
        let year = date.get_full_year();
        let month = date.get_month() + 1;
        let day = date.get_date();
        let hour = date.get_hours();
        let minute = date.get_minutes();
        return format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::{Local, TimeZone};

        let seconds = if timestamp.abs() >= 1_000_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        };
        Local
            .timestamp_opt(seconds, 0)
            .single()
            .map(|value| value.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| timestamp.to_string())
    }
}

#[cfg(test)]
fn now_unix_seconds() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as i64
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};

        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

#[cfg(test)]
fn audit_log_matches_window(entry: &AuditLogEntry, window: &str) -> bool {
    let max_age_secs = match window {
        "24h" => Some(24 * 60 * 60),
        "7d" => Some(7 * 24 * 60 * 60),
        "30d" => Some(30 * 24 * 60 * 60),
        _ => None,
    };
    let Some(max_age_secs) = max_age_secs else {
        return true;
    };
    now_unix_seconds().saturating_sub(entry.created_at) <= max_age_secs
}

#[cfg(test)]
fn filter_audit_logs(
    items: &[AuditLogEntry],
    query: &str,
    action_filter: &str,
    resource_filter: &str,
    actor_filter: &str,
    window_filter: &str,
) -> Vec<AuditLogEntry> {
    let normalized_query = query.trim().to_lowercase();
    let normalized_actor = actor_filter.trim().to_lowercase();

    items
        .iter()
        .filter(|entry| {
            if action_filter != "all" && entry.action != action_filter {
                return false;
            }
            if resource_filter != "all" && entry.resource_type != resource_filter {
                return false;
            }
            if !normalized_actor.is_empty()
                && !entry
                    .actor_id
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&normalized_actor)
            {
                return false;
            }
            if !normalized_query.is_empty()
                && !entry.action.to_lowercase().contains(&normalized_query)
                && !entry
                    .resource_type
                    .to_lowercase()
                    .contains(&normalized_query)
                && !entry.resource_id.to_lowercase().contains(&normalized_query)
                && !entry
                    .actor_id
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&normalized_query)
            {
                return false;
            }
            audit_log_matches_window(entry, window_filter)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
fn paginate_items<T: Clone>(items: &[T], page: usize, page_size: usize) -> Vec<T> {
    let start = page.saturating_sub(1) * page_size;
    items.iter().skip(start).take(page_size).cloned().collect()
}

#[cfg(test)]
fn audit_logs_to_csv(items: &[AuditLogEntry]) -> String {
    #[derive(Serialize)]
    struct CsvRow<'a> {
        action: &'a str,
        resource_type: &'a str,
        resource_id: &'a str,
        actor_id: &'a str,
        created_at: String,
    }

    let mut content = String::from("action,resource_type,resource_id,actor_id,created_at\n");
    for entry in items {
        let row = CsvRow {
            action: &entry.action,
            resource_type: &entry.resource_type,
            resource_id: &entry.resource_id,
            actor_id: entry.actor_id.as_deref().unwrap_or("system"),
            created_at: format_unix_timestamp(entry.created_at),
        };
        if let Ok(line) = serde_json::to_string(&row) {
            let trimmed = line.trim_matches('{').trim_matches('}');
            let mut csv = Vec::new();
            for field in trimmed.split(",\"") {
                let value = field
                    .split_once(':')
                    .map(|(_, value)| value.trim_matches('"').replace('"', "\"\""))
                    .unwrap_or_default();
                csv.push(format!("\"{value}\""));
            }
            content.push_str(&csv.join(","));
            content.push('\n');
        }
    }
    content
}

#[cfg(target_arch = "wasm32")]
fn export_text_file(filename: &str, content: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let array = js_sys::Array::new();
    array.push(&wasm_bindgen::JsValue::from_str(content));
    let blob = web_sys::Blob::new_with_str_sequence(&array)
        .map_err(|_| "failed to create blob".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "failed to create object URL".to_string())?;
    let window = web_sys::window().ok_or_else(|| "missing window".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "missing document".to_string())?;
    let anchor = document
        .create_element("a")
        .map_err(|_| "failed to create link".to_string())?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "failed to cast link".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn export_text_file(_filename: &str, _content: &str) -> Result<(), String> {
    Err("export is only available after hydration in the browser".to_string())
}

const ALL_ORGS_VALUE: &str = "__all__";
const USAGE_PERIOD_OPTIONS: &[&str] = &["7d", "30d", "90d"];

fn usage_period_label(locale: Locale, period: &str) -> &'static str {
    match period {
        "7d" => choose(locale, "近 7 天", "Last 7 days"),
        "90d" => choose(locale, "近 90 天", "Last 90 days"),
        _ => choose(locale, "近 30 天", "Last 30 days"),
    }
}

fn usage_period_hint(locale: Locale, period: &str) -> &'static str {
    match period {
        "7d" => choose(
            locale,
            "适合看短期波动和最近活跃变化。",
            "Best for short-term trends and recent activity.",
        ),
        "90d" => choose(
            locale,
            "适合看更稳定的季度级趋势。",
            "Best for steadier quarterly trends.",
        ),
        _ => choose(
            locale,
            "默认窗口，适合日常巡检和月度观察。",
            "Default window for routine monitoring.",
        ),
    }
}

fn empty_admin_usage() -> AdminUsageResponse {
    AdminUsageResponse {
        total_requests: 0,
        total_tokens: 0,
        total_documents: 0,
    }
}

fn accumulate_admin_usage(target: &mut AdminUsageResponse, item: &AdminUsageResponse) {
    target.total_requests += item.total_requests;
    target.total_tokens += item.total_tokens;
    target.total_documents += item.total_documents;
}

async fn load_admin_usage_for_scope(
    client: ApiClient,
    orgs: Vec<web_sdk::dtos::OrgRow>,
    selected_org_id: &str,
    period: &str,
) -> anyhow::Result<(AdminUsageResponse, Vec<String>)> {
    if selected_org_id.is_empty() {
        return Ok((empty_admin_usage(), Vec::new()));
    }

    if selected_org_id == ALL_ORGS_VALUE {
        let mut total = empty_admin_usage();
        let mut failed_orgs = Vec::new();
        let mut success_count = 0_u32;

        for org in orgs {
            match client
                .get_admin_usage_for_org_with_period(&org.id, period)
                .await
            {
                Ok(usage) => {
                    accumulate_admin_usage(&mut total, &usage);
                    success_count += 1;
                }
                Err(_) => failed_orgs.push(org.name),
            }
        }

        if success_count == 0 && !failed_orgs.is_empty() {
            return Err(anyhow::anyhow!(
                "failed to load usage for all organizations"
            ));
        }

        return Ok((total, failed_orgs));
    }

    let usage = client
        .get_admin_usage_for_org_with_period(selected_org_id, period)
        .await?;
    Ok((usage, Vec::new()))
}

