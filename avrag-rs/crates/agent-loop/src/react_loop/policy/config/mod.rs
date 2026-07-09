//! Mode configuration types, skill catalog, and YAML loaders.

mod config_types;
mod mode_loader;
mod skill_catalog;

pub use config_types::*;
pub use mode_loader::{load_mode_config, load_system_prompt};
pub use skill_catalog::*;

#[cfg(test)]
mod tests;
