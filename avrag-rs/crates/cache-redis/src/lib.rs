use contracts::auth_runtime::OrgId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

mod cache;
mod lock;

pub use cache::CacheStore;
pub use lock::{DocumentLock, DocumentLockGuard};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgScopedKeyspace {
    namespace: String,
    org_id: OrgId,
}

impl OrgScopedKeyspace {
    pub fn new(namespace: impl Into<String>, org_id: OrgId) -> Result<Self, CacheKeyError> {
        let namespace = namespace.into();
        if namespace.trim().is_empty() {
            return Err(CacheKeyError::EmptyNamespace);
        }

        Ok(Self { namespace, org_id })
    }

    pub fn cache_key(&self, suffix: &str) -> Result<String, CacheKeyError> {
        Ok(format!(
            "{}:{}:cache:{}",
            self.namespace,
            self.org_id,
            sanitize_segment(suffix)?
        ))
    }

    pub fn lock_key(
        &self,
        resource_kind: &str,
        resource_id: Uuid,
    ) -> Result<String, CacheKeyError> {
        Ok(format!(
            "{}:{}:lock:{}:{}",
            self.namespace,
            self.org_id,
            sanitize_segment(resource_kind)?,
            resource_id
        ))
    }

    pub fn idempotency_key(
        &self,
        operation: &str,
        idempotency_id: &str,
    ) -> Result<String, CacheKeyError> {
        Ok(format!(
            "{}:{}:idempotency:{}:{}",
            self.namespace,
            self.org_id,
            sanitize_segment(operation)?,
            sanitize_segment(idempotency_id)?
        ))
    }
}

#[derive(Debug, Error)]
pub enum CacheKeyError {
    #[error("cache namespace must not be empty")]
    EmptyNamespace,
    #[error("cache key segment must not be empty")]
    EmptySegment,
    #[error("cache key segment contains unsupported separator ':'")]
    InvalidSegment,
}

fn sanitize_segment(segment: &str) -> Result<&str, CacheKeyError> {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return Err(CacheKeyError::EmptySegment);
    }
    if trimmed.contains(':') {
        return Err(CacheKeyError::InvalidSegment);
    }
    Ok(trimmed)
}
