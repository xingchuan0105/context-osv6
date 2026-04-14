use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyResponse {
    pub status: String,
    pub scope: String,
    pub checks: Vec<ReadyCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}
