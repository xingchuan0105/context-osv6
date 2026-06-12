//! Weather query client for the UnifiedAgent.
//!
//! Uses the OpenWeatherMap Current Weather API (free tier).
//! Requires `OPENWEATHER_API_KEY` environment variable.

use common::AppError;
use serde::{Deserialize, Serialize};

fn openweather_base() -> String {
    std::env::var("OPENWEATHER_BASE")
        .unwrap_or_else(|_| "https://api.openweathermap.org/data/2.5".to_string())
}

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

    // -----------------------------------------------------------------------
    // Mock HTTP server for weather API tests
    // -----------------------------------------------------------------------

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
    async fn test_query_weather_city_name() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let api_key = "test-api-key-123";
        unsafe {
            std::env::set_var("OPENWEATHER_API_KEY", api_key);
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

        assert!(data.is_ok());
        let weather = data.unwrap();
        assert_eq!(weather.location, "Beijing");
        assert_eq!(weather.temperature, 25.0);
        assert_eq!(weather.feels_like, 23.0);
        assert_eq!(weather.humidity, 60);
        assert_eq!(weather.description, "clear sky");
        assert_eq!(weather.wind_speed, 3.5);
        assert_eq!(weather.units, "°C");
        assert_eq!(weather.icon, Some("01d".to_string()));
    }

    #[tokio::test]
    async fn test_query_weather_coords() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let api_key = "test-api-key-456";
        unsafe {
            std::env::set_var("OPENWEATHER_API_KEY", api_key);
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

        assert!(data.is_ok());
        let weather = data.unwrap();
        assert_eq!(weather.location, "Tokyo");
        assert_eq!(weather.temperature, 77.0);
        assert_eq!(weather.units, "°F");
    }

    #[tokio::test]
    async fn test_query_weather_api_error() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
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
