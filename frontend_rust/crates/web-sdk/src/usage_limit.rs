//! Usage Limit API client — per-user LLM usage metering

use crate::ApiClient;
pub use contracts::usage_limit::{
    UsageLimitPolicy, UsageLimitResponse, UsageScope, UsageWindow, UsageWindows,
};

// ---------------------------------------------------------------------------
// API method
// ---------------------------------------------------------------------------

impl ApiClient {
    /// GET /api/auth/usage-limit
    ///
    /// Backend returns bare JSON UsageLimitResponse (not wrapped in an envelope).
    pub async fn get_usage_limit(&self) -> anyhow::Result<UsageLimitResponse> {
        self.get("/api/auth/usage-limit").await
    }
}
