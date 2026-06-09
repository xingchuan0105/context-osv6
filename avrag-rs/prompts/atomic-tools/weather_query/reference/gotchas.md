# Gotchas

## Geocoding behavior

OpenWeatherMap's geocoder is invoked for non-coordinate inputs.
The tool does NOT do its own geocoding; it delegates to the
upstream service. Practical implications:

- **Accepts**: well-known English city names, "City, State" /
  "City, Country" forms, native scripts for some cities
  (e.g. "Москва", "北京"). Coverage varies by region.
- **Non-English names**: many major cities are indexed under
  both their English and local-script names ("Beijing" and
  "北京" both work). Smaller cities may only resolve under
  English. If the user wrote in another language, prefer
  coordinates or English form for reliability.
- **Ambiguous names**: the geocoder returns its **first /
  highest-population** match. For "Paris" this is Paris, France
  (not Paris, Texas); for "Springfield" the first match depends
  on the geocoder's internal ordering and may be the wrong one.
- **Unknown location**: returns a `LOCATION_NOT_FOUND` error.
- **Coordinates**: the only form with zero ambiguity. Accepts
  `"lat,lon"` as decimal degrees with the latitude first, e.g.
  `"39.9042,116.4074"`. Negative values use a leading minus
  sign (e.g. `"40.7128,-74.0060"`). Hemisphere letters
  (`"39.9N"`) and degree-minute-second formats (`"39°54'15"N"`)
  are NOT accepted — convert to decimal first.

**Mitigation for ambiguity**: Use coordinates for precision, or include country/state: "Springfield, Illinois".

## `icon` field is a display code

The `icon` field is an OpenWeatherMap icon code (e.g. `"01d"`,
`"02n"`, `"10d"`). The first two digits encode the condition
class (`01` clear, `02-04` clouds, `09-10` rain, `11`
thunderstorm, `13` snow, `50` mist); the trailing `d`/`n`
distinguishes day from night. This field is intended for
**frontend rendering**, not for reasoning. Do not include
`icon` in user-facing prose answers — translate to natural
language if needed.

## No historical or forecast data

This tool returns **current conditions only**. It cannot answer:
- "What was the temperature yesterday?"
- "Will it rain tomorrow?"
- "What is the average rainfall in April?"

For these, use `web_search`.

## Requires external API availability

The weather backend depends on an external API (OpenWeatherMap). If the API is unreachable or the API key is missing, the call returns an error. Do not crash the agent loop — handle gracefully.

## Units mismatch

Default is `"metric"` (°C, m/s). If the user asks in Fahrenheit
or mph and does not specify units, default to matching the user's
stated unit; if ambiguous, prefer metric for technical or
scientific contexts and imperial for US domestic contexts.

## Missing or empty `location`

A missing or empty `location` field returns an error object
(see `reference/args-schema.md` Error codes). Do not retry —
fix the caller to provide a non-empty `location` string.

## Rate limits

The OpenWeatherMap free tier allows ~60 calls/min. The tool
does NOT batch internally — if you make N calls in a tight
loop, you may hit the limit. For 5+ locations, prefer issuing
parallel calls in a single plan (the runtime schedules them
concurrently with backoff) over sequential calls.
