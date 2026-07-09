//! Re-export Write material pack from `write-core` (ADR 0006 crate split).

pub use write_core::{MaterialPack, ResearchMaterials};

use super::invoker::ResearchOutcome;

impl From<&ResearchOutcome> for ResearchMaterials {
    fn from(outcome: &ResearchOutcome) -> Self {
        Self {
            cards: outcome.cards.clone(),
            citations: outcome.citations.clone(),
            reservoir: outcome.reservoir.clone(),
        }
    }
}

impl ResearchOutcome {
    pub fn materials(&self) -> ResearchMaterials {
        ResearchMaterials::from(self)
    }
}
