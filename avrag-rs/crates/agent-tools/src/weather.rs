//! Weather query client for the UnifiedAgent.
//!
//! Primary backend: [和风天气 QWeather](https://dev.qweather.com/) with JWT
//! (Ed25519 / EdDSA) per
//! <https://dev.qweather.com/docs/configuration/authentication/#json-web-token>.
//!
//! Env:
//! - `QWEATHER_HOST` — API Host (e.g. `https://xxxx.re.qweatherapi.com`)
//! - `QWEATHER_KID` — credential id (JWT header `kid`)
//! - `QWEATHER_PROJECT_ID` — project id (JWT payload `sub`)
//! - `QWEATHER_PRIVATE_KEY_PATH` — path to Ed25519 PKCS#8 PEM private key
//!   (or `QWEATHER_PRIVATE_KEY` for inline PEM)
//!
//! Optional legacy fallback: `OPENWEATHER_API_KEY` (+ `OPENWEATHER_BASE`).

use common::AppError;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

/// Weather data returned by a successful query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
    pub location: String,
    pub temperature: f64,
    pub feels_like: f64,
    pub humidity: u32,
    pub description: String,
    pub wind_speed: f64,
    pub units: String,
    pub icon: Option<String>,
}

/// Query current weather for a location.
///
/// `location` can be a city name (e.g. "Beijing") or "lat,lon" coordinates.
/// `units` should be "metric" (Celsius) or "imperial" (Fahrenheit).
pub async fn query_weather(location: &str, units: &str) -> Result<WeatherData, AppError> {
    if qweather_configured() {
        return query_qweather(location, units).await;
    }
    if std::env::var("OPENWEATHER_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return query_openweather(location, units).await;
    }
    Err(AppError::internal(
        "weather backend not configured: set QWEATHER_HOST + QWEATHER_KID + QWEATHER_PROJECT_ID + QWEATHER_PRIVATE_KEY_PATH (preferred) or OPENWEATHER_API_KEY",
    ))
}

fn qweather_configured() -> bool {
    let host = std::env::var("QWEATHER_HOST").unwrap_or_default();
    let kid = std::env::var("QWEATHER_KID").unwrap_or_default();
    let sub = std::env::var("QWEATHER_PROJECT_ID").unwrap_or_default();
    let has_key = std::env::var("QWEATHER_PRIVATE_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("QWEATHER_PRIVATE_KEY_PATH")
            .map(|p| std::path::Path::new(p.trim()).is_file())
            .unwrap_or(false);
    !host.trim().is_empty() && !kid.trim().is_empty() && !sub.trim().is_empty() && has_key
}

fn qweather_host() -> Result<String, AppError> {
    let host = std::env::var("QWEATHER_HOST")
        .map_err(|_| AppError::internal("QWEATHER_HOST is not set"))?;
    let host = host.trim().trim_end_matches('/').to_string();
    if host.is_empty() {
        return Err(AppError::internal("QWEATHER_HOST is empty"));
    }
    if host.starts_with("http://") || host.starts_with("https://") {
        Ok(host)
    } else {
        Ok(format!("https://{host}"))
    }
}

fn load_qweather_private_pem() -> Result<String, AppError> {
    if let Ok(inline) = std::env::var("QWEATHER_PRIVATE_KEY") {
        if !inline.trim().is_empty() {
            return Ok(inline);
        }
    }
    let path = std::env::var("QWEATHER_PRIVATE_KEY_PATH").map_err(|_| {
        AppError::internal("QWEATHER_PRIVATE_KEY_PATH or QWEATHER_PRIVATE_KEY is not set")
    })?;
    std::fs::read_to_string(path.trim())
        .map_err(|e| AppError::internal(format!("read QWEATHER private key: {e}")))
}

fn mint_qweather_jwt() -> Result<String, AppError> {
    let kid = std::env::var("QWEATHER_KID")
        .map_err(|_| AppError::internal("QWEATHER_KID is not set"))?;
    let sub = std::env::var("QWEATHER_PROJECT_ID")
        .map_err(|_| AppError::internal("QWEATHER_PROJECT_ID is not set"))?;
    let pem = load_qweather_private_pem()?;
    let key = EncodingKey::from_ed_pem(pem.as_bytes())
        .map_err(|e| AppError::internal(format!("QWEATHER Ed25519 key: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    // Docs: iat = now - 30s (clock skew); exp max 24h — use 15 minutes.
    let iat = now - 30;
    let exp = iat + 900;

    let mut header = Header::new(Algorithm::EdDSA);
    header.kid = Some(kid.trim().to_string());
    // Keep typ=JWT if present (allowed by QWeather).

    #[derive(Serialize)]
    struct Claims {
        sub: String,
        iat: i64,
        exp: i64,
    }
    let claims = Claims {
        sub: sub.trim().to_string(),
        iat,
        exp,
    };
    encode(&header, &claims, &key)
        .map_err(|e| AppError::internal(format!("QWEATHER JWT encode: {e}")))
}

async fn qweather_get_json(path_and_query: &str) -> Result<serde_json::Value, AppError> {
    let host = qweather_host()?;
    let token = mint_qweather_jwt()?;
    let url = format!("{host}{path_and_query}");
    let client = reqwest::Client::builder()
        .gzip(true)
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| AppError::internal(format!("http client: {e}")))?;
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| AppError::internal(format!("weather request failed: {e}")))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| AppError::internal(format!("weather body: {e}")))?;
    if !status.is_success() {
        return Err(AppError::internal(format!(
            "weather API HTTP {status}: {body}"
        )));
    }
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::internal(format!("weather JSON parse: {e}; body={body}")))?;
    let code = v
        .get("code")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    if code != "200" {
        return Err(AppError::internal(format!(
            "weather API code={code}: {body}"
        )));
    }
    Ok(v)
}

async fn query_qweather(location: &str, units: &str) -> Result<WeatherData, AppError> {
    let (loc_param, display_name) = resolve_qweather_location(location).await?;
    let path = format!(
        "/v7/weather/now?location={}&lang=zh",
        urlencoding(&loc_param)
    );
    let v = qweather_get_json(&path).await?;
    let now = v
        .get("now")
        .ok_or_else(|| AppError::internal("weather response missing now"))?;

    let temp: f64 = now
        .get("temp")
        .and_then(|t| t.as_str().and_then(|s| s.parse().ok()).or_else(|| t.as_f64()))
        .unwrap_or(0.0);
    let feels: f64 = now
        .get("feelsLike")
        .and_then(|t| t.as_str().and_then(|s| s.parse().ok()).or_else(|| t.as_f64()))
        .unwrap_or(temp);
    let humidity: u32 = now
        .get("humidity")
        .and_then(|t| t.as_str().and_then(|s| s.parse().ok()).or_else(|| t.as_u64().map(|n| n as u32)))
        .unwrap_or(0);
    let description = now
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    // QWeather windSpeed is km/h
    let wind_speed: f64 = now
        .get("windSpeed")
        .and_then(|t| t.as_str().and_then(|s| s.parse().ok()).or_else(|| t.as_f64()))
        .unwrap_or(0.0);
    let icon = now
        .get("icon")
        .and_then(|t| t.as_str())
        .map(str::to_string);

    let (temperature, feels_like, unit_label) = if units == "imperial" {
        (c_to_f(temp), c_to_f(feels), "°F".to_string())
    } else {
        (temp, feels, "°C".to_string())
    };

    Ok(WeatherData {
        location: display_name,
        temperature,
        feels_like,
        humidity,
        description,
        wind_speed,
        units: unit_label,
        icon,
    })
}

fn c_to_f(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}

/// Returns (location query for weather/now, human display name).
async fn resolve_qweather_location(location: &str) -> Result<(String, String), AppError> {
    let loc = location.trim();
    if loc.is_empty() {
        return Err(AppError::validation("location", "location is required"));
    }
    // Coordinates: lon,lat for QWeather (docs use 经度,纬度). Our skill may pass lat,lon.
    if loc.contains(',') {
        let parts: Vec<&str> = loc.split(',').map(str::trim).collect();
        if parts.len() != 2 {
            return Err(AppError::validation(
                "invalid_coords",
                "Expected 'lat,lon' or 'lon,lat' format",
            ));
        }
        let a: f64 = parts[0]
            .parse()
            .map_err(|_| AppError::validation("invalid_coords", "invalid number"))?;
        let b: f64 = parts[1]
            .parse()
            .map_err(|_| AppError::validation("invalid_coords", "invalid number"))?;
        // Heuristic: if first looks like latitude (|lat|<=90) and second like lon, swap for QWeather.
        let (lon, lat) = if a.abs() <= 90.0 && b.abs() > 90.0 {
            (b, a)
        } else if a.abs() <= 90.0 && b.abs() <= 90.0 {
            // ambiguous: product convention is lat,lon → convert to lon,lat
            (b, a)
        } else {
            (a, b)
        };
        let param = format!("{lon},{lat}");
        // Optional reverse name via geo lookup
        let path = format!(
            "/geo/v2/city/lookup?location={}&number=1&lang=zh",
            urlencoding(&param)
        );
        if let Ok(v) = qweather_get_json(&path).await {
            if let Some(name) = v
                .get("location")
                .and_then(|a| a.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
            {
                return Ok((param, name.to_string()));
            }
        }
        return Ok((param, format!("{lat},{lon}")));
    }

    // City name → GeoAPI LocationID
    let path = format!(
        "/geo/v2/city/lookup?location={}&number=1&lang=zh",
        urlencoding(loc)
    );
    let v = qweather_get_json(&path).await?;
    let city = v
        .get("location")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| AppError::internal(format!("city not found: {loc}")))?;
    let id = city
        .get("id")
        .and_then(|i| i.as_str())
        .ok_or_else(|| AppError::internal("city lookup missing id"))?
        .to_string();
    let name = city
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(loc)
        .to_string();
    let adm = city
        .get("adm1")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty() && *s != name);
    let display = match adm {
        Some(a) => format!("{name}, {a}"),
        None => name,
    };
    Ok((id, display))
}

// ── Optional OpenWeather fallback (tests / legacy) ─────────────────────────

fn openweather_base() -> String {
    std::env::var("OPENWEATHER_BASE")
        .unwrap_or_else(|_| "https://api.openweathermap.org/data/2.5".to_string())
}

async fn query_openweather(location: &str, units: &str) -> Result<WeatherData, AppError> {
    let api_key = std::env::var("OPENWEATHER_API_KEY")
        .map_err(|_| AppError::internal("OPENWEATHER_API_KEY is not set"))?;

    let is_coords = location.contains(',');
    let url = if is_coords {
        let parts: Vec<&str> = location.split(',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(AppError::validation(
                "invalid_coords",
                "Expected 'lat,lon' format",
            ));
        }
        format!(
            "{}/weather?lat={}&lon={}&units={}&appid={}",
            openweather_base(),
            parts[0],
            parts[1],
            units,
            api_key
        )
    } else {
        format!(
            "{}/weather?q={}&units={}&appid={}",
            openweather_base(),
            urlencoding(location),
            units,
            api_key
        )
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AppError::internal(format!("weather request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "weather API returned {status}: {body}"
        )));
    }

    let raw: OpenWeatherResponse = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("weather parse error: {e}")))?;

    Ok(WeatherData {
        location: raw.name.clone(),
        temperature: raw.main.temp,
        feels_like: raw.main.feels_like,
        humidity: raw.main.humidity,
        description: raw
            .weather
            .first()
            .map(|w| w.description.clone())
            .unwrap_or_default(),
        wind_speed: raw.wind.speed,
        units: if units == "imperial" {
            "°F".to_string()
        } else {
            "°C".to_string()
        },
        icon: raw.weather.first().map(|w| w.icon.clone()),
    })
}

fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

#[derive(Debug, Deserialize)]
struct OpenWeatherResponse {
    name: String,
    main: OpenWeatherMain,
    weather: Vec<OpenWeatherWeather>,
    wind: OpenWeatherWind,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherMain {
    temp: f64,
    feels_like: f64,
    humidity: u32,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherWeather {
    description: String,
    icon: String,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherWind {
    speed: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_urlencoding_space() {
        assert_eq!(urlencoding("New York"), "New%20York");
    }

    #[test]
    fn test_urlencoding_special() {
        assert_eq!(urlencoding("Beijing, CN"), "Beijing%2C%20CN");
    }

    #[test]
    fn test_urlencoding_no_change() {
        assert_eq!(urlencoding("London"), "London");
    }

    async fn mock_server_bind() -> (tokio::net::TcpListener, u16) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock server bind");
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    async fn serve_mock_response(listener: tokio::net::TcpListener, response_body: String) {
        let socket_res =
            tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept()).await;
        let (mut socket, _) = match socket_res {
            Ok(Ok(s)) => s,
            _ => return,
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
    }

    #[tokio::test]
    async fn test_query_weather_city_name_openweather_fallback() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Ensure QWeather path is off so OpenWeather mock is used.
        unsafe {
            std::env::remove_var("QWEATHER_HOST");
            std::env::remove_var("QWEATHER_KID");
            std::env::remove_var("QWEATHER_PROJECT_ID");
            std::env::remove_var("QWEATHER_PRIVATE_KEY");
            std::env::remove_var("QWEATHER_PRIVATE_KEY_PATH");
            std::env::set_var("OPENWEATHER_API_KEY", "test-api-key-123");
        }

        let body = serde_json::json!({
            "name": "Beijing",
            "main": { "temp": 25.0, "feels_like": 23.0, "humidity": 60 },
            "weather": [{ "description": "clear sky", "icon": "01d" }],
            "wind": { "speed": 3.5 }
        })
        .to_string();

        let (listener, port) = mock_server_bind().await;
        unsafe {
            std::env::set_var("OPENWEATHER_BASE", format!("http://127.0.0.1:{}", port));
        }

        let server = serve_mock_response(listener, body);
        let query = query_weather("Beijing", "metric");
        let ((), data) = tokio::join!(server, query);

        assert!(data.is_ok(), "{data:?}");
        let weather = data.unwrap();
        assert_eq!(weather.location, "Beijing");
        assert_eq!(weather.temperature, 25.0);
        assert_eq!(weather.units, "°C");
    }

    #[tokio::test]
    async fn test_query_weather_coords_openweather_fallback() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("QWEATHER_HOST");
            std::env::remove_var("QWEATHER_KID");
            std::env::remove_var("QWEATHER_PROJECT_ID");
            std::env::remove_var("QWEATHER_PRIVATE_KEY");
            std::env::remove_var("QWEATHER_PRIVATE_KEY_PATH");
            std::env::set_var("OPENWEATHER_API_KEY", "test-api-key-456");
        }

        let body = serde_json::json!({
            "name": "Tokyo",
            "main": { "temp": 77.0, "feels_like": 75.0, "humidity": 55 },
            "weather": [{ "description": "few clouds", "icon": "02d" }],
            "wind": { "speed": 5.2 }
        })
        .to_string();

        let (listener, port) = mock_server_bind().await;
        unsafe {
            std::env::set_var("OPENWEATHER_BASE", format!("http://127.0.0.1:{}", port));
        }

        let server = serve_mock_response(listener, body);
        let query = query_weather("35.6762,139.6503", "imperial");
        let ((), data) = tokio::join!(server, query);

        assert!(data.is_ok(), "{data:?}");
        let weather = data.unwrap();
        assert_eq!(weather.location, "Tokyo");
        assert_eq!(weather.temperature, 77.0);
        assert_eq!(weather.units, "°F");
    }

    #[tokio::test]
    async fn test_query_weather_api_error() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("QWEATHER_HOST");
            std::env::remove_var("QWEATHER_KID");
            std::env::remove_var("QWEATHER_PROJECT_ID");
            std::env::remove_var("QWEATHER_PRIVATE_KEY");
            std::env::remove_var("QWEATHER_PRIVATE_KEY_PATH");
            std::env::set_var("OPENWEATHER_API_KEY", "test-key");
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        unsafe {
            std::env::set_var("OPENWEATHER_BASE", format!("http://127.0.0.1:{}", port));
        }

        let server = async {
            let socket_res =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept()).await;
            if let Ok(Ok((mut socket, _))) = socket_res {
                let response =
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
            }
        };

        let query = query_weather("Nowhere", "metric");
        let ((), data) = tokio::join!(server, query);

        assert!(data.is_err());
    }
}
