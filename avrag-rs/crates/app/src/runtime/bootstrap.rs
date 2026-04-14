use crate::{AppState, runtime::container::ServiceRegistry};

#[derive(Clone)]
pub struct Runtime {
    pub config: crate::runtime::config::AppConfig,
    pub services: ServiceRegistry,
    runtime_mode: &'static str,
}

impl Runtime {
    pub async fn new_memory() -> anyhow::Result<Self> {
        let state = AppState::new(crate::AppConfig::default());
        Ok(Self {
            config: state.config().clone(),
            services: ServiceRegistry::from_memory_state(&state),
            runtime_mode: "memory",
        })
    }

    pub fn runtime_mode(&self) -> &'static str {
        self.runtime_mode
    }
}
