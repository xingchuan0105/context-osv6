use std::collections::BTreeMap;
use std::sync::Arc;

use contracts::auth_runtime::OrgId;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MemoryApiKeyRecord {
    pub id: Uuid,
    pub org_id: OrgId,
    pub workspace_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub rate_limit_rpm: u32,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

pub fn hash_api_key(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn register_memory_api_key(
    index: &Arc<RwLock<BTreeMap<String, MemoryApiKeyRecord>>>,
    plaintext_key: &str,
    record: MemoryApiKeyRecord,
) {
    let key_hash = hash_api_key(plaintext_key);
    index.write().await.insert(key_hash, record);
}

pub async fn validate_memory_api_key(
    index: &Arc<RwLock<BTreeMap<String, MemoryApiKeyRecord>>>,
    plaintext_key: &str,
) -> Option<MemoryApiKeyRecord> {
    let key_hash = hash_api_key(plaintext_key);
    let index = index.write().await;
    let record = index.get(&key_hash)?.clone();
    if !record.is_active {
        return None;
    }
    if record
        .expires_at
        .is_some_and(|expires_at| expires_at < Utc::now())
    {
        return None;
    }
    Some(record)
}

pub async fn deactivate_memory_api_key(
    index: &Arc<RwLock<BTreeMap<String, MemoryApiKeyRecord>>>,
    key_id: &str,
) {
    let key_uuid = Uuid::parse_str(key_id).ok();
    let mut index = index.write().await;
    for record in index.values_mut() {
        if Some(record.id) == key_uuid {
            record.is_active = false;
        }
    }
}
