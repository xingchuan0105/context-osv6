use crate::{AppState, runtime::container::ServiceRegistry};

#[derive(Clone)]
pub struct Runtime {
    pub config: crate::runtime::config::AppConfig,
    pub services: ServiceRegistry,
    runtime_mode: &'static str,
}

impl Runtime {
    pub async fn new_memory() -> anyhow::Result<Self> {
        let config = crate::AppConfig::default();
        let state = AppState::new(config.clone());
        Ok(Self {
            config,
            services: ServiceRegistry::from_memory_state(&state),
            runtime_mode: "memory",
        })
    }

    pub fn runtime_mode(&self) -> &'static str {
        self.runtime_mode
    }
}
