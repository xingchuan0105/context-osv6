# Gotchas

## Ambiguous city names may resolve incorrectly

Geocoding resolves city names to coordinates. Common names like "Springfield", "Cambridge", or "Paris" may resolve to a different country than the user intended.

**Mitigation**: Use coordinates for precision, or include country/state: "Springfield, Illinois".

## No historical or forecast data

This tool returns **current conditions only**. It cannot answer:
- "What was the temperature yesterday?"
- "Will it rain tomorrow?"
- "What is the average rainfall in April?"

For these, use `web_search`.

## Requires external API availability

The weather backend depends on an external API (OpenWeatherMap). If the API is unreachable or the API key is missing, the call returns an error. Do not crash the agent loop — handle gracefully.

## Units mismatch

Default is `"metric"` (°C, m/s). If the user asks in Fahrenheit or mph and does not specify units, you may need to either:
- Call with `"imperial"` units
- Convert the metric result in your answer

## Empty location returns Error

A missing or empty `location` field returns `ToolStatus::Error` with `"missing location"`.
