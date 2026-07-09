use avrag_licensing::KeygenClient;
use common::ApiResponse;

#[test]
fn licensing_config_disabled_without_env() {
    // Avoid mutating process env in parallel tests; just assert from_env needs vars.
    if std::env::var("KEYGEN_ACCOUNT_ID").is_err() {
        assert!(KeygenClient::from_env().is_err());
    }
}

#[test]
fn create_checkout_request_deserializes() {
    let raw = r#"{"plan_id":"desktop-standard","provider":"creem","device_id":"abc"}"#;
    let parsed: avrag_licensing::CreateLicenseCheckoutRequest =
        serde_json::from_str(raw).expect("deserialize");
    assert_eq!(parsed.plan_id, "desktop-standard");
    assert_eq!(parsed.device_id.as_deref(), Some("abc"));
}

#[test]
fn license_list_response_serializes() {
    let response = ApiResponse::ok(avrag_licensing::LicenseListResponse {
        licenses: vec![avrag_licensing::LicenseSummary {
            id: "lic-1".into(),
            key: "AVRG-TEST".into(),
            status: "ACTIVE".into(),
            kind: "pro".into(),
            max_machines: Some(3),
            machines_count: Some(1),
            metadata: serde_json::json!({}),
            created_at: None,
        }],
    });
    let json = serde_json::to_string(&response).expect("serialize");
    assert!(json.contains("AVRG-TEST"));
}
