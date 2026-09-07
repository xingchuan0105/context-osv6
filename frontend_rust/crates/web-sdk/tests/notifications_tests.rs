use web_sdk::{notification_read_url, notifications_url, parse_notifications};

#[test]
fn notification_urls_are_canonical() {
    assert_eq!(
        notifications_url("http://x/"),
        "http://x/api/v1/notifications"
    );
    assert_eq!(
        notification_read_url("http://x", "n 1"),
        "http://x/api/v1/notifications/n%201/read"
    );
}

#[test]
fn parse_notifications_list() {
    let body = br#"{"notifications":[{"id":"n1","owner_user_id":"o","user_id":"u","event_type":"info","title":"t","body":"b","data":{},"read_at":null,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}]}"#;
    let parsed = parse_notifications(body).expect("parse");
    assert_eq!(parsed.notifications.len(), 1);
    assert_eq!(parsed.notifications[0].id, "n1");
    assert!(parsed.notifications[0].read_at.is_none());
}
