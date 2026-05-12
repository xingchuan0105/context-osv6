use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Abstraction for credential storage and retrieval.
///
/// Implementations may be backed by environment variables, HashiCorp Vault,
/// AWS Secrets Manager, or an in-memory map for testing.
pub trait KeyVault: Send + Sync {
    /// Retrieve a credential by its key identifier.
    fn get(&self, key_id: &str) -> Option<String>;

    /// Rotate a credential to a new value.
    fn rotate(&self, key_id: &str, new_value: String) -> Result<(), String>;
}

/// In-memory key vault backed by a `HashMap`.
///
/// Used in production as `EnvKeyVault`: populated once at startup from
/// environment variables / config files and then treated as read-only
/// for the process lifetime.  Rotations are supported for hot-reload
/// scenarios.
#[derive(Clone)]
pub struct EnvKeyVault {
    inner: Arc<RwLock<HashMap<String, String>>>,
}

impl EnvKeyVault {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_entry(self, key_id: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner
            .write()
            .expect("key vault lock poisoned")
            .insert(key_id.into(), value.into());
        self
    }

    pub fn from_map(map: HashMap<String, String>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(map)),
        }
    }
}

impl Default for EnvKeyVault {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyVault for EnvKeyVault {
    fn get(&self, key_id: &str) -> Option<String> {
        self.inner
            .read()
            .expect("key vault lock poisoned")
            .get(key_id)
            .cloned()
    }

    fn rotate(&self, key_id: &str, new_value: String) -> Result<(), String> {
        self.inner
            .write()
            .expect("key vault lock poisoned")
            .insert(key_id.to_string(), new_value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_key_vault_get_missing_returns_none() {
        let vault = EnvKeyVault::new();
        assert!(vault.get("missing").is_none());
    }

    #[test]
    fn env_key_vault_roundtrip() {
        let vault = EnvKeyVault::new().with_entry("agent_llm", "sk-secret");
        assert_eq!(vault.get("agent_llm"), Some("sk-secret".to_string()));
    }

    #[test]
    fn env_key_vault_rotate() {
        let vault = EnvKeyVault::new().with_entry("agent_llm", "sk-old");
        vault.rotate("agent_llm", "sk-new".to_string()).unwrap();
        assert_eq!(vault.get("agent_llm"), Some("sk-new".to_string()));
    }
}
