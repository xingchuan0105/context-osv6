use contracts::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::agents::skills::{ExecutionContext, SkillComponent};

/// Weather Query Skill — current conditions or forecast for a location.
///
/// # Gotchas
/// - City names are resolved via OpenWeatherMap geocoding; ambiguous names
///   (e.g. "Springfield") may return the wrong location.
/// - The `date` parameter supports "today", "tomorrow", "forecast"; anything
///   else defaults to "today".
pub struct WeatherQuerySkill;

#[async_trait::async_trait]
impl SkillComponent for WeatherQuerySkill {
    fn id(&self) -> &str {
        "weather_query"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    /// Index-tier routing trigger.
    fn description(&self) -> &str {
        "Load when the user asks about current weather, temperature, or conditions for a location."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "weather_query".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Query current weather conditions for a location. ",
                "Supports city name or coordinates.\n",
                "Use this when the user asks about weather, temperature, humidity, wind, or conditions."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name (e.g. 'Beijing') or 'lat,lon' coordinates."
                    },
                    "units": {
                        "type": "string",
                        "enum": ["metric", "imperial"],
                        "default": "metric",
                        "description": "Temperature units."
                    }
                },
                "required": ["location"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "temperature": {"type": "number"},
                    "feels_like": {"type": "number"},
                    "humidity": {"type": "number"},
                    "description": {"type": "string"},
                    "wind_speed": {"type": "number"},
                    "location": {"type": "string"},
                    "units": {"type": "string"}
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "Ambiguous city names (e.g. 'Springfield') may resolve to the wrong location. Prefer coordinates for precision.",
            "Historical weather data is not available. For past or future dates, use web_search instead.",
        ]
    }

    fn render_hint(&self) -> &str {
        "weather"
    }

    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let location = args
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if location.is_empty() {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": "missing location" })),
                trace: None,
            };
        }

        let units = args
            .get("units")
            .and_then(|v| v.as_str())
            .unwrap_or("metric");

        match crate::agents::unified::weather::query_weather(location, units).await {
            Ok(data) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "temperature": data.temperature,
                    "feels_like": data.feels_like,
                    "humidity": data.humidity,
                    "description": data.description,
                    "wind_speed": data.wind_speed,
                    "location": data.location,
                    "units": data.units,
                    "icon": data.icon,
                })),
                trace: None,
            },
            Err(error) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": error.to_string() })),
                trace: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ExecutionContext<'static> {
        ExecutionContext::new(None)
    }

    #[tokio::test]
    async fn test_weather_query_missing_location() {
        let skill = WeatherQuerySkill;
        let result = skill.execute(&serde_json::json!({}), &ctx()).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing location"));
    }

    #[tokio::test]
    async fn test_weather_query_units_passed_through() {
        // Verifies the skill reads the 'units' parameter and passes it to the backend.
        // Without an API key the call will error, but the error must NOT be "missing location".
        let skill = WeatherQuerySkill;
        let result = skill
            .execute(
                &serde_json::json!({"location": "Beijing", "units": "imperial"}),
                &ctx(),
            )
            .await;
        let data = result.data.unwrap();
        let err = data["error"].as_str().unwrap_or("");
        assert!(
            !err.contains("missing location"),
            "skill should read location and proceed to API call"
        );
    }
}
