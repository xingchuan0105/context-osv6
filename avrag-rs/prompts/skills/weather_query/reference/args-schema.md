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
      "description": "Unit system. Metric = °C and m/s; imperial = °F and mph."
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
    "temperature": { "type": "number", "description": "Current temperature as a float (°C or °F depending on units)." },
    "feels_like": { "type": "number", "description": "Feels-like temperature as a float (°C or °F depending on units)." },
    "humidity": { "type": "number", "minimum": 0, "maximum": 100, "description": "Relative humidity percentage (0-100)." },
    "description": { "type": "string", "description": "Weather condition text." },
    "wind_speed": { "type": "number", "description": "Wind speed in m/s (metric) or mph (imperial)." },
    "location": { "type": "string", "description": "Resolved location name." },
    "units": { "type": "string", "description": "Units used (metric or imperial)." },
    "icon": { "type": "string", "description": "OpenWeatherMap icon code for frontend rendering." },
    "observed_at": { "type": "string", "format": "date-time", "description": "ISO 8601 UTC timestamp of when the conditions were observed. May be 0-10 minutes before 'now' due to caching." }
  }
}
```

## Error response

When the call fails, the runtime returns:

```json
{
  "status": "error",
  "error": {
    "code": "MISSING_LOCATION | LOCATION_NOT_FOUND | GEOCODING_AMBIGUOUS | API_UNREACHABLE | API_KEY_MISSING | RATE_LIMITED | INVALID_COORDINATE_FORMAT | INVALID_UNITS",
    "message": "Human-readable description."
  }
}
```

### Error codes

| Error code | When it happens | Caller action |
|------------|-----------------|---------------|
| `MISSING_LOCATION` | `location` field is empty or absent. | Fix caller; do not retry. |
| `LOCATION_NOT_FOUND` | Geocoding could not resolve the name. | Try coordinates, or broader name. |
| `GEOCODING_AMBIGUOUS` | Multiple matches and the resolver picked a low-confidence one. | Prefer coordinates or include country. |
| `API_UNREACHABLE` | OpenWeatherMap is down or network error. | Retry once after a short delay; otherwise inform user. |
| `API_KEY_MISSING` | The OpenWeatherMap API key is not configured. | This is a server-side issue; do not retry. |
| `RATE_LIMITED` | Too many calls in a window. | Back off and retry; do not flood. |
| `INVALID_COORDINATE_FORMAT` | `lat,lon` is malformed. | Fix caller. |
| `INVALID_UNITS` | `units` is not `"metric"` or `"imperial"`. | Fix caller. |

The caller MUST check `status` before reading success fields.
