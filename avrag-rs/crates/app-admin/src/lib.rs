mod admin_context;
mod preferences;

pub use admin_context::AdminContext;

/// Facade alias for API key and notification operations.
pub type AdminService = AdminContext;
