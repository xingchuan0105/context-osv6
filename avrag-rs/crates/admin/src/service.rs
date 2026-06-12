use crate::models::HealthStatus;

pub struct AdminService;

impl AdminService {
    pub async fn get_health() -> HealthStatus {
        HealthStatus {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }
}
