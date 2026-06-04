# Args Schema

The full JSON Schema for `weather_query` args, as enforced by the runtime at the call boundary.

```json
{
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
}
```

## Field details

### `location` (required, string)

The target location for weather lookup.

**City name** (preferred for well-known cities):
- "Beijing"
- "New York"
- "London"
- "Tokyo"

**Coordinates** (preferred for precision or ambiguous city names):
- "39.9042,116.4074" (Beijing)
- "40.7128,-74.0060" (New York)

**Bad**:
- "" — empty location (runtime error)
- "near the airport" — too vague for geocoding
- "Springfield" — ambiguous without state/country context

### `units` (optional, default "metric")

- `"metric"`: Temperature in °C, wind speed in m/s.
- `"imperial"`: Temperature in °F, wind speed in mph.

When the user does not specify units, default to `"metric"` unless the user's locale or prior preferences suggest imperial.

## Output schema

```json
{
  "type": "object",
  "properties": {
    "temperature": { "type": "number", "description": "Current temperature." },
    "feels_like": { "type": "number", "description": "Feels-like temperature." },
    "humidity": { "type": "number", "description": "Relative humidity percentage." },
    "description": { "type": "string", "description": "Weather condition text." },
    "wind_speed": { "type": "number", "description": "Wind speed." },
    "location": { "type": "string", "description": "Resolved location name." },
    "units": { "type": "string", "description": "Units used (metric or imperial)." },
    "icon": { "type": "string", "description": "Weather icon code." }
  }
}
```
