use super::support::*;

#[tokio::test]
async fn search_conversation_history_notebook_scope_spans_sessions_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_id = OrgId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));

    let notebook = repo
        .bootstrap().create_notebook(&ctx, "memory-search", "memory search test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();

    let session_a = repo
        .sessions().create_session(&ctx, workspace_id, Some("session-a"), "rag")
        .await
        .unwrap();
    let session_a_id = Uuid::parse_str(&session_a.id).unwrap();
    let session_b = repo
        .sessions().create_session(&ctx, workspace_id, Some("session-b"), "rag")
        .await
        .unwrap();
    let session_b_id = Uuid::parse_str(&session_b.id).unwrap();

    repo.sessions().append_chat_turn(
        &ctx,
        session_a_id,
        &ChatTurn {
            user_content: "What is antifragility?",
            assistant_content: "Antifragility gains from disorder.",
            assistant_answer_blocks: &[],
            agent_type: "rag",
            citations: &[],
            tool_results: &[],
            user_turn_metadata: None,
            user_resolved_query: Some("What is antifragility?"),
        },
    )
    .await
    .unwrap();

    repo.sessions().append_chat_turn(
        &ctx,
        session_b_id,
        &ChatTurn {
            user_content: "Open a second session.",
            assistant_content: "Sure.",
            assistant_answer_blocks: &[],
            agent_type: "rag",
            citations: &[],
            tool_results: &[],
            user_turn_metadata: None,
            user_resolved_query: None,
        },
    )
    .await
    .unwrap();

    let tokens_row: Option<(Option<String>,)> = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.into_uuid().to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        let row = sqlx::query_as(
            "SELECT search_tokens FROM chat_messages WHERE session_id = $1 AND role = 'user' ORDER BY id DESC LIMIT 1",
        )
        .bind(session_a_id)
        .fetch_optional(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        row
    };
    assert!(
        tokens_row
            .and_then(|(t,)| t)
            .is_some_and(|t| !t.trim().is_empty()),
        "search_tokens should be populated on insert"
    );

    let hits = repo
        .conversation_memory().search_conversation_history(
            &ctx,
            session_b_id,
            "antifragility",
            ConversationHistoryScope::Notebook,
            10,
            &[],
        )
        .await
        .unwrap();

    assert!(
        hits.iter().any(|hit| hit.session_id == session_a_id),
        "notebook scope should return messages from another session in the same notebook"
    );
}

#[tokio::test]
async fn search_sessions_matches_assistant_message_body_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_id = OrgId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));

    let notebook = repo
        .bootstrap().create_notebook(&ctx, "session-search", "session search test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();

    let session = repo
        .sessions().create_session(&ctx, workspace_id, Some("generic title"), "rag")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();

    repo.sessions().append_chat_turn(
        &ctx,
        session_id,
        &ChatTurn {
            user_content: "Tell me something.",
            assistant_content: "The secret roadmap keyword is zephyrneedle2026.",
            assistant_answer_blocks: &[],
            agent_type: "rag",
            citations: &[],
            tool_results: &[],
            user_turn_metadata: None,
            user_resolved_query: None,
        },
    )
    .await
    .unwrap();

    let pattern = "%zephyrneedle2026%";
    let matches = repo.chunks().search_sessions(&ctx, pattern).await.unwrap();
    assert!(
        matches.iter().any(|item| item.id == session.id),
        "search_sessions should match assistant message FTS, not only session title"
    );
}

#[tokio::test]
async fn search_conversation_history_matches_assistant_message_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_id = OrgId::from(Uuid::new_v4());
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(org_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));

    let notebook = repo
        .bootstrap().create_notebook(&ctx, "assistant-history", "assistant history test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();

    let session_a = repo
        .sessions().create_session(&ctx, workspace_id, Some("session-a"), "rag")
        .await
        .unwrap();
    let session_a_id = Uuid::parse_str(&session_a.id).unwrap();
    let session_b = repo
        .sessions().create_session(&ctx, workspace_id, Some("session-b"), "rag")
        .await
        .unwrap();
    let session_b_id = Uuid::parse_str(&session_b.id).unwrap();

    repo.sessions().append_chat_turn(
        &ctx,
        session_a_id,
        &ChatTurn {
            user_content: "Explain a concept.",
            assistant_content: "Antifragility gains from volatility and stressors.",
            assistant_answer_blocks: &[],
            agent_type: "rag",
            citations: &[],
            tool_results: &[],
            user_turn_metadata: None,
            user_resolved_query: None,
        },
    )
    .await
    .unwrap();

    repo.sessions().append_chat_turn(
        &ctx,
        session_b_id,
        &ChatTurn {
            user_content: "Another topic.",
            assistant_content: "Sure.",
            assistant_answer_blocks: &[],
            agent_type: "rag",
            citations: &[],
            tool_results: &[],
            user_turn_metadata: None,
            user_resolved_query: None,
        },
    )
    .await
    .unwrap();

    let hits = repo
        .conversation_memory().search_conversation_history(
            &ctx,
            session_b_id,
            "antifragility",
            ConversationHistoryScope::Notebook,
            10,
            &[],
        )
        .await
        .unwrap();

    assert!(
        hits.iter().any(|hit| hit.session_id == session_a_id && hit.role == "assistant"),
        "conversation_history should match assistant message body across sessions"
    );
}

