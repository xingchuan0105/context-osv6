# Examples

## Current weather by city name

```json
{ "location": "Beijing", "units": "metric" }
```
Result:
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

## Current weather by coordinates

```json
{ "location": "40.7128,-74.0060", "units": "imperial" }
```
Result:
```json
{
  "temperature": 68.5,
  "feels_like": 66.2,
  "humidity": 55,
  "description": "few clouds",
  "wind_speed": 8.2,
  "location": "New York",
  "units": "imperial",
  "icon": "02d",
  "observed_at": "2026-06-06T10:30:00Z"
}
```

## Default units (metric)

```json
{ "location": "London" }
```
Result: temperature in °C, wind in m/s.

## Error: missing location

```json
{}
```

Result:

```json
{
  "status": "error",
  "error": {
    "code": "MISSING_LOCATION",
    "message": "missing location"
  }
}
```

## Pre-validation: ambiguous city

```json
{ "location": "Springfield, Massachusetts" }
```

Prefer "Springfield, Massachusetts" or coordinates over bare
"Springfield" to avoid geocoding ambiguity.

## Multi-city weather (parallel calls)

**Context**: User asks "weather in Beijing, Shanghai, and
Shenzhen right now." Issue three parallel `weather_query`
calls in the same plan.

```json
[
  { "tool": "weather_query", "args": { "location": "Beijing",  "units": "metric" } },
  { "tool": "weather_query", "args": { "location": "Shanghai", "units": "metric" } },
  { "tool": "weather_query", "args": { "location": "Shenzhen", "units": "metric" } }
]
```

Do NOT batch by string concatenation ("Beijing, Shanghai,
Shenzhen") — geocoding will not parse that. Issue one call
per location.

**When NOT to batch**
- "What's the weather like from Beijing to Shanghai?" →
  road-trip weather needs `web_search` (or 5-10 waypoint
  calls, but warn the user about cost first).
- "Weather in the US Northeast" → too vague, geocoding fails.
  Use `web_search` for region-level summaries.
