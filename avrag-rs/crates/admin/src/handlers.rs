use common::ApiResponse;

use crate::models::HealthStatus;
use crate::service::AdminService;

pub async fn handle_health() -> ApiResponse<HealthStatus> {
    ApiResponse::ok(AdminService::get_health().await)
}
