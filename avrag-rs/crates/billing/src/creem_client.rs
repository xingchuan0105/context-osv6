use crate::types::BillingConfig;
use anyhow::{Result, bail};
use common::UserId;
use serde::{Deserialize, Serialize};

pub struct CreemClient {
    config: BillingConfig,
    http: reqwest::Client,
}

#[derive(Serialize)]
struct CreemMetadata {
    user_id: String,
    plan_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pack_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount_fen: Option<i64>,
}

#[derive(Serialize)]
struct CreateCreemCheckoutRequest {
    product_id: String,
    request_id: String,
    success_url: String,
    metadata: CreemMetadata,
}

#[derive(Deserialize)]
struct CreemCheckoutResponse {
    id: String,
    checkout_url: String,
}

impl CreemClient {
    pub fn new(config: BillingConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    pub async fn create_checkout_session(
        &self,
        product_id: &str,
        user_id: UserId,
        plan_id: &str,
    ) -> Result<(String, String)> {
        self.create_checkout_session_with_metadata(
            product_id,
            user_id,
            plan_id,
            None,
            None,
            None,
            "/dashboard?billing=success",
        )
        .await
    }

    /// Wallet top-up checkout: metadata carries `purpose=wallet_topup` for webhook routing.
    pub async fn create_topup_checkout_session(
        &self,
        product_id: &str,
        user_id: UserId,
        pack_id: &str,
        amount_fen: i64,
    ) -> Result<(String, String)> {
        self.create_checkout_session_with_metadata(
            product_id,
            user_id,
            pack_id,
            Some(app_core::PRODUCT_KIND_WALLET_TOPUP.to_string()),
            Some(pack_id.to_string()),
            Some(amount_fen),
            "/settings?tab=billing&wallet_topup=success",
        )
        .await
    }

    async fn create_checkout_session_with_metadata(
        &self,
        product_id: &str,
        user_id: UserId,
        plan_id: &str,
        purpose: Option<String>,
        pack_id: Option<String>,
        amount_fen: Option<i64>,
        success_path: &str,
    ) -> Result<(String, String)> {
        if !self.config.creem_enabled() {
            bail!("creem_billing_unconfigured");
        }

        let success_url = format!(
            "{}{}",
            self.config.public_app_base_url,
            success_path
        );

        let req_body = CreateCreemCheckoutRequest {
            product_id: product_id.to_string(),
            request_id: format!("creem_{}_{}", user_id, uuid::Uuid::new_v4()),
            success_url,
            metadata: CreemMetadata {
                user_id: user_id.to_string(),
                plan_id: plan_id.to_string(),
                purpose,
                pack_id,
                amount_fen,
            },
        };

        let response = self
            .http
            .post("https://api.creem.io/v1/checkouts")
            .header("x-api-key", &self.config.creem_api_key)
            .json(&req_body)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("creem_checkout_failed: {body}");
        }

        let resp: CreemCheckoutResponse = serde_json::from_str(&body)?;
        Ok((resp.checkout_url, resp.id))
    }
}
