use crate::types::{BillingConfig, HmacSha256};
use anyhow::{Result, anyhow, bail};
use common::OrgId;
use hmac::Mac;

pub struct StripeClient {
    config: BillingConfig,
    http: reqwest::Client,
}

impl StripeClient {
    pub fn new(config: BillingConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    fn auth_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("https://api.stripe.com{path}"))
            .bearer_auth(&self.config.stripe_secret_key)
    }

    pub async fn create_customer(
        &self,
        org_id: OrgId,
        org_name: &str,
        email: &str,
    ) -> Result<String> {
        if !self.config.stripe_enabled() {
            bail!("billing_unconfigured");
        }

        let response = self
            .auth_request(reqwest::Method::POST, "/v1/customers")
            .form(&[
                ("name", org_name.to_string()),
                ("email", email.to_string()),
                ("metadata[org_id]", org_id.to_string()),
            ])
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("stripe_create_customer_failed: {body}");
        }
        let json: serde_json::Value = serde_json::from_str(&body)?;
        json.get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("stripe_create_customer_failed: missing id"))
    }

    pub async fn create_checkout_session(
        &self,
        customer_id: &str,
        price_id: &str,
        org_id: OrgId,
        plan_id: &str,
    ) -> Result<(String, String)> {
        if !self.config.stripe_enabled() {
            bail!("billing_unconfigured");
        }

        let success_url = format!(
            "{}/dashboard?billing=success",
            self.config.public_app_base_url
        );
        let cancel_url = format!(
            "{}/dashboard?billing=cancelled",
            self.config.public_app_base_url
        );
        let response = self
            .auth_request(reqwest::Method::POST, "/v1/checkout/sessions")
            .form(&[
                ("mode", "subscription".to_string()),
                ("customer", customer_id.to_string()),
                ("success_url", success_url),
                ("cancel_url", cancel_url),
                ("line_items[0][price]", price_id.to_string()),
                ("line_items[0][quantity]", "1".to_string()),
                ("metadata[org_id]", org_id.to_string()),
                ("metadata[plan_id]", plan_id.to_string()),
                ("subscription_data[metadata][org_id]", org_id.to_string()),
                ("subscription_data[metadata][plan_id]", plan_id.to_string()),
            ])
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("stripe_checkout_failed: {body}");
        }
        let json: serde_json::Value = serde_json::from_str(&body)?;
        let url = json
            .get("url")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("stripe_checkout_failed: missing url"))?
            .to_string();
        let session_id = json
            .get("id")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("stripe_checkout_failed: missing id"))?
            .to_string();
        Ok((url, session_id))
    }

    pub async fn create_portal_session(&self, customer_id: &str) -> Result<String> {
        if !self.config.stripe_enabled() {
            bail!("billing_unconfigured");
        }
        let return_url = format!("{}/dashboard", self.config.public_app_base_url);
        let response = self
            .auth_request(reqwest::Method::POST, "/v1/billing_portal/sessions")
            .form(&[
                ("customer", customer_id.to_string()),
                ("return_url", return_url),
            ])
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("stripe_portal_failed: {body}");
        }
        let json: serde_json::Value = serde_json::from_str(&body)?;
        json.get("url")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("stripe_portal_failed: missing url"))
    }

    pub fn verify_webhook_signature(&self, payload: &[u8], sig: &str) -> Result<()> {
        if !self.config.webhook_enabled() {
            bail!("billing_unconfigured");
        }
        let mut timestamp = None;
        let mut signatures = Vec::new();
        for part in sig.split(',') {
            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "t" => timestamp = Some(value.trim().to_string()),
                    "v1" => signatures.push(value.trim().to_string()),
                    _ => {}
                }
            }
        }
        let timestamp = timestamp.ok_or_else(|| anyhow!("missing stripe timestamp"))?;
        let signed_payload = format!("{timestamp}.{}", String::from_utf8_lossy(payload));
        let mut mac = HmacSha256::new_from_slice(self.config.stripe_webhook_secret.as_bytes())?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());
        if signatures.iter().any(|sig| sig == &expected) {
            return Ok(());
        }
        bail!("invalid stripe signature")
    }
}
