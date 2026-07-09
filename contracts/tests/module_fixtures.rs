use contracts::{HealthResponse, NotebookListResponse, PlansResponse};

#[test]
fn notebook_list_minimal_fixture_roundtrips() {
    let legacy = serde_json::json!({
        "notebooks": [{
            "id": "nb-1",
            "org_id": "org-1",
            "owner_id": "user-1",
            "name": "demo",
            "title": "Demo",
            "description": "",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "document_count": 0,
            "status_summary": {},
            "shared": false
        }]
    });

    let parsed: NotebookListResponse =
        serde_json::from_value(legacy).expect("legacy notebooks key should deserialize");
    assert_eq!(parsed.notebooks.len(), 1);
    assert_eq!(parsed.notebooks[0].name, "demo");

    let serialized = serde_json::to_value(parsed).expect("notebook list should serialize");
    assert!(
        serialized.get("workspaces").is_some(),
        "product wire key is workspaces"
    );
    assert!(serialized.get("notebooks").is_none());

    let product = serde_json::json!({
        "workspaces": [{
            "id": "nb-1",
            "org_id": "org-1",
            "owner_id": "user-1",
            "name": "demo",
            "title": "Demo",
            "description": "",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "document_count": 0,
            "status_summary": {},
            "shared": false
        }]
    });
    let parsed2: NotebookListResponse =
        serde_json::from_value(product).expect("workspaces key should deserialize");
    assert_eq!(parsed2.notebooks[0].id, "nb-1");
}

#[test]
fn billing_plans_minimal_fixture_roundtrips() {
    let json = serde_json::json!({
        "plans": [{
            "id": "free",
            "name": "Free",
            "price": 0,
            "features": ["base"]
        }]
    });

    let parsed: PlansResponse =
        serde_json::from_value(json.clone()).expect("plans response should deserialize");
    assert_eq!(parsed.plans.len(), 1);
    assert_eq!(parsed.plans[0].id, "free");

    let serialized = serde_json::to_value(parsed).expect("plans response should serialize");
    assert_eq!(serialized, json);
}

#[test]
fn admin_health_minimal_fixture_roundtrips() {
    let json = serde_json::json!({
        "status": "ok",
        "service": "avrag-api",
        "version": "0.1.0"
    });

    let parsed: HealthResponse =
        serde_json::from_value(json.clone()).expect("health response should deserialize");
    assert_eq!(parsed.status, "ok");
    assert_eq!(parsed.service, "avrag-api");

    let serialized = serde_json::to_value(parsed).expect("health response should serialize");
    assert_eq!(serialized, json);
}
