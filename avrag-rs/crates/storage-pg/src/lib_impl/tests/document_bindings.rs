use super::support::*;

/// W2a chat-first: typed document bindings are the scope truth.
/// `documents` no longer carries a workspace column; workspace / session
/// visibility derives from `workspace_document_bindings` and
/// `conversation_document_bindings` (design 2026-09-02-chat-first §4.3).

#[tokio::test]
async fn create_document_writes_workspace_binding_and_lists_via_binding() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "binding test notebook", "binding test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "bound.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    // The artifact row no longer carries the scope; the binding does.
    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(tx.as_mut())
        .await
        .unwrap();
    let binding_count = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from workspace_document_bindings where artifact_id = $1 and workspace_id = $2",
    )
    .bind(document_id)
    .bind(workspace_id)
    .fetch_one(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(binding_count, 1, "workspace binding must exist for a workspace upload");

    assert_eq!(document.workspace_id.as_deref(), Some(notebook.id.as_str()));

    let listed = repo
        .list_documents(&ctx, Some(workspace_id), Some(document_id))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1, "list must resolve scope via the binding");
}

#[tokio::test]
async fn workspace_document_bindings_reject_duplicate_scope_pair() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "unique binding notebook", "unique binding")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "unique.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(tx.as_mut())
        .await
        .unwrap();
    let duplicate = sqlx::query(
        "insert into workspace_document_bindings (artifact_id, workspace_id, owner_user_id) values ($1, $2, $3)",
    )
    .bind(document_id)
    .bind(workspace_id)
    .bind(owner_user_id.into_uuid())
    .execute(tx.as_mut())
    .await;
    tx.commit().await.unwrap();
    assert!(
        duplicate.is_err(),
        "UNIQUE (workspace_id, artifact_id) must reject a duplicate binding"
    );
}

#[tokio::test]
async fn deleting_workspace_cascades_binding_but_artifact_survives_for_gc() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "cascade notebook", "cascade")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "cascade.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let deleted = repo
        .bootstrap().delete_workspace(&ctx, workspace_id)
        .await
        .unwrap();
    assert!(deleted);

    let bindings_left = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from workspace_document_bindings where artifact_id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        count
    };
    assert_eq!(bindings_left, 0, "workspace deletion must cascade the binding");

    // The artifact row itself survives; async zero-binding GC (W2e) owns its fate.
    let artifact_status = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>(
            "select status from documents where id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        status
    };
    // W2e orphan sweep: zero-binding artifacts enter the async cleanup flow
    // (soft-deleted here; the worker performs the full cleanup).
    assert_eq!(artifact_status, "deleting", "orphaned artifact must enter async GC");
    let listed = repo
        .list_documents(&ctx, Some(workspace_id), None)
        .await
        .unwrap();
    assert!(listed.is_empty(), "no visibility without a binding");
}

#[tokio::test]
async fn conversation_binding_keeps_artifact_when_workspace_binding_is_gone() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "dual binding notebook", "dual binding")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "dual.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let session = repo
        .sessions()
        .create_session(&ctx, None, Some("dual binding session"), "chat", "quick_chat")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();

    {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        sqlx::query(
            "insert into conversation_document_bindings (artifact_id, conversation_id, owner_user_id) values ($1, $2, $3)",
        )
        .bind(document_id)
        .bind(session_id)
        .bind(owner_user_id.into_uuid())
        .execute(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // Both scopes see the same artifact; deleting the workspace binding side
    // must not touch the artifact while the conversation binding remains.
    repo.bootstrap().delete_workspace(&ctx, workspace_id).await.unwrap();

    let artifact_status = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>(
            "select status from documents where id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        status
    };
    assert_eq!(artifact_status, "pending", "session-bound artifact survives workspace deletion");

    // Deleting the conversation cascades its binding; artifact is then
    // zero-bound and left for async GC (not deleted inline).
    let deleted_sessions = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let result = sqlx::query("delete from chat_sessions where id = $1")
            .bind(session_id)
            .execute(tx.as_mut())
            .await
            .unwrap();
        let bindings_left = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from conversation_document_bindings where artifact_id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        (result.rows_affected(), bindings_left)
    };
    assert_eq!(deleted_sessions.0, 1);
    assert_eq!(deleted_sessions.1, 0, "conversation deletion cascades its binding");
}

#[tokio::test]
async fn binding_rows_are_invisible_to_other_owners() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "rls binding notebook", "rls binding")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap().create_document(&ctx, workspace_id, "rls.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let other_owner = UserId::from(Uuid::new_v4());
    let other_ctx = AuthContext::new(other_owner, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let mut tx = repo.raw().begin().await.unwrap();
    // No super_admin here: under FORCE RLS the owner role is subject to the
    // policy, so switching only the owner GUC must hide the foreign rows.
    let seen_by_other = {
        sqlx::query("select set_config('app.current_user', $1, true)")
            .bind(other_owner.into_uuid().to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        let probe: (String, Option<String>, Option<String>) = sqlx::query_as(
            "select current_user, current_setting('app.current_user', true), current_setting('app.current_role', true)",
        )
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        eprintln!("PROBE current_user={} guc_user={:?} guc_role={:?}", probe.0, probe.1, probe.2);
        sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from workspace_document_bindings where artifact_id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap()
    };
    tx.commit().await.unwrap();
    assert_eq!(seen_by_other, 0, "forced RLS must hide foreign bindings");

    let listed_by_other = repo
        .list_documents(&other_ctx, Some(workspace_id), Some(document_id))
        .await
        .unwrap();
    assert!(listed_by_other.is_empty(), "cross-owner document reach must stay empty");
}

#[tokio::test]
async fn session_file_roundtrip_creates_lists_and_deletes_bindings() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let session = repo
        .sessions()
        .create_session(&ctx, None, Some("files session"), "chat", "quick_chat")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();

    let document = repo
        .bootstrap()
        .create_session_document(&ctx, session_id, "tray.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    assert!(document.workspace_id.is_none(), "session artifacts carry no workspace");

    let files = repo
        .bootstrap()
        .list_session_files(&ctx, session_id)
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].document_id, document.id);
    assert_eq!(files[0].file_name, "tray.txt");
    assert_eq!(files[0].status, "pending");
    let binding_id = Uuid::parse_str(&files[0].binding_id).unwrap();

    // Cross-owner list stays empty.
    let other_ctx = AuthContext::new(UserId::from(Uuid::new_v4()), contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));
    let foreign = repo
        .bootstrap()
        .list_session_files(&other_ctx, session_id)
        .await
        .unwrap();
    assert!(foreign.is_empty(), "foreign owner must not see the binding");

    // Delete by binding id; the artifact row survives for async GC.
    let deleted = repo
        .bootstrap()
        .delete_session_file_binding(&ctx, session_id, binding_id)
        .await
        .unwrap();
    assert!(deleted.is_some(), "the binding must be removable by its id");
    let files_after = repo
        .bootstrap()
        .list_session_files(&ctx, session_id)
        .await
        .unwrap();
    assert!(files_after.is_empty());

    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(tx.as_mut())
        .await
        .unwrap();
    let artifact_left = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from documents where id = $1",
    )
    .bind(document_id)
    .fetch_one(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(artifact_left, 1, "artifact deletion belongs to async GC, not binding delete");
}

#[tokio::test]
async fn deleting_last_session_binding_sends_unbound_artifact_to_cleanup() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let session = repo
        .sessions()
        .create_session(&ctx, None, Some("gc session"), "chat", "quick_chat")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();
    let document = repo
        .bootstrap()
        .create_session_document(&ctx, session_id, "gc.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    let files = repo
        .bootstrap()
        .list_session_files(&ctx, session_id)
        .await
        .unwrap();
    let binding_id = Uuid::parse_str(&files[0].binding_id).unwrap();

    // Remove the last binding: the artifact must enter the async cleanup flow.
    let removed = repo
        .bootstrap()
        .delete_session_file_binding(&ctx, session_id, binding_id)
        .await
        .unwrap();
    assert_eq!(removed.as_deref(), Some(document.id.as_str()));
    let entered_gc = repo
        .documents()
        .delete_document_if_unbound(&ctx, document_id)
        .await
        .unwrap();
    assert!(entered_gc, "zero-binding artifact must enter async cleanup");

    let cleanup_tasks = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from document_cleanup_tasks where document_id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        let status = sqlx::query_scalar::<_, String>(
            "select status from documents where id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        (count, status)
    };
    assert_eq!(cleanup_tasks.0, 1, "exactly one idempotent cleanup task");
    assert_eq!(cleanup_tasks.1, "deleting", "artifact soft-deleted for the worker");

    // A second call finds nothing left to do (idempotent).
    let again = repo
        .documents()
        .delete_document_if_unbound(&ctx, document_id)
        .await
        .unwrap();
    assert!(!again, "already-deleting artifact must be a no-op");
}

#[tokio::test]
async fn workspace_binding_keeps_artifact_out_of_session_delete_gc() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx, "dual gc notebook", "dual gc")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "dual-gc.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let session = repo
        .sessions()
        .create_session(&ctx, None, Some("dual gc session"), "chat", "quick_chat")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();
    let document_uuid = document_id;
    let session_document = repo
        .bootstrap()
        .create_session_document(&ctx, session_id, "ignored.txt", 1, "text/plain")
        .await
        .unwrap();
    let _ = session_document;
    // Bind the same artifact to the conversation as well (dual binding).
    {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        sqlx::query(
            "insert into conversation_document_bindings (artifact_id, conversation_id, owner_user_id) values ($1, $2, $3) on conflict do nothing",
        )
        .bind(document_uuid)
        .bind(session_id)
        .bind(owner_user_id.into_uuid())
        .execute(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // The workspace binding still exists, so the artifact is not unbound.
    let entered_gc = repo
        .documents()
        .delete_document_if_unbound(&ctx, document_uuid)
        .await
        .unwrap();
    assert!(!entered_gc, "workspace-bound artifact must not enter GC");

    let status = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>(
            "select status from documents where id = $1",
        )
        .bind(document_uuid)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        status
    };
    assert_eq!(status, "pending", "artifact stays untouched while any binding remains");
}
