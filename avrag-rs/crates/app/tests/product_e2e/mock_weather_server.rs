//! Mock OpenWeatherMap Current Weather API for Product E2E.
//!
//! Used when `OPENWEATHER_API_KEY` is not configured so G-17 `weather_query`
//! path smoke does not fail on missing third-party credentials.

use super::persistent_runtime::{bind_persistent_listener, spawn_persistent};
use axum::{
    Json, Router,
    extract::Query,
    response::IntoResponse,
    routing::get,
};
use serde_json::json;

#[derive(Debug, serde::Deserialize)]
struct WeatherQuery {
    q: Option<String>,
    lat: Option<String>,
    lon: Option<String>,
    #[allow(dead_code)]
    units: Option<String>,
    #[allow(dead_code)]
    appid: Option<String>,
}

/// Start a long-lived mock OpenWeather server.
///
/// Returns base URL suitable for `OPENWEATHER_BASE` (client appends `/weather`).
pub(crate) async fn start_mock_weather_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = Router::new().route("/weather", get(mock_weather_handler));

    let (listener, base_url) = bind_persistent_listener().await;
    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx)
}

async fn mock_weather_handler(Query(params): Query<WeatherQuery>) -> axum::response::Response {
    // Prefer explicit city; fall back to coords label or Beijing (golden Q125).
    let name = params
        .q
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            match (params.lat.as_deref(), params.lon.as_deref()) {
                (Some(lat), Some(lon)) => Some(format!("{lat},{lon}")),
                _ => None,
            }
        })
        .unwrap_or_else(|| "Beijing".to_string());

    // Mild localization for the golden Chinese query city.
    let (temp, desc) = if name.contains('北') || name.eq_ignore_ascii_case("beijing") {
        (22.5_f64, "clear sky")
    } else {
        (20.0_f64, "few clouds")
    };

    Json(json!({
        "name": name,
        "main": {
            "temp": temp,
            "feels_like": temp - 1.0,
            "humidity": 55
        },
        "weather": [{
            "description": desc,
            "icon": "01d"
        }],
        "wind": { "speed": 3.2 }
    }))
    .into_response()
}
