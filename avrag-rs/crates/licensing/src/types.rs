use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseSummary {
    pub id: String,
    pub key: String,
    pub status: String,
    pub kind: String,
    pub max_machines: Option<u32>,
    pub machines_count: Option<u32>,
    pub metadata: serde_json::Value,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseMachine {
    pub id: String,
    pub fingerprint: Option<String>,
    pub name: Option<String>,
    pub platform: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct KeygenListResponse<T> {
    pub data: Vec<KeygenResource<T>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct KeygenResource<T> {
    pub id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub resource_type: String,
    pub attributes: T,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct KeygenLicenseAttributes {
    pub key: String,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub max_machines: Option<u32>,
    #[serde(default)]
    pub machines_count: Option<u32>,
    pub created: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct KeygenMachineAttributes {
    pub fingerprint: Option<String>,
    pub name: Option<String>,
    pub platform: Option<String>,
    pub heartbeat: Option<String>,
    pub created: Option<String>,
}
