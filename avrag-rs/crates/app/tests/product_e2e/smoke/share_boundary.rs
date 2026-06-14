//! Share-link collaboration boundary (mode A: cross-user read via token only).

use crate::product_e2e::{ChatResponse, TestContext, assertions::*};

const ORG_A: &str = "55555555-5555-5555-5555-555555555555";
const USER_A: &str = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee";
const ORG_B: &str = "66666666-6666-6666-6666-666666666666";
const USER_B: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

#[tokio::test]
async fn share_token_allows_cross_user_readonly_chat() {

    super::require_smoke_suite();
    let ctx_a = TestContext::new_smoke_with_org(ORG_A, USER_A).await;
    let notebook = ctx_a.create_notebook("shared-notebook").await.unwrap();
    let share_token = ctx_a.create_share_token(&notebook.id).await.unwrap();

    let ctx_b = TestContext::new_smoke_with_org(ORG_B, USER_B).await;
    let http_resp = ctx_b
        .chat_with_share("What is in this notebook?", &notebook.id, &share_token)
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_observability_contract(&resp);
    assert_answer_substantive(&resp, 5);
}

#[tokio::test]
async fn cross_user_direct_get_notebook_without_token_returns_4xx() {

    super::require_smoke_suite();
    let ctx_a = TestContext::new_smoke_with_org(ORG_A, USER_A).await;
    let notebook = ctx_a.create_notebook("private-notebook").await.unwrap();

    let ctx_b = TestContext::new_smoke_with_org(ORG_B, USER_B).await;
    let resp = ctx_b
        .http_client
        .get(format!(
            "{}/api/v1/notebooks/{}",
            ctx_b.base_url, notebook.id
        ))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    assert!(
        (400..500).contains(&status),
        "cross-user notebook GET should be 4xx, got HTTP {status}"
    );
}

#[tokio::test]
async fn share_chat_with_invalid_token_returns_401_or_403() {

    super::require_smoke_suite();
    let ctx_a = TestContext::new_smoke_with_org(ORG_A, USER_A).await;
    let notebook = ctx_a.create_notebook("bad-token-notebook").await.unwrap();

    let ctx_b = TestContext::new_smoke_with_org(ORG_B, USER_B).await;
    let http_resp = ctx_b
        .chat_with_share(
            "Hello",
            &notebook.id,
            "00000000-0000-0000-0000-000000000099",
        )
        .await
        .unwrap();

    let status = http_resp.status;
    assert!(
        (400..500).contains(&status),
        "invalid share token should be rejected with 4xx, got HTTP {status}"
    );
    assert_ne!(status, 200, "invalid share token must not succeed");
}

#[tokio::test]
async fn owner_can_invite_member_via_http() {
    super::require_smoke_suite();
    let ctx_a = TestContext::new_smoke_with_org(ORG_A, USER_A).await;
    let notebook = ctx_a.create_notebook("invite-notebook").await.unwrap();

    let invite_resp = ctx_a
        .invite_notebook_member(&notebook.id, "collaborator@example.test", "write")
        .await
        .expect("invite member");
    assert_eq!(invite_resp.status, 200, "invite should succeed");

    let members_body = ctx_a
        .list_notebook_members(&notebook.id)
        .await
        .expect("list members");
    let members = members_body["members"]
        .as_array()
        .expect("members array");
    assert!(
        members.iter().any(|m| {
            m.get("email").and_then(|v| v.as_str()) == Some("collaborator@example.test")
                && m.get("status").and_then(|v| v.as_str()) == Some("pending")
        }),
        "expected pending invite for collaborator@example.test, got {members_body}"
    );
}
