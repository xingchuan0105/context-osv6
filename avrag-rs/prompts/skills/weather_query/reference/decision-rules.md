# Decision Rules

## When `weather_query` is the right tool

- The user asks about current weather, temperature, humidity, or wind for a specific location.
- The user asks "what should I wear today" or "is it raining now" for a known location.
- A travel or logistics question requires current weather as a factor.

## When to prefer a different tool

- **Historical weather data** ("what was the weather last Tuesday?") → `web_search`. This tool only covers current conditions.
- **Future forecast beyond today** ("will it rain next weekend?") → `web_search`. This tool does not provide forecasts.
- **Climate statistics or averages** ("average rainfall in July") → `web_search` or answer from training data.
- **Weather for a moving target** ("weather along my road trip route") → `web_search` or multiple `weather_query` calls with explicit waypoints.

## Interaction with other tools

- `weather_query` + `web_search`: Use `web_search` for severe weather alerts or breaking weather news; use `weather_query` for basic current conditions.
- In RAG mode, weather is almost never answered from documents. Prefer `weather_query` directly unless the user is asking about weather described in a specific uploaded document.
