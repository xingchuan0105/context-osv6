use crate::conversation_api::{encode_path_segment, trim_base_url};
use crate::transport::TransportError;

pub use contracts::auth::{NotificationRow, NotificationsResponse};

pub fn notifications_url(base_url: &str) -> String {
    format!("{}/api/v1/notifications", trim_base_url(base_url))
}

pub fn notification_read_url(base_url: &str, notification_id: &str) -> String {
    format!(
        "{}/api/v1/notifications/{}/read",
        trim_base_url(base_url),
        encode_path_segment(notification_id)
    )
}

pub fn parse_notifications(body: &[u8]) -> Result<NotificationsResponse, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}
