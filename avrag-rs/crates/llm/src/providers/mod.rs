use crate::route::AnyRoute;
use std::sync::Arc;

/// A configured LLM provider with a type-erased route.
#[derive(Debug, Clone)]
pub struct Provider {
    pub id: String,
    pub route: Arc<AnyRoute>,
}

impl Provider {
    pub fn new(id: impl Into<String>, route: AnyRoute) -> Self {
        Self {
            id: id.into(),
            route: Arc::new(route),
        }
    }

    pub fn from_route(id: impl Into<String>, route: impl Into<AnyRoute>) -> Self {
        Self::new(id, route.into())
    }
}

pub mod anthropic;
pub mod google;
pub mod openai;
pub mod openai_compatible;
