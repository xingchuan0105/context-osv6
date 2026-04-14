//! Admin API client
#![allow(dead_code)]

use crate::{ApiClient, dtos::*};
use anyhow::{anyhow, bail};
use serde::Deserialize;
use urlencoding::encode;

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[serde(default)]
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorEnvelope>,
}

#[derive(Debug, Deserialize)]
struct RawOrgRow {
    id: String,
    name: String,
    created_at: i64,
    blocked: bool,
    user_count: i64,
    document_count: i64,
    query_count: i64,
}

#[derive(Debug, Deserialize)]
struct RawUserRow {
    id: String,
    email: String,
    org_id: String,
    role: String,
    created_at: i64,
}

#[derive(Debug, Deserialize)]
struct RawUsageResponse {
    org_id: String,
    period: String,
    query_count: i64,
    document_count: i64,
    chunk_count: i64,
    storage_bytes: i64,
}

#[derive(Debug, Deserialize)]
struct RawHealthResponse {
    status: String,
    version: String,
    uptime_secs: i64,
}

fn unwrap_api_data<T>(envelope: ApiEnvelope<T>, fallback: &str) -> anyhow::Result<T> {
    if envelope.ok {
        return envelope
            .data
            .ok_or_else(|| anyhow!("missing data in API response"));
    }

    let message = envelope
        .error
        .map(|err| err.message)
        .unwrap_or_else(|| fallback.to_string());
    bail!(message)
}

fn feature_flag_change_requests_path(status: Option<&str>) -> String {
    status
        .map(|value| format!("/api/v1/admin/feature-flags/change-requests?status={value}"))
        .unwrap_or_else(|| "/api/v1/admin/feature-flags/change-requests".to_string())
}

fn map_org_row(raw: RawOrgRow) -> OrgRow {
    OrgRow {
        id: raw.id,
        name: raw.name,
        plan: "N/A".to_string(),
        user_count: raw.user_count,
        notebook_count: raw.document_count,
        query_count: raw.query_count,
        blocked: raw.blocked,
        created_at: raw.created_at.to_string(),
    }
}

impl ApiClient {
    /// GET /api/v1/admin/organizations
    pub async fn list_orgs(&self) -> anyhow::Result<OrgListResponse> {
        let envelope: ApiEnvelope<Vec<RawOrgRow>> = self.get("/api/v1/admin/organizations").await?;
        let orgs = unwrap_api_data(envelope, "failed to load organizations")?
            .into_iter()
            .map(map_org_row)
            .collect();
        Ok(OrgListResponse { orgs })
    }

    /// GET /api/v1/admin/organizations/{org_id}
    pub async fn get_org(&self, org_id: &str) -> anyhow::Result<OrgResponse> {
        let envelope: ApiEnvelope<RawOrgRow> = self
            .get(&format!("/api/v1/admin/organizations/{}", org_id))
            .await?;
        let org = map_org_row(unwrap_api_data(envelope, "failed to load organization")?);
        Ok(OrgResponse { org })
    }

    /// GET /api/v1/admin/users
    ///
    /// The backend requires `org_id`. Callers must choose an organization
    /// explicitly instead of relying on an arbitrary default.
    pub async fn list_users(&self) -> anyhow::Result<UserListResponse> {
        bail!("admin users endpoint requires org_id; use list_users_for_org")
    }

    pub async fn list_users_for_org(&self, org_id: &str) -> anyhow::Result<UserListResponse> {
        let envelope: ApiEnvelope<Vec<RawUserRow>> = self
            .get(&format!("/api/v1/admin/users?org_id={org_id}"))
            .await?;
        let users = unwrap_api_data(envelope, "failed to load users")?
            .into_iter()
            .map(|raw| UserRow {
                id: raw.id,
                email: raw.email,
                full_name: String::new(),
                org_id: raw.org_id,
                role: raw.role,
                created_at: raw.created_at.to_string(),
                last_active_at: None,
            })
            .collect();
        Ok(UserListResponse { users })
    }

    /// GET /api/v1/admin/usage
    ///
    /// The backend requires `org_id`. Callers that need an aggregate view
    /// should fetch per-org usage and combine the results client-side.
    pub async fn get_admin_usage(&self) -> anyhow::Result<AdminUsageResponse> {
        bail!("admin usage endpoint requires org_id; use get_admin_usage_for_org")
    }

    pub async fn get_admin_usage_for_org(
        &self,
        org_id: &str,
    ) -> anyhow::Result<AdminUsageResponse> {
        self.get_admin_usage_for_org_with_period(org_id, "30d")
            .await
    }

    pub async fn get_admin_usage_for_org_with_period(
        &self,
        org_id: &str,
        period: &str,
    ) -> anyhow::Result<AdminUsageResponse> {
        let envelope: ApiEnvelope<RawUsageResponse> = self
            .get(&format!(
                "/api/v1/admin/usage?org_id={org_id}&period={period}"
            ))
            .await?;
        let usage = unwrap_api_data(envelope, "failed to load usage")?;
        Ok(AdminUsageResponse {
            total_requests: usage.query_count,
            total_tokens: usage.chunk_count,
            total_documents: usage.document_count,
        })
    }

    /// POST /api/v1/admin/billing/block
    pub async fn block_org(&self, org_id: &str, blocked: bool) -> anyhow::Result<EmptyResponse> {
        #[derive(serde::Serialize)]
        struct Body {
            org_id: String,
            blocked: bool,
        }
        let _envelope: ApiEnvelope<serde_json::Value> = self
            .post(
                "/api/v1/admin/billing/block",
                &Body {
                    org_id: org_id.to_string(),
                    blocked,
                },
            )
            .await?;
        Ok(EmptyResponse {})
    }

    /// GET /api/v1/admin/health
    pub async fn get_health(&self) -> anyhow::Result<HealthResponse> {
        let envelope: ApiEnvelope<RawHealthResponse> = self.get("/api/v1/admin/health").await?;
        let health = unwrap_api_data(envelope, "failed to load health")?;
        Ok(HealthResponse {
            status: health.status,
            service: "avrag-api".to_string(),
            version: health.version,
        })
    }

    pub async fn list_feature_flags(&self) -> anyhow::Result<Vec<FeatureFlagEntry>> {
        let envelope: ApiEnvelope<Vec<FeatureFlagEntry>> =
            self.get("/api/v1/admin/feature-flags").await?;
        unwrap_api_data(envelope, "failed to load feature flags")
    }

    pub async fn set_feature_flag(
        &self,
        key: &str,
        enabled: bool,
    ) -> anyhow::Result<FeatureFlagEntry> {
        #[derive(serde::Serialize)]
        struct Body {
            enabled: bool,
        }
        let envelope: ApiEnvelope<FeatureFlagEntry> = self
            .put(
                &format!("/api/v1/admin/feature-flags/{}", key),
                &Body { enabled },
            )
            .await?;
        unwrap_api_data(envelope, "failed to update feature flag")
    }

    pub async fn list_feature_flag_change_requests(
        &self,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<FeatureFlagChangeRequest>> {
        let path = feature_flag_change_requests_path(status);
        let envelope: ApiEnvelope<Vec<FeatureFlagChangeRequest>> = self.get(&path).await?;
        unwrap_api_data(envelope, "failed to load feature flag change requests")
    }

    pub async fn request_feature_flag_change(
        &self,
        key: &str,
        enabled: bool,
        reason: &str,
    ) -> anyhow::Result<FeatureFlagChangeRequest> {
        #[derive(serde::Serialize)]
        struct Body {
            enabled: bool,
            reason: String,
        }
        let envelope: ApiEnvelope<FeatureFlagChangeRequest> = self
            .post(
                &format!("/api/v1/admin/feature-flags/{key}/change-requests"),
                &Body {
                    enabled,
                    reason: reason.to_string(),
                },
            )
            .await?;
        unwrap_api_data(envelope, "failed to request feature flag change")
    }

    pub async fn review_feature_flag_change(
        &self,
        request_id: &str,
        approved: bool,
        review_note: Option<&str>,
    ) -> anyhow::Result<FeatureFlagChangeRequest> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            approved: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            review_note: Option<&'a str>,
        }
        let envelope: ApiEnvelope<FeatureFlagChangeRequest> = self
            .post(
                &format!("/api/v1/admin/feature-flags/change-requests/{request_id}/review"),
                &Body {
                    approved,
                    review_note,
                },
            )
            .await?;
        unwrap_api_data(envelope, "failed to review feature flag change")
    }

    pub async fn get_worker_status(&self) -> anyhow::Result<WorkerStatusResponse> {
        let envelope: ApiEnvelope<WorkerStatusResponse> =
            self.get("/api/v1/admin/system/workers").await?;
        unwrap_api_data(envelope, "failed to load worker status")
    }

    pub async fn get_degradation_status(&self) -> anyhow::Result<DegradationStatusResponse> {
        let envelope: ApiEnvelope<DegradationStatusResponse> =
            self.get("/api/v1/admin/system/degradation").await?;
        unwrap_api_data(envelope, "failed to load degradation status")
    }

    pub async fn list_audit_logs(
        &self,
        query: &AuditLogQuery,
    ) -> anyhow::Result<AuditLogListResponse> {
        let mut pairs = Vec::<String>::new();
        if let Some(value) = query
            .query
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("q={}", encode(value.trim())));
        }
        if let Some(value) = query
            .action
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("action={}", encode(value.trim())));
        }
        if let Some(value) = query
            .resource_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("resource_type={}", encode(value.trim())));
        }
        if let Some(value) = query
            .actor
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("actor={}", encode(value.trim())));
        }
        if let Some(value) = query
            .window
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("window={}", encode(value.trim())));
        }
        if let Some(value) = query.page {
            pairs.push(format!("page={value}"));
        }
        if let Some(value) = query.per_page {
            pairs.push(format!("per_page={value}"));
        }

        let path = if pairs.is_empty() {
            "/api/v1/admin/audit-logs".to_string()
        } else {
            format!("/api/v1/admin/audit-logs?{}", pairs.join("&"))
        };
        let envelope: ApiEnvelope<AuditLogListResponse> = self.get(&path).await?;
        unwrap_api_data(envelope, "failed to load audit logs")
    }

    pub async fn export_audit_logs_csv(&self, query: &AuditLogQuery) -> anyhow::Result<String> {
        let mut pairs = Vec::<String>::new();
        if let Some(value) = query
            .query
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("q={}", encode(value.trim())));
        }
        if let Some(value) = query
            .action
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("action={}", encode(value.trim())));
        }
        if let Some(value) = query
            .resource_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("resource_type={}", encode(value.trim())));
        }
        if let Some(value) = query
            .actor
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("actor={}", encode(value.trim())));
        }
        if let Some(value) = query
            .window
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            pairs.push(format!("window={}", encode(value.trim())));
        }
        pairs.push("format=csv".to_string());
        let path = format!("/api/v1/admin/audit-logs?{}", pairs.join("&"));
        #[cfg(target_arch = "wasm32")]
        {
            let bytes = self
                .send_wasm_request("GET", &path, Option::<&()>::None)
                .await?;
            return Ok(String::from_utf8(bytes)?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = format!("{}{}", self.base_url, path);
            let mut req = self.client.get(&url);
            if let Some(ref token) = self.auth_header() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            let resp = req.send().await?;
            if !resp.status().is_success() {
                anyhow::bail!("API error: {}", resp.status());
            }
            return Ok(resp.text().await?);
        }
    }

    pub async fn get_billing_overview(&self) -> anyhow::Result<BillingOverview> {
        let envelope: ApiEnvelope<BillingOverview> = self.get("/api/v1/admin/billing").await?;
        unwrap_api_data(envelope, "failed to load billing overview")
    }

    pub async fn get_rag_health(&self) -> anyhow::Result<RagHealthStatus> {
        let envelope: ApiEnvelope<RagHealthStatus> = self.get("/api/v1/admin/rag-health").await?;
        unwrap_api_data(envelope, "failed to load rag health")
    }
}
