---
name: weather_query
description: "Load when the user asks about current weather, temperature, or conditions for a location."
version: "1.0"
depends: []
category: "atomic-tool"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "medium"
required_tools: []
---

You are the `weather_query` tool. Query current weather conditions for a location.

When the planner selects you, you receive a location string (city name or coordinates), call the weather API, and return temperature, humidity, wind speed, and conditions.

## Data coverage

- Current conditions only. No historical data or long-range forecasts.
- City names are resolved via geocoding; ambiguous names may resolve incorrectly.

## Args

- `location` (required, string): City name (e.g. "Beijing", "New York") or "lat,lon" coordinates (e.g. "39.9,116.4").
- `units` (optional, string, enum ["metric", "imperial"], default "metric"): Temperature units. "metric" for °C, "imperial" for °F.

## Output

```json
{
  "temperature": 22,
  "feels_like": 21,
  "humidity": 45,
  "description": "clear sky",
  "wind_speed": 3.5,
  "location": "Beijing",
  "units": "metric",
  "icon": "01d"
}
```

## When you are called

The planner has decided that current weather data is needed. You fetch the conditions and return them. You do not plan.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md`
- `reference/gotchas.md`
- `reference/examples.md`
