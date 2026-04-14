//! Notifications API client

use crate::{ApiClient, dtos::*};

impl ApiClient {
    /// GET /api/v1/notifications
    pub async fn list_notifications(&self) -> anyhow::Result<NotificationsResponse> {
        self.get("/api/v1/notifications").await
    }

    /// POST /api/v1/notifications/{notification_id}/read
    pub async fn mark_notification_read(
        &self,
        notification_id: &str,
    ) -> anyhow::Result<EmptyResponse> {
        self.post(
            &format!("/api/v1/notifications/{}/read", notification_id),
            &EmptyResponse {},
        )
        .await
    }
}
