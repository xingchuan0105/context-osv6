use super::support::*;

#[tokio::test]
async fn chat_message_tool_results_roundtrip_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let _org_uuid = owner_user_id.into_uuid();
    let user_id = Uuid::new_v4();
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(user_id));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "tool-results-test", "tool results test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();

    let session = repo
        .sessions().create_session(
            &ctx,
            workspace_id,
            Some("test-session-title"),
            "rag",
        )
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();

    let tool_results: Vec<contracts::ToolResult> = vec![
        contracts::ToolResult {
            tool: "calculator".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"result": 42.0, "expression": "6*7"})),
            trace: None,
        },
        contracts::ToolResult {
            tool: "code_interpreter".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Error,
            data: Some(serde_json::json!({"error": "SyntaxError"})),
            trace: None,
        },
    ];

    let message_id = repo
        .sessions().append_chat_turn(
            &ctx,
            session_id,
            &ChatTurn {
                user_content: "Calculate something",
                assistant_content: "Here are the results.",
                assistant_answer_blocks: &[],
                agent_type: "chat",
                citations: &[],
                tool_results: &tool_results,
                user_turn_metadata: None,
                user_resolved_query: None,
            },
        )
        .await
        .unwrap();
    assert!(message_id > 0);

    let messages = repo.list_messages(&ctx, session_id).await.unwrap();
    let assistant_message = messages
        .iter()
        .find(|m| m.role == "assistant")
        .expect("assistant message exists");

    assert_eq!(assistant_message.tool_results.len(), 2);
    assert_eq!(assistant_message.tool_results[0].tool, "calculator");
    assert_eq!(assistant_message.tool_results[0].status, contracts::ToolStatus::Ok);
    assert_eq!(
        assistant_message.tool_results[0].data.as_ref().unwrap()["result"],
        42.0
    );
    assert_eq!(assistant_message.tool_results[1].tool, "code_interpreter");
    assert_eq!(assistant_message.tool_results[1].status, contracts::ToolStatus::Error);
    assert_eq!(
        assistant_message.tool_results[1].data.as_ref().unwrap()["error"],
        "SyntaxError"
    );
}

#[tokio::test]
async fn chat_message_turn_metadata_roundtrip_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "turn-metadata-test", "turn metadata test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let session = repo
        .sessions().create_session(&ctx, workspace_id, Some("meta-session"), "rag")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();

    let metadata = serde_json::json!({
        "query_resolution": {
            "raw_query": "Who wrote it?",
            "resolved_query": "Who wrote Antifragile?",
            "slots": ["pronoun"],
            "method": "heuristic"
        }
    });

    let message_id = repo
        .sessions().append_chat_turn(
            &ctx,
            session_id,
            &ChatTurn {
                user_content: "Who wrote it?",
                assistant_content: "Taleb.",
                assistant_answer_blocks: &[],
                agent_type: "rag",
                citations: &[],
                tool_results: &[],
                user_turn_metadata: Some(metadata),
                user_resolved_query: Some("Who wrote Antifragile?"),
            },
        )
        .await
        .unwrap();

    let messages = repo.list_messages(&ctx, session_id).await.unwrap();
    let user_row = messages
        .iter()
        .find(|m| m.role == "user")
        .expect("user row");
    assert_eq!(user_row.content, "Who wrote it?");
    let stored_meta = user_row
        .turn_metadata
        .as_ref()
        .expect("turn_metadata should roundtrip");
    assert_eq!(
        stored_meta["query_resolution"]["resolved_query"],
        "Who wrote Antifragile?"
    );
    assert_eq!(
        user_row.resolved_query.as_deref(),
        Some("Who wrote Antifragile?")
    );
    assert!(message_id > 0);
}

