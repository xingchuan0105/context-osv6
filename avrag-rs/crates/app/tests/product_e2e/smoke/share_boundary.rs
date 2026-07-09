//! Share-link collaboration boundary (mode A: cross-user read via token only).

use crate::product_e2e::test_context::local_dev_email;
use crate::product_e2e::{ChatResponse, TestContext, assertions::*};

const ORG_A: &str = "55555555-5555-5555-5555-555555555555";
const USER_OWNER: &str = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee";
const USER_COLLAB: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";
const USER_A: &str = USER_OWNER;
const USER_B: &str = USER_COLLAB;
const ORG_B: &str = "66666666-6666-6666-6666-666666666666";

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
            "{}/api/v1/workspaces/{}",
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
        .invite_notebook_member(&notebook.id, &local_dev_email(USER_COLLAB), "write")
        .await
        .expect("invite member");
    assert_eq!(invite_resp.status, 200, "invite should succeed");

    let members_body = ctx_a
        .list_workspace_members(&notebook.id)
        .await
        .expect("list members");
    let members = members_body["members"].as_array().expect("members array");
    assert!(
        members.iter().any(|m| {
            m.get("email").and_then(|v| v.as_str()) == Some(local_dev_email(USER_COLLAB).as_str())
                && m.get("status").and_then(|v| v.as_str()) == Some("pending")
        }),
        "expected pending invite for collaborator, got {members_body}"
    );
}

#[tokio::test]
async fn invited_member_can_accept_and_access_notebook() {
    super::require_smoke_suite();
    let ctx_owner = TestContext::new_smoke_with_org(ORG_A, USER_OWNER).await;
    let notebook = ctx_owner
        .create_notebook("invite-accept-notebook")
        .await
        .unwrap();

    let collab_email = local_dev_email(USER_COLLAB);
    let invite_resp = ctx_owner
        .invite_notebook_member(&notebook.id, &collab_email, "write")
        .await
        .expect("invite member");
    assert_eq!(invite_resp.status, 200);

    let members_body = ctx_owner
        .list_workspace_members(&notebook.id)
        .await
        .expect("list members");
    let member_id = members_body["members"]
        .as_array()
        .expect("members array")
        .iter()
        .find(|m| m.get("email").and_then(|v| v.as_str()) == Some(collab_email.as_str()))
        .and_then(|m| m.get("member_id").and_then(|v| v.as_str()))
        .expect("pending member row")
        .to_string();

    let ctx_collab = TestContext::new_smoke_with_org(ORG_A, USER_COLLAB).await;
    // Write path seeds `{USER_COLLAB}@local.dev` via ensure_org_and_actor (list is read-only).
    let _ = ctx_collab
        .create_notebook("collab-seed-notebook")
        .await
        .expect("seed collaborator user");

    let accept_resp = ctx_collab
        .accept_notebook_invite(&notebook.id, &member_id)
        .await
        .expect("accept invite");
    assert_eq!(accept_resp.status, 200, "accept invite: {accept_resp:?}");

    let access_resp = ctx_collab.get_notebook(&notebook.id).await.unwrap();
    assert_eq!(
        access_resp.status, 200,
        "accepted member should read notebook, got {access_resp:?}"
    );

    let members_after = ctx_owner
        .list_workspace_members(&notebook.id)
        .await
        .expect("list members after accept");
    assert!(
        members_after["members"]
            .as_array()
            .expect("members array")
            .iter()
            .any(|m| {
                m.get("member_id").and_then(|v| v.as_str()) == Some(member_id.as_str())
                    && m.get("status").and_then(|v| v.as_str()) == Some("accepted")
            }),
        "member should be accepted, got {members_after}"
    );
}
