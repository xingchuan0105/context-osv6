use anyhow::Result;

use crate::client::KeygenClient;

/// Issue a desktop license after successful payment webhook.
pub async fn fulfill_desktop_license(user_id: &str, plan_id: &str) -> Result<crate::LicenseSummary> {
    let client = KeygenClient::from_env()?;
    client.create_paid_license(user_id, plan_id).await
}
