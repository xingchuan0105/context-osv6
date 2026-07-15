//! Base skill: user local clock + IP city geo (MaxMind).

use contracts::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::geoip::lookup_city;
use crate::skills::{ExecutionContext, SkillComponent};

pub struct UserContextSkill;

#[async_trait::async_trait]
impl SkillComponent for UserContextSkill {
    fn id(&self) -> &str {
        "user_context"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    fn description(&self) -> &str {
        "Load when the user asks about local time, today/tomorrow, nearby weather, or location-dependent facts without giving a city."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "user_context".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Return the user's local time/timezone (from the client) and city-level location ",
                "inferred from request IP via MaxMind GeoLite2. Call this before weather_query when ",
                "the user does not specify a city or date. Never invent a city if geo.confidence is not city."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "local_time": {"type": ["string", "null"]},
                    "timezone": {"type": ["string", "null"]},
                    "geo": {
                        "type": "object",
                        "properties": {
                            "country": {"type": ["string", "null"]},
                            "region": {"type": ["string", "null"]},
                            "city": {"type": ["string", "null"]},
                            "confidence": {"type": "string"},
                            "source": {"type": "string"},
                            "reason": {"type": ["string", "null"]}
                        }
                    }
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "Never invent city when geo.confidence is none.",
            "City is approximate (egress IP / VPN may differ from user location).",
            "Clock comes from the client; missing client_context leaves time fields null.",
        ]
    }

    fn render_hint(&self) -> &str {
        "user_context"
    }

    async fn execute<'a>(&self, _args: &Value, ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let local_time = ctx.client_local_time.clone();
        let timezone = ctx.client_timezone.clone();

        let geo = match ctx.client_ip.as_deref() {
            None | Some("") => geo_none("missing_client_ip"),
            Some(ip) => match lookup_city(ip) {
                Ok(city) => {
                    let confidence = if city.city.is_some() {
                        "city"
                    } else if city.region.is_some() {
                        "region"
                    } else {
                        "country"
                    };
                    serde_json::json!({
                        "country": city.country,
                        "region": city.region,
                        "city": city.city,
                        "confidence": confidence,
                        "source": "maxmind_geolite2",
                        "reason": null
                    })
                }
                Err(e) => geo_none(e.as_reason()),
            },
        };

        ToolResult {
            tool: self.id().to_string(),
            version: self.version().to_string(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "local_time": local_time,
                "timezone": timezone,
                "geo": geo,
            })),
            trace: None,
        }
    }
}

fn geo_none(reason: &str) -> Value {
    serde_json::json!({
        "country": null,
        "region": null,
        "city": null,
        "confidence": "none",
        "source": "maxmind_geolite2",
        "reason": reason
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::ExecutionContext;

    #[tokio::test]
    async fn user_context_returns_clock_without_geo_db() {
        let ctx = ExecutionContext::new(None).with_client_context(
            Some("8.8.8.8".into()),
            Some("2026-07-15T14:32:00+08:00".into()),
            Some("Asia/Shanghai".into()),
        );
        let skill = UserContextSkill;
        let result = skill.execute(&serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert_eq!(data["timezone"], "Asia/Shanghai");
        assert_eq!(data["local_time"], "2026-07-15T14:32:00+08:00");
        assert!(data.get("geo").is_some());
        // Without mmdb, confidence is none (or city if env has DB).
        let conf = data["geo"]["confidence"].as_str().unwrap_or("");
        assert!(conf == "none" || conf == "city" || conf == "region" || conf == "country");
    }

    #[tokio::test]
    async fn missing_client_context_still_ok() {
        let ctx = ExecutionContext::new(None);
        let skill = UserContextSkill;
        let result = skill.execute(&serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["local_time"].is_null());
        assert_eq!(data["geo"]["confidence"], "none");
    }
}
