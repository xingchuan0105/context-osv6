use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct LicensingConfig {
    pub host: String,
    pub account_id: String,
    pub product_token: String,
    pub license_token: String,
    pub trial_policy_id: String,
    pub standard_policy_id: String,
    pub pro_policy_id: String,
    pub public_app_base_url: String,
}

impl LicensingConfig {
    pub fn from_env() -> Result<Self> {
        let host = std::env::var("KEYGEN_HOST").unwrap_or_else(|_| "http://127.0.0.1:3001".into());
        let account_id = std::env::var("KEYGEN_ACCOUNT_ID").context("KEYGEN_ACCOUNT_ID is required")?;
        let product_token =
            std::env::var("KEYGEN_PRODUCT_TOKEN").context("KEYGEN_PRODUCT_TOKEN is required")?;
        let license_token =
            std::env::var("KEYGEN_LICENSE_TOKEN").context("KEYGEN_LICENSE_TOKEN is required")?;
        let trial_policy_id =
            std::env::var("KEYGEN_TRIAL_POLICY_ID").context("KEYGEN_TRIAL_POLICY_ID is required")?;
        let standard_policy_id = std::env::var("KEYGEN_STANDARD_POLICY_ID")
            .or_else(|_| std::env::var("KEYGEN_LICENSE_STANDARD_POLICY_ID"))
            .unwrap_or_default();
        let pro_policy_id = std::env::var("KEYGEN_PRO_POLICY_ID")
            .or_else(|_| std::env::var("KEYGEN_LICENSE_PRO_POLICY_ID"))
            .unwrap_or_default();
        let public_app_base_url = std::env::var("AVRAG_PUBLIC_BASE_URL")
            .or_else(|_| std::env::var("PUBLIC_APP_BASE_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:3000".into());

        Ok(Self {
            host: host.trim_end_matches('/').to_string(),
            account_id,
            product_token,
            license_token,
            trial_policy_id,
            standard_policy_id,
            pro_policy_id,
            public_app_base_url: public_app_base_url.trim_end_matches('/').to_string(),
        })
    }

    pub fn enabled(&self) -> bool {
        !self.account_id.is_empty()
            && !self.product_token.is_empty()
            && !self.license_token.is_empty()
    }

    pub fn policy_id_for_plan(&self, plan_id: &str) -> Result<&str> {
        match plan_id {
            "desktop-standard" | "standard" => {
                if self.standard_policy_id.is_empty() {
                    bail!("KEYGEN_STANDARD_POLICY_ID is not configured");
                }
                Ok(&self.standard_policy_id)
            }
            "desktop-pro" | "pro" => {
                if self.pro_policy_id.is_empty() {
                    bail!("KEYGEN_PRO_POLICY_ID is not configured");
                }
                Ok(&self.pro_policy_id)
            }
            other => bail!("unsupported desktop license plan: {other}"),
        }
    }
}
