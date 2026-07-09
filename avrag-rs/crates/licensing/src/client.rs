use anyhow::{Context, Result, bail};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

use crate::config::LicensingConfig;
use crate::types::{
    KeygenLicenseAttributes, KeygenListResponse, KeygenMachineAttributes, KeygenResource,
    LicenseMachine, LicenseSummary,
};

#[derive(Clone)]
pub struct KeygenClient {
    config: LicensingConfig,
    http: reqwest::Client,
}

#[derive(Serialize)]
struct CreateLicenseRequest<'a> {
    data: CreateLicenseData<'a>,
}

#[derive(Serialize)]
struct CreateLicenseData<'a> {
    #[serde(rename = "type")]
    resource_type: &'static str,
    attributes: CreateLicenseAttributes<'a>,
    relationships: CreateLicenseRelationships<'a>,
}

#[derive(Serialize)]
struct CreateLicenseAttributes<'a> {
    metadata: CreateLicenseMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<&'a str>,
}

#[derive(Serialize)]
struct CreateLicenseMetadata {
    user_id: String,
    product: String,
    major_version_included: u32,
}

#[derive(Serialize)]
struct CreateLicenseRelationships<'a> {
    policy: PolicyRelationship<'a>,
}

#[derive(Serialize)]
struct PolicyRelationship<'a> {
    data: PolicyRef<'a>,
}

#[derive(Serialize)]
struct PolicyRef<'a> {
    #[serde(rename = "type")]
    resource_type: &'static str,
    id: &'a str,
}

#[derive(Deserialize)]
struct KeygenSingleResponse<T> {
    data: KeygenResource<T>,
}

#[derive(Deserialize)]
struct KeygenLicenseKeyAttributes {
    key: String,
}

impl KeygenClient {
    pub fn new(config: LicensingConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self::new(LicensingConfig::from_env()?))
    }

    pub fn config(&self) -> &LicensingConfig {
        &self.config
    }

    pub async fn ping(&self) -> Result<()> {
        let url = format!("{}/v1/ping", self.config.host);
        let response = self.http.get(url).send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            bail!("keygen ping failed: {}", response.status());
        }
    }

    pub async fn list_licenses_for_user(&self, user_id: &str) -> Result<Vec<LicenseSummary>> {
        let url = format!(
            "{}/v1/accounts/{}/licenses?limit=100",
            self.config.host, self.config.account_id
        );
        let response = self
            .authorized_get(&url)
            .send()
            .await
            .context("keygen list licenses request failed")?;
        let body = response.text().await?;
        let parsed: KeygenListResponse<KeygenLicenseAttributes> =
            serde_json::from_str(&body).context(format!("invalid keygen licenses response: {body}"))?;

        Ok(parsed
            .data
            .into_iter()
            .filter(|item| {
                item.attributes
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("user_id"))
                    .and_then(|v| v.as_str())
                    .map(|id| id == user_id)
                    .unwrap_or(false)
            })
            .map(map_license)
            .collect())
    }

    pub async fn list_machines(&self, license_id: &str) -> Result<Vec<LicenseMachine>> {
        let url = format!("{}/v1/licenses/{license_id}/machines", self.config.host);
        let response = self
            .authorized_get(&url)
            .send()
            .await
            .context("keygen list machines request failed")?;
        let body = response.text().await?;
        let parsed: KeygenListResponse<KeygenMachineAttributes> =
            serde_json::from_str(&body).context(format!("invalid keygen machines response: {body}"))?;
        Ok(parsed.data.into_iter().map(map_machine).collect())
    }

    pub async fn deactivate_machine(&self, machine_id: &str) -> Result<()> {
        let url = format!("{}/v1/machines/{machine_id}", self.config.host);
        let response = self
            .authorized_delete(&url)
            .send()
            .await
            .context("keygen deactivate machine request failed")?;
        if response.status().is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            bail!("keygen deactivate machine failed: {body}");
        }
    }

    pub async fn create_trial_license(&self, user_id: &str) -> Result<LicenseSummary> {
        self.create_license(user_id, &self.config.trial_policy_id, "trial")
            .await
    }

    pub async fn create_paid_license(
        &self,
        user_id: &str,
        plan_id: &str,
    ) -> Result<LicenseSummary> {
        let policy_id = self.config.policy_id_for_plan(plan_id)?;
        let kind = if plan_id.contains("pro") { "pro" } else { "standard" };
        self.create_license(user_id, policy_id, kind).await
    }

    async fn create_license(
        &self,
        user_id: &str,
        policy_id: &str,
        kind: &str,
    ) -> Result<LicenseSummary> {
        let url = format!("{}/v1/accounts/{}/licenses", self.config.host, self.config.account_id);
        let payload = CreateLicenseRequest {
            data: CreateLicenseData {
                resource_type: "licenses",
                attributes: CreateLicenseAttributes {
                    metadata: CreateLicenseMetadata {
                        user_id: user_id.to_string(),
                        product: "desktop".to_string(),
                        major_version_included: 1,
                    },
                    user: None,
                },
                relationships: CreateLicenseRelationships {
                    policy: PolicyRelationship {
                        data: PolicyRef {
                            resource_type: "policies",
                            id: policy_id,
                        },
                    },
                },
            },
        };

        let response = self
            .authorized_post(&url)
            .json(&payload)
            .send()
            .await
            .context("keygen create license request failed")?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            bail!("keygen create license failed ({status}): {body}");
        }

        let parsed: KeygenSingleResponse<KeygenLicenseAttributes> =
            serde_json::from_str(&body).context(format!("invalid keygen create license response: {body}"))?;
        let mut summary = map_license(parsed.data);
        summary.kind = kind.to_string();
        Ok(summary)
    }

    fn authorized_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", self.config.license_token))
            .header(CONTENT_TYPE, "application/vnd.api+json")
    }

    fn authorized_post(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {}", self.config.license_token))
            .header(CONTENT_TYPE, "application/vnd.api+json")
    }

    fn authorized_delete(&self, url: &str) -> reqwest::RequestBuilder {
        self.http
            .delete(url)
            .header(AUTHORIZATION, format!("Bearer {}", self.config.license_token))
            .header(CONTENT_TYPE, "application/vnd.api+json")
    }
}

fn map_license(item: KeygenResource<KeygenLicenseAttributes>) -> LicenseSummary {
    let kind = item
        .attributes
        .metadata
        .as_ref()
        .and_then(|m| m.get("product"))
        .and_then(|v| v.as_str())
        .unwrap_or("desktop")
        .to_string();
    LicenseSummary {
        id: item.id,
        key: item.attributes.key,
        status: item.attributes.status,
        kind,
        max_machines: item.attributes.max_machines,
        machines_count: item.attributes.machines_count,
        metadata: item.attributes.metadata.unwrap_or(serde_json::json!({})),
        created_at: item.attributes.created,
    }
}

fn map_machine(item: KeygenResource<KeygenMachineAttributes>) -> LicenseMachine {
    LicenseMachine {
        id: item.id,
        fingerprint: item.attributes.fingerprint,
        name: item.attributes.name,
        platform: item.attributes.platform,
        last_heartbeat_at: item.attributes.heartbeat,
        created_at: item.attributes.created,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_license_extracts_fields() {
        let item = KeygenResource {
            id: "lic-1".into(),
            resource_type: "licenses".into(),
            attributes: KeygenLicenseAttributes {
                key: "AVRG-TEST".into(),
                status: "ACTIVE".into(),
                metadata: Some(serde_json::json!({"user_id":"u1","product":"desktop"})),
                max_machines: Some(3),
                machines_count: Some(1),
                created: Some("2026-07-08".into()),
            },
        };
        let summary = map_license(item);
        assert_eq!(summary.key, "AVRG-TEST");
        assert_eq!(summary.max_machines, Some(3));
    }
}
