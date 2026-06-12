//! HTTP auth headers and per-test identity helpers for product E2E.

/// Default test org/user IDs for stable identity tests.
pub const DEFAULT_TEST_ORG_ID: &str = "00000000-0000-0000-0000-000000000001";
pub const DEFAULT_TEST_USER_ID: &str = "00000000-0000-0000-0000-000000000001";

/// Unique org/user pair so parallel tests do not share rate-limit buckets.
pub fn unique_test_identity() -> (String, String) {
    use uuid::Uuid;
    (Uuid::new_v4().to_string(), Uuid::new_v4().to_string())
}

pub fn milvus_collection_prefix_for_identity(org_id: &str) -> String {
    let suffix = org_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("avrag_e2e_{suffix}")
}

pub fn test_auth_headers_for(org_id: &str, user_id: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-org-id", org_id.parse().unwrap());
    headers.insert("x-user-id", user_id.parse().unwrap());
    headers.insert("x-permissions", "external_network".parse().unwrap());
    headers
}
