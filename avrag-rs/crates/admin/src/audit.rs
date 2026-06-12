use crate::models::AuditLogEntry;
use chrono::{DateTime, Utc};

pub fn usage_period_start(period: &str) -> DateTime<Utc> {
    let days = match period {
        "7d" => 7,
        "90d" => 90,
        _ => 30,
    };
    Utc::now() - chrono::TimeDelta::days(days)
}

pub fn clamp_audit_per_page(value: usize) -> usize {
    value.clamp(1, 200)
}

pub fn audit_window_start(window: Option<&str>) -> Option<DateTime<Utc>> {
    let duration = match window {
        Some("24h") => Some(chrono::TimeDelta::hours(24)),
        Some("7d") => Some(chrono::TimeDelta::days(7)),
        Some("30d") => Some(chrono::TimeDelta::days(30)),
        Some("90d") => Some(chrono::TimeDelta::days(90)),
        _ => None,
    }?;
    Some(Utc::now() - duration)
}

pub fn audit_logs_to_csv(items: &[AuditLogEntry]) -> String {
    let mut lines =
        vec!["id,action,resource_type,resource_id,actor_id,org_id,created_at".to_string()];
    for item in items {
        lines.push(format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            item.id,
            item.action.replace('"', "\"\""),
            item.resource_type.replace('"', "\"\""),
            item.resource_id.replace('"', "\"\""),
            item.actor_id
                .clone()
                .unwrap_or_default()
                .replace('"', "\"\""),
            item.org_id.clone().unwrap_or_default().replace('"', "\"\""),
            item.created_at
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_query_window_defaults_to_open_range() {
        assert!(audit_window_start(None).is_none());
        assert!(audit_window_start(Some("30d")).is_some());
    }

    #[test]
    fn clamp_audit_per_page_enforces_bounds() {
        assert_eq!(clamp_audit_per_page(0), 1);
        assert_eq!(clamp_audit_per_page(500), 200);
    }
}
