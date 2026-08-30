//! Dual-tenant negative tests for the explicit `owner_user_id` conditions.
//!
//! Second isolation layer under RLS: B cannot list/search/read/modify/delete
//! A's workspace/document/session/message, cannot create children under A's
//! parents, and A's data is bit-identical after every rejection. Skips when
//! DATABASE_URL is unset (same convention as the other live-PG tests here).
use super::support::*;

fn ctx_for(user: UserId) -> AuthContext {
    AuthContext::new(user, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()))
}

async fn dual_tenant_repo() -> Option<PgAppRepository> {
    let Ok(database_url) = env::var("DATABASE_URL") else {
        return None;
    };
    if database_url.trim().is_empty() {
        return None;
    }
    migration_role_context();
    let bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: bootstrap.pool.clone(),
    };
    Some(repo)
}

#[tokio::test]
async fn cross_tenant_workspace_document_session_all_denied() {
    let Some(repo) = dual_tenant_repo().await else {
        return;
    };

    let owner_a = UserId::from(Uuid::new_v4());
    let user_b = UserId::from(Uuid::new_v4());
    let ctx_a = ctx_for(owner_a);
    let ctx_b = ctx_for(user_b);

    // A's tree: workspace → document → session → message.
    let ws = repo
        .bootstrap()
        .create_workspace(&ctx_a, "tenant-a-ws", "dual tenant")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&ws.id).unwrap();
    let doc = repo
        .bootstrap()
        .create_document(&ctx_a, workspace_id, "a-doc.md", 10, "text/markdown")
        .await
        .unwrap();
    let doc_id = Uuid::parse_str(&doc.id).unwrap();
    let session = repo
        .sessions()
        .create_session(&ctx_a, workspace_id, Some("a-session"), "chat")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();
    let turn = ChatTurn {
        user_content: "tenant-a question",
        assistant_content: "tenant-a answer",
        assistant_answer_blocks: &[],
        agent_type: "chat",
        citations: &[],
        tool_results: &[],
        user_turn_metadata: None,
        user_resolved_query: None,
        assistant_turn_metadata: None,
    };
    let message_id = repo
        .sessions()
        .append_chat_turn(&ctx_a, session_id, &turn)
        .await
        .unwrap();

    // ---- B reads: all empty/None ----
    assert!(
        repo.bootstrap()
            .get_workspace(&ctx_b, workspace_id)
            .await
            .unwrap()
            .is_none(),
        "B must not read A's workspace"
    );
    assert!(
        repo
            .list_documents(&ctx_b, Some(workspace_id), None)
            .await
            .unwrap()
            .iter()
            .all(|d| d.id != doc.id),
        "B must not list A's documents"
    );
    assert!(
        repo.sessions()
            .get_session(&ctx_b, session_id)
            .await
            .unwrap()
            .is_none(),
        "B must not read A's session by ID"
    );
    assert!(
        repo.sessions()
            .list_sessions(&ctx_b, None)
            .await
            .unwrap()
            .iter()
            .all(|s| s.id != session.id),
        "B must not see A's session via list"
    );
    assert!(
        repo.sessions()
            .get_message(&ctx_b, session_id, message_id)
            .await
            .unwrap()
            .is_none(),
        "B must not read A's message by ID"
    );
    assert!(
        repo.list_messages(&ctx_b, session_id).await.unwrap().is_empty(),
        "B must not list A's messages"
    );
    assert!(
        repo.chunks()
            .search_workspaces(&ctx_b, "%tenant-a%")
            .await
            .unwrap()
            .iter()
            .all(|w| w.id != ws.id),
        "B global search must not surface A's workspace"
    );
    assert!(
        repo.chunks()
            .search_sessions(&ctx_b, "%tenant-a%")
            .await
            .unwrap()
            .iter()
            .all(|s| s.id != session.id),
        "B global search must not surface A's session"
    );

    // ---- B writes under A's parents: refused (no child row anywhere) ----
    let child = repo
        .sessions()
        .create_session(&ctx_b, workspace_id, Some("b-under-a"), "chat")
        .await;
    assert!(
        child.is_err(),
        "B must not create a session under A's workspace"
    );
    let msg = repo
        .sessions()
        .append_chat_turn(&ctx_b, session_id, &turn)
        .await;
    assert!(msg.is_err(), "B must not append a turn to A's session");

    // ---- B mutations: no effect ----
    let updated = repo
        .bootstrap()
        .update_workspace(&ctx_b, workspace_id, "hijacked", "hijacked")
        .await
        .unwrap();
    assert!(updated.is_none(), "B must not update A's workspace");
    let deleted = repo
        .sessions()
        .delete_session(&ctx_b, session_id)
        .await
        .unwrap();
    assert!(!deleted, "B must not delete A's session");
    // Cross-tenant get-by-ID yields nothing.
    let doc_seen_by_b = repo
        .list_documents(&ctx_b, None, Some(doc_id))
        .await
        .unwrap();
    assert!(doc_seen_by_b.iter().all(|d| d.id != doc.id));

    // ---- A's data unchanged after all rejects ----
    let ws_after = repo
        .bootstrap()
        .get_workspace(&ctx_a, workspace_id)
        .await
        .unwrap()
        .expect("A's workspace survived");
    assert_eq!(ws_after.title, "tenant-a-ws");
    let sess_after = repo
        .sessions()
        .get_session(&ctx_a, session_id)
        .await
        .unwrap()
        .expect("A's session survived");
    assert_eq!(sess_after.title.as_deref(), Some("a-session"));
    let msgs = repo.list_messages(&ctx_a, session_id).await.unwrap();
    assert_eq!(msgs.len(), 2, "A's two messages intact");
}