//! Output guards — run after synthesis.

pub mod pii_scrubber;
pub mod prompt_leak;

pub use pii_scrubber::PiiScrubberGuard;
pub use prompt_leak::PromptLeakGuard;

#[derive(Debug, Clone)]
pub struct OutputGuardPipeline {
    pub prompt_leak: PromptLeakGuard,
    pub pii_scrubber: PiiScrubberGuard,
}

impl OutputGuardPipeline {
    pub fn new() -> Self {
        Self {
            prompt_leak: PromptLeakGuard::new(),
            pii_scrubber: PiiScrubberGuard::new(),
        }
    }
}

impl Default for OutputGuardPipeline {
    fn default() -> Self {
        Self::new()
    }
}
