use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use contracts::preferences::{AgentPreference, DailyPreferenceLog, UserPreferences};
use sqlx::Row;
use tokio::time::Duration;
use tracing::info;
use uuid::Uuid;

const DAILY_LOG_RETENTION_DAYS: i64 = 30;

pub(crate) struct AgentPreferenceConsolidationJobRunner {
    pool: sqlx::PgPool,
    interval: Duration,
    last_checked_at: Option<DateTime<Utc>>,
}

impl AgentPreferenceConsolidationJobRunner {
    pub(crate) fn from_env(pool: sqlx::PgPool) -> Option<Self> {
        let enabled = std::env::var("AVRAG_AGENT_PREF_CONSOLIDATION_ENABLED")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(true);
        if !enabled {
            return None;
        }

        let interval_hours = std::env::var("AVRAG_AGENT_PREF_CONSOLIDATION_INTERVAL_HOURS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(24);

        Some(Self {
            pool,
            interval: Duration::from_secs(interval_hours.max(1) * 60 * 60),
            last_checked_at: None,
        })
    }

    pub(crate) async fn maybe_run(&mut self) -> Result<()> {
        let now = Utc::now();
        if let Some(last_checked_at) = self.last_checked_at {
            let elapsed = now.signed_duration_since(last_checked_at);
            if elapsed.to_std().unwrap_or_default() < self.interval {
                return Ok(());
            }
        }

        let updated = self.run_once(now).await?;
        self.last_checked_at = Some(now);
        info!(
            updated_profiles = updated,
            "agent preference consolidation completed"
        );
        Ok(())
    }

    async fn run_once(&self, now: DateTime<Utc>) -> Result<usize> {
        let org_ids = sqlx::query("select id from organizations")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| row.try_get::<Uuid, _>("id").ok())
            .collect::<Vec<_>>();

        let mut updated_profiles = 0usize;
        for org_id in org_ids {
            let mut tx = self.pool.begin().await?;
            sqlx::query("select set_config('app.current_org', $1, true)")
                .bind(org_id.to_string())
                .execute(tx.as_mut())
                .await?;

            let users = sqlx::query(
                r#"
                select distinct sessions.user_id,
                       profiles.custom_preferences,
                       profiles.user_id is not null as has_profile
                from chat_sessions sessions
                left join user_profiles profiles
                  on profiles.org_id = sessions.org_id
                 and profiles.user_id = sessions.user_id
                where sessions.org_id = $1
                  and sessions.user_id is not null
                  and exists (
                    select 1
                    from chat_messages messages
                    where messages.session_id = sessions.id
                      and messages.role = 'user'
                  )
                "#,
            )
            .bind(org_id)
            .fetch_all(tx.as_mut())
            .await?;

            for row in users {
                let user_id: Uuid = row.try_get("user_id")?;
                let custom_preferences: serde_json::Value = row
                    .try_get::<Option<serde_json::Value>, _>("custom_preferences")?
                    .unwrap_or_else(|| serde_json::json!({}));
                let has_profile: bool = row.try_get("has_profile")?;
                let mut preferences =
                    serde_json::from_value::<UserPreferences>(custom_preferences.clone())
                        .unwrap_or_default();
                let since = preferences
                    .agent_memory
                    .last_consolidated_at
                    .as_deref()
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&Utc))
                    .unwrap_or_else(|| {
                        if has_profile {
                            now - ChronoDuration::days(1)
                        } else {
                            DateTime::<Utc>::from_timestamp(0, 0)
                                .unwrap_or_else(|| now - ChronoDuration::days(3650))
                        }
                    });

                let user_messages = sqlx::query(
                    r#"
                    select messages.content
                    from chat_messages messages
                    join chat_sessions sessions on sessions.id = messages.session_id
                    where sessions.org_id = $1
                      and sessions.user_id = $2
                      and messages.role = 'user'
                      and messages.created_at > $3
                    order by messages.created_at asc
                    "#,
                )
                .bind(org_id)
                .bind(user_id)
                .bind(since)
                .fetch_all(tx.as_mut())
                .await?
                .into_iter()
                .filter_map(|row| row.try_get::<String, _>("content").ok())
                .collect::<Vec<_>>();

                if user_messages.is_empty() {
                    continue;
                }

                let additions = user_messages
                    .iter()
                    .flat_map(|content| extract_explicit_interaction_preferences(content))
                    .filter(|text| !is_existing_or_blocked(&preferences, text))
                    .collect::<Vec<_>>();

                if additions.is_empty() {
                    let now_text = now.to_rfc3339();
                    preferences.agent_memory.last_consolidated_at = Some(now_text);

                    sqlx::query(
                        r#"
                        insert into user_profiles (
                            user_id, org_id, expertise_domains, preferred_answer_style,
                            frequently_asked_topics, custom_preferences, inferred_at, inference_version
                        )
                        values ($1, $2, '[]'::jsonb, null, '[]'::jsonb, $3, $4, 'agent-preference-memory-v1')
                        on conflict (user_id) do update
                        set custom_preferences = excluded.custom_preferences,
                            inferred_at = excluded.inferred_at,
                            inference_version = excluded.inference_version,
                            updated_at = now()
                        "#,
                    )
                    .bind(user_id)
                    .bind(org_id)
                    .bind(serde_json::to_value(&preferences)?)
                    .bind(now)
                    .execute(tx.as_mut())
                    .await?;
                    continue;
                }

                let now_text = now.to_rfc3339();
                preferences.agent_memory.last_consolidated_at = Some(now_text.clone());
                for text in &additions {
                    preferences.agent_memory.active.push(AgentPreference {
                        id: Uuid::new_v4().to_string(),
                        text: text.clone(),
                        category: "interaction".to_string(),
                        scope: "global".to_string(),
                        confidence: "explicit_message".to_string(),
                        source: "daily_consolidation".to_string(),
                        updated_at: now_text.clone(),
                    });
                }
                truncate_agent_daily_log(&mut preferences, now.date_naive());
                preferences.agent_memory.daily_log.push(DailyPreferenceLog {
                    date: now.date_naive().to_string(),
                    added: additions.clone(),
                    no_change: Vec::new(),
                });

                sqlx::query(
                    r#"
                    insert into user_profiles (
                        user_id, org_id, expertise_domains, preferred_answer_style,
                        frequently_asked_topics, custom_preferences, inferred_at, inference_version
                    )
                    values ($1, $2, '[]'::jsonb, null, '[]'::jsonb, $3, $4, 'agent-preference-memory-v1')
                    on conflict (user_id) do update
                    set custom_preferences = excluded.custom_preferences,
                        inferred_at = excluded.inferred_at,
                        inference_version = excluded.inference_version,
                        updated_at = now()
                    "#,
                )
                .bind(user_id)
                .bind(org_id)
                .bind(serde_json::to_value(&preferences)?)
                .bind(now)
                .execute(tx.as_mut())
                .await?;
                updated_profiles += 1;
            }

            tx.commit().await?;
        }

        Ok(updated_profiles)
    }
}

fn extract_explicit_interaction_preferences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(pref) = explicit_agent_preference_text(line) {
            out.push(pref);
            continue;
        }
        if let Some(pref) = inline_explicit_agent_preference_text(line) {
            out.push(pref);
        }
    }
    out
}

fn inline_explicit_agent_preference_text(line: &str) -> Option<String> {
    for marker in [
        "preference:",
        "用户偏好：",
        "用户偏好:",
        "remember that ",
        "remember ",
    ] {
        if let Some((_, rest)) = split_once_case_insensitive(line, marker) {
            let text = rest.trim_matches([' ', ':', '：', ',', '，', '.', '。', ';']);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn split_once_case_insensitive<'a>(value: &'a str, marker: &str) -> Option<(usize, &'a str)> {
    let lower_value = value.to_ascii_lowercase();
    let lower_marker = marker.to_ascii_lowercase();
    let idx = lower_value.find(&lower_marker)?;
    Some((idx, &value[idx + marker.len()..]))
}

fn explicit_agent_preference_text(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    for prefix in [
        "user preference:",
        "preference:",
        "用户偏好：",
        "用户偏好:",
        "请记住",
        "记住",
        "以后都",
        "以后请",
    ] {
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, prefix) {
            let text = rest.trim_matches([' ', ':', '：', ',', '，', '.', '。']);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    for prefix in ["remember that ", "remember "] {
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, prefix) {
            let text = rest.trim_matches([' ', ':', ',', '.', ';']);
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn strip_prefix_case_insensitive<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
        .then(|| &value[prefix.len()..])
}

fn is_existing_or_blocked(preferences: &UserPreferences, text: &str) -> bool {
    let normalized = normalize_preference_text(text);
    preferences
        .agent_memory
        .active
        .iter()
        .any(|item| normalize_preference_text(&item.text) == normalized)
        || preferences
            .agent_memory
            .blocked
            .iter()
            .any(|item| normalize_preference_text(&item.text) == normalized)
}

fn normalize_preference_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn truncate_agent_daily_log(preferences: &mut UserPreferences, today: NaiveDate) {
    let cutoff = today - ChronoDuration::days(DAILY_LOG_RETENTION_DAYS);
    preferences.agent_memory.daily_log.retain(|entry| {
        NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d")
            .map(|date| date >= cutoff)
            .unwrap_or(false)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_preference_extraction_ignores_plain_facts() {
        assert_eq!(
            extract_explicit_interaction_preferences(
                "The document says revenue increased.\npreference: Use concise answers."
            ),
            vec!["Use concise answers".to_string()]
        );
    }

    #[test]
    fn explicit_preference_extraction_reads_inline_marker() {
        assert_eq!(
            extract_explicit_interaction_preferences(
                "The document says revenue increased. preference: Use concise answers."
            ),
            vec!["Use concise answers".to_string()]
        );
    }

    #[test]
    fn daily_log_retention_drops_entries_older_than_thirty_days() {
        let mut preferences = UserPreferences::default();
        preferences.agent_memory.daily_log = vec![
            DailyPreferenceLog {
                date: "2026-04-01".to_string(),
                added: vec!["old".to_string()],
                no_change: Vec::new(),
            },
            DailyPreferenceLog {
                date: "2026-06-01".to_string(),
                added: vec!["recent".to_string()],
                no_change: Vec::new(),
            },
        ];
        truncate_agent_daily_log(&mut preferences, NaiveDate::from_ymd_opt(2026, 6, 14).unwrap());
        assert_eq!(preferences.agent_memory.daily_log.len(), 1);
        assert_eq!(preferences.agent_memory.daily_log[0].date, "2026-06-01");
    }
}
