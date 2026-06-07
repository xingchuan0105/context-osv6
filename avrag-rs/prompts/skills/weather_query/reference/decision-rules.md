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
- `weather_query` + `calculator`: when the user wants a derived
  metric (e.g. Beaufort wind scale, heat index in Fahrenheit),
  fetch conditions with `weather_query` then call `calculator`
  on the extracted values.
- In RAG mode, weather queries **bypass document retrieval
  entirely** — always call `weather_query` directly. The
  only exception is when the user explicitly references an
  uploaded document for the answer (e.g. "what does the
  2024 climate report say about Beijing's rainfall"), in
  which case use RAG tools against that document.
