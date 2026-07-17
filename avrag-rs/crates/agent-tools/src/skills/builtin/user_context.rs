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

    /// Index-tier routing trigger (tool catalog / progressive disclosure).
    fn description(&self) -> &str {
        "Use when the user needs local time, 'today/tomorrow', nearby weather, or other \
         location/time-dependent facts and has not given a city or calendar date. \
         Skip when the user already provided city and date/time, or the question needs \
         neither local clock nor city."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "user_context".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Use when: local clock, 'today/tomorrow', nearby/local weather, or city-dependent ",
                "facts, and the user did not supply city and/or date.\n",
                "Skip when: city and date/time already in the user message, or neither time nor ",
                "location is needed.\n",
                "Returns: local_time + timezone from the client; city-level geo from request IP ",
                "(MaxMind GeoLite2).\n",
                "Rules: do not invent a city unless geo.confidence is city; if confidence is lower, ",
                "ask the user or say location is unknown. Do not echo raw IP to the user. ",
                "Call before weather_query when city is missing."
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
            "Use when local time/nearby/city is needed and user omitted city or date.",
            "Never invent city unless geo.confidence is city; otherwise ask or admit unknown.",
            "Do not surface raw IP to the user.",
            "City is approximate (egress IP / VPN may differ from true location).",
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
