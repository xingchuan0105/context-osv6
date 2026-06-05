# Examples

## Current weather by city name

```json
{ "location": "Beijing", "units": "metric" }
```
Result:
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

## Current weather by coordinates

```json
{ "location": "40.7128,-74.0060", "units": "imperial" }
```
Result:
```json
{
  "temperature": 68,
  "feels_like": 66,
  "humidity": 55,
  "description": "few clouds",
  "wind_speed": 8.2,
  "location": "New York",
  "units": "imperial",
  "icon": "02d"
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
Result: `{"error": "missing location"}` (status: Error)

## Error: ambiguous city (example scenario)

```json
{ "location": "Springfield" }
```
May resolve to Springfield, Illinois, USA (or another Springfield) without the user's intent. Prefer coordinates or "Springfield, Massachusetts" for precision.
