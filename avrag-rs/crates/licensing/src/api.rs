use common::{ApiResponse, UserId};
use serde::{Deserialize, Serialize};

use crate::client::KeygenClient;
use crate::types::{LicenseMachine, LicenseSummary};

#[derive(Debug, Deserialize)]
pub struct CreateLicenseCheckoutRequest {
    pub plan_id: String,
    pub provider: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateLicenseCheckoutResponse {
    pub checkout_url: String,
    pub session_id: String,
    pub plan_id: String,
}

#[derive(Debug, Serialize)]
pub struct LicenseCheckoutResponse {
    pub license: LicenseSummary,
    pub deep_link: String,
}

#[derive(Debug, Serialize)]
pub struct LicenseListResponse {
    pub licenses: Vec<LicenseSummary>,
}

#[derive(Debug, Serialize)]
pub struct LicenseMachineListResponse {
    pub machines: Vec<LicenseMachine>,
}

pub async fn handle_list_user_licenses(
    client: &KeygenClient,
    user_id: UserId,
) -> ApiResponse<LicenseListResponse> {
    match client.list_licenses_for_user(&user_id.to_string()).await {
        Ok(licenses) => ApiResponse::ok(LicenseListResponse { licenses }),
        Err(error) => ApiResponse::err("license_list_failed", &error.to_string()),
    }
}

pub async fn handle_list_license_machines(
    client: &KeygenClient,
    license_id: &str,
) -> ApiResponse<LicenseMachineListResponse> {
    match client.list_machines(license_id).await {
        Ok(machines) => ApiResponse::ok(LicenseMachineListResponse { machines }),
        Err(error) => ApiResponse::err("license_machines_failed", &error.to_string()),
    }
}

pub async fn handle_deactivate_machine(
    client: &KeygenClient,
    machine_id: &str,
) -> ApiResponse<serde_json::Value> {
    match client.deactivate_machine(machine_id).await {
        Ok(()) => ApiResponse::ok(serde_json::json!({ "deactivated": true })),
        Err(error) => ApiResponse::err("license_deactivate_failed", &error.to_string()),
    }
}

pub async fn handle_create_trial_license(
    client: &KeygenClient,
    user_id: UserId,
) -> ApiResponse<LicenseCheckoutResponse> {
    match client.create_trial_license(&user_id.to_string()).await {
        Ok(license) => {
            let deep_link = format!(
                "avrag-desktop://activate?key={}",
                urlencoding::encode(&license.key)
            );
            ApiResponse::ok(LicenseCheckoutResponse { license, deep_link })
        }
        Err(error) => ApiResponse::err("license_trial_failed", &error.to_string()),
    }
}

pub async fn handle_create_license_checkout(
    client: &KeygenClient,
    user_id: UserId,
    request: CreateLicenseCheckoutRequest,
) -> ApiResponse<CreateLicenseCheckoutResponse> {
    let provider = request.provider.as_deref().unwrap_or("creem");
    let success_url = format!(
        "{}/desktop/buy?success=1&plan_id={}&device_id={}",
        client.config().public_app_base_url,
        request.plan_id,
        request.device_id.unwrap_or_default()
    );

    // Checkout session creation is delegated to billing in WP6.
    // For WP2 we return a structured placeholder that the frontend can route to billing.
    ApiResponse::ok(CreateLicenseCheckoutResponse {
        checkout_url: format!(
            "{}/api/v1/billing/checkout-session?product=desktop&plan_id={}&provider={}&success_url={}",
            client.config().public_app_base_url,
            request.plan_id,
            provider,
            urlencoding::encode(&success_url)
        ),
        session_id: format!("desktop_{}_{}", user_id, uuid::Uuid::new_v4()),
        plan_id: request.plan_id,
    })
}
