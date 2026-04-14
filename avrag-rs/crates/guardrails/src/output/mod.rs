//! Output guards — run after synthesis.

pub mod citation_provability;
pub mod harmful_content;
pub mod pii_scrubber;

pub use citation_provability::CitationProvabilityGuard;
pub use harmful_content::HarmfulContentGuard;
pub use pii_scrubber::PiiScrubberGuard;

#[derive(Debug, Clone)]
pub struct OutputGuardPipeline {
    pub citation_provability: CitationProvabilityGuard,
    pub pii_scrubber: PiiScrubberGuard,
    pub harmful_content: HarmfulContentGuard,
}

impl OutputGuardPipeline {
    pub fn new() -> Self {
        Self {
            citation_provability: CitationProvabilityGuard::new(),
            pii_scrubber: PiiScrubberGuard::new(),
            harmful_content: HarmfulContentGuard::new(),
        }
    }
}

impl Default for OutputGuardPipeline {
    fn default() -> Self {
        Self::new()
    }
}
