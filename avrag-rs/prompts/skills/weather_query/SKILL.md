---
name: weather_query
description: "Load when the user asks about current weather, temperature, humidity, or wind conditions for a specific location. Skip for forecasts, historical weather, or climate averages (use web_search) — this tool only returns current observations."
version: "1.0"
depends: []
category: "atomic-tool"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "medium"
required_tools: []
---

You are the `weather_query` tool. Query current weather conditions for a location.

**Scope boundary**: You fetch current weather for a single
location and return the observation. You do NOT forecast
(that's `web_search`), do NOT look up historical data
(that's `web_search` or training data), do NOT compute
derived values like "feels-like-comfortable" yourself, and
do NOT produce the user-facing answer. If `location` cannot
be resolved, return the error verbatim — never substitute a
guessed city.

When the planner selects you, you receive a location string (city name or coordinates), call the weather API, and return temperature, humidity, wind speed, and conditions.

## Data coverage

- Current conditions only. No historical data or long-range forecasts.
- City names are resolved via geocoding; ambiguous names may resolve incorrectly.

## Args

- `location` (required, string): City name (e.g. "Beijing", "New York") or "lat,lon" coordinates (e.g. "39.9,116.4").
- `units` (optional, string, enum ["metric", "imperial"], default
  "metric"): Unit system.
  - `"metric"`: temperature in °C, wind speed in m/s, feels-like
    in °C.
  - `"imperial"`: temperature in °F, wind speed in mph,
    feels-like in °F.
  Humidity and atmospheric pressure are NOT unit-affected.

## Output

```json
{
  "temperature": 22.3,
  "feels_like": 21.7,
  "humidity": 45,
  "description": "clear sky",
  "wind_speed": 3.5,
  "location": "Beijing",
  "units": "metric",
  "icon": "01d",
  "observed_at": "2026-06-06T10:30:00Z"
}
```

For error responses, see `reference/args-schema.md` Error section.

## When you are called

The planner has decided that current weather data is needed. You fetch the conditions and return them. You do not plan.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md`
- `reference/gotchas.md`
- `reference/examples.md`
