use web_sdk::{
    AdminAuditLogQuery, AdminBroadcastRequest, BrowserRestClient, TransportError,
    admin_account_url, admin_accounts_url, admin_audit_logs_url, admin_billing_block_url,
    admin_billing_url, admin_broadcast_url, admin_degradation_url, admin_feature_flags_url,
    admin_feature_flag_change_request_create_url, admin_feature_flag_change_request_review_url,
    admin_feature_flag_change_requests_url, admin_health_url, admin_rag_health_url,
    admin_usage_url, admin_user_url, admin_users_url, admin_workers_url, parse_admin_account,
    parse_admin_accounts, parse_admin_audit_logs, parse_admin_billing, parse_admin_broadcast,
    parse_admin_degradation, parse_admin_feature_flag_change_request,
    parse_admin_feature_flag_change_requests, parse_admin_feature_flags, parse_admin_health,
    parse_admin_rag_health, parse_admin_usage, parse_admin_users, parse_admin_workers,
};

#[test]
fn admin_urls_are_canonical() {
    assert_eq!(admin_accounts_url("http://x/", 1, 50), "http://x/api/v1/admin/accounts?page=1&per_page=50");
    assert_eq!(admin_accounts_url("http://x", 0, 0), "http://x/api/v1/admin/accounts");
    assert_eq!(admin_account_url("", "owner-1"), "/api/v1/admin/accounts/owner-1");
    assert_eq!(admin_users_url("http://x", "ow 1"), "http://x/api/v1/admin/users?owner_user_id=ow%201");
    assert_eq!(admin_user_url("http://x", "u-1"), "http://x/api/v1/admin/users/u-1");
    assert_eq!(admin_usage_url("http://x", "ow-1", "30d"), "http://x/api/v1/admin/usage?owner_user_id=ow-1&period=30d");
    assert_eq!(admin_health_url(""), "/api/v1/admin/health");
    assert_eq!(admin_billing_url("http://x"), "http://x/api/v1/admin/billing");
    assert_eq!(admin_billing_block_url("http://x"), "http://x/api/v1/admin/billing/block");
    assert_eq!(admin_rag_health_url("http://x"), "http://x/api/v1/admin/rag-health");
    assert_eq!(admin_workers_url("http://x"), "http://x/api/v1/admin/system/workers");
    assert_eq!(admin_degradation_url("http://x"), "http://x/api/v1/admin/system/degradation");
    assert_eq!(admin_feature_flags_url("http://x"), "http://x/api/v1/admin/feature-flags");
    assert_eq!(admin_feature_flag_change_requests_url("http://x", Some("pending")), "http://x/api/v1/admin/feature-flags/change-requests?status=pending");
    assert_eq!(admin_feature_flag_change_requests_url("http://x", None), "http://x/api/v1/admin/feature-flags/change-requests");
    assert_eq!(admin_feature_flag_change_request_create_url("http://x", "rag.offline"), "http://x/api/v1/admin/feature-flags/rag.offline/change-requests");
    assert_eq!(admin_feature_flag_change_request_review_url("http://x", "req-9"), "http://x/api/v1/admin/feature-flags/change-requests/req-9/review");
    assert_eq!(admin_broadcast_url("http://x"), "http://x/api/v1/admin/notifications/broadcast");
    assert_eq!(
        admin_audit_logs_url("http://x", &AdminAuditLogQuery { page: 2, per_page: 50, ..Default::default() }),
        "http://x/api/v1/admin/audit-logs?page=2&per_page=50"
    );
}

#[test]
fn parse_admin_envelope_payloads() {
    let accounts_raw = r#"{"data":[{"id":"ow-1","name":" acme ","created_at":1720000000,"blocked":false,"user_count":3,"document_count":12,"query_count":99}],"ok":true}"#;
    let accounts = parse_admin_accounts(accounts_raw.as_bytes()).expect("accounts");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].id, "ow-1");
    assert_eq!(accounts[0].document_count, 12);

    let account_raw = r#"{"data":{"id":"ow-1","name":"acme","created_at":1720000000,"blocked":true,"user_count":3,"document_count":12,"query_count":99},"ok":true}"#;
    let account = parse_admin_account(account_raw.as_bytes()).expect("account");
    assert!(account.blocked);

    let users_raw = r#"{"data":[{"id":"u-1","email":"a@x.com","role":"super_admin","created_at":1720000000}],"ok":true}"#;
    let users = parse_admin_users(users_raw.as_bytes()).expect("users");
    assert_eq!(users[0].role, "super_admin");

    let usage_raw = r#"{"data":{"owner_user_id":"ow-1","period":"30d","query_count":10,"document_count":4,"chunk_count":40,"storage_bytes":1024},"ok":true}"#;
    let usage = parse_admin_usage(usage_raw.as_bytes()).expect("usage");
    assert_eq!(usage.chunk_count, 40);

    let health_raw = r#"{"data":{"status":"ok","version":"0.4.2","uptime_secs":3600},"ok":true}"#;
    let health = parse_admin_health(health_raw.as_bytes()).expect("health");
    assert_eq!(health.status, "ok");

    let billing_raw = r#"{"data":{"active_subscriptions":5,"past_due_subscriptions":1,"unpaid_subscriptions":0,"canceled_subscriptions":2},"ok":true}"#;
    let billing = parse_admin_billing(billing_raw.as_bytes()).expect("billing");
    assert_eq!(billing.active_subscriptions, 5);

    let rag_raw = r#"{"data":{"failed_documents":1,"queued_tasks":2,"processing_tasks":3,"dead_letter_tasks":0,"recent_guard_events":4},"ok":true}"#;
    let rag = parse_admin_rag_health(rag_raw.as_bytes()).expect("rag");
    assert_eq!(rag.dead_letter_tasks, 0);

    let workers_raw = r#"{"data":{"runtime_mode":"standalone","queued_tasks":2,"processing_tasks":1,"dead_letter_tasks":0,"failed_documents":3},"ok":true}"#;
    let workers = parse_admin_workers(workers_raw.as_bytes()).expect("workers");
    assert_eq!(workers.runtime_mode, "standalone");

    let degradation_raw = r#"{"data":{"failed_documents":0,"recent_guard_events":1,"share_access_events":7},"ok":true}"#;
    let degradation = parse_admin_degradation(degradation_raw.as_bytes()).expect("degradation");
    assert_eq!(degradation.share_access_events, 7);

    let flags_raw = r#"{"data":[{"key":"rag.offline","category":"rag","description":"离线检索","enabled":true,"effective_enabled":true,"config_ready":true,"requires_config":false,"source":"default","updated_at":null,"has_pending_request":false}],"ok":true}"#;
    let flags = parse_admin_feature_flags(flags_raw.as_bytes()).expect("flags");
    assert_eq!(flags[0].key, "rag.offline");

    let requests_raw = r#"{"data":[{"id":"req-1","flag_key":"rag.offline","current_enabled":false,"requested_enabled":true,"reason":"上线灰度","status":"pending","requested_by":"ow-1","reviewed_by":null,"review_note":null,"created_at":1720000000,"reviewed_at":null,"executed_at":null}],"ok":true}"#;
    let requests = parse_admin_feature_flag_change_requests(requests_raw.as_bytes()).expect("requests");
    assert_eq!(requests[0].status, "pending");

    let request_raw = r#"{"data":{"id":"req-1","flag_key":"rag.offline","current_enabled":false,"requested_enabled":true,"reason":"上线灰度","status":"approved","requested_by":"ow-1","reviewed_by":"adm-1","review_note":"同意","created_at":1720000000,"reviewed_at":1720000100,"executed_at":null},"ok":true}"#;
    let request = parse_admin_feature_flag_change_request(request_raw.as_bytes()).expect("request");
    assert_eq!(request.status, "approved");
    assert_eq!(request.review_note.as_deref(), Some("同意"));

    let broadcast_raw = r#"{"data":{"created":42},"ok":true}"#;
    let broadcast = parse_admin_broadcast(broadcast_raw.as_bytes()).expect("broadcast");
    assert_eq!(broadcast.created, 42);
}

#[test]
fn parse_admin_audit_log_page_envelope() {
    let raw = r#"{"data":{"items":[{"id":7,"actor_id":"adm-1","action":"account.block","resource_type":"account","resource_id":"ow-1","owner_user_id":null,"created_at":1720000000}],"total":101,"page":3,"per_page":20},"ok":true}"#;
    let page = parse_admin_audit_logs(raw.as_bytes()).expect("audit page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.total, 101);
    assert_eq!(page.page, 3);
    assert_eq!(page.per_page, 20);
    assert!(page.items[0].owner_user_id.is_none());
}

#[test]
fn parse_admin_error_envelope_maps_to_http_status() {
    let raw = br#"{"data":null,"error":{"code":"admin_access_denied","message":"admin access denied"},"ok":false}"#;
    let err = parse_admin_accounts(raw).unwrap_err();
    match err {
        TransportError::HttpStatus { status, body } => {
            assert_eq!(status, 0);
            assert!(body.contains("admin_access_denied"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn native_admin_methods_are_unavailable() {
    let client = BrowserRestClient::new("http://x", Some("tok".to_string()));
    let query = AdminAuditLogQuery::default();
    let req = AdminBroadcastRequest {
        event_type: None,
        title: "t".to_string(),
        body: "b".to_string(),
        data: None,
    };
    assert!(matches!(client.list_admin_accounts(1, 50).await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_account("ow").await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.list_admin_users("ow").await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.delete_admin_user("u").await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_usage("ow", "30d").await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.block_admin_account("ow", true).await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_health().await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_billing_overview().await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_rag_health().await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_worker_status().await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_degradation_status().await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.list_admin_feature_flags().await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.list_admin_feature_flag_change_requests(None).await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.create_admin_feature_flag_change_request("k", true, "r").await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.review_admin_feature_flag_change_request("id", true, None).await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.list_admin_audit_logs(&query).await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.get_admin_audit_logs_csv(&query).await.unwrap_err(), TransportError::Unavailable(_)));
    assert!(matches!(client.broadcast_admin_notification(&req).await.unwrap_err(), TransportError::Unavailable(_)));
}
