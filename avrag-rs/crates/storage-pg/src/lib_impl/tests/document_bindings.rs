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
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "binding test workspace", "binding test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "bound.txt", 42, "text/plain")
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
    assert_eq!(
        binding_count, 1,
        "workspace binding must exist for a workspace upload"
    );

    assert_eq!(
        document.workspace_id.as_deref(),
        Some(workspace.id.as_str())
    );

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
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "unique binding workspace", "unique binding")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "unique.txt", 42, "text/plain")
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
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "cascade workspace", "cascade")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "cascade.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let deleted = repo
        .bootstrap()
        .delete_workspace(&ctx, workspace_id)
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
    assert_eq!(
        bindings_left, 0,
        "workspace deletion must cascade the binding"
    );

    // The artifact row itself survives; async zero-binding GC (W2e) owns its fate.
    let artifact_status = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        status
    };
    // W2e orphan sweep: zero-binding artifacts enter the async cleanup flow
    // (soft-deleted here; the worker performs the full cleanup).
    assert_eq!(
        artifact_status, "deleting",
        "orphaned artifact must enter async GC"
    );
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
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "dual binding workspace", "dual binding")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "dual.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let session = repo
        .sessions()
        .create_session(
            &ctx,
            None,
            Some("dual binding session"),
            "chat",
            "quick_chat",
        )
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
    repo.bootstrap()
        .delete_workspace(&ctx, workspace_id)
        .await
        .unwrap();

    let artifact_status = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        status
    };
    assert_eq!(
        artifact_status, "pending",
        "session-bound artifact survives workspace deletion"
    );

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
    assert_eq!(
        deleted_sessions.1, 0,
        "conversation deletion cascades its binding"
    );
}

#[tokio::test]
async fn binding_rows_are_invisible_to_other_owners() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "rls binding workspace", "rls binding")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let document = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "rls.txt", 42, "text/plain")
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
        eprintln!(
            "PROBE current_user={} guc_user={:?} guc_role={:?}",
            probe.0, probe.1, probe.2
        );
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
    assert!(
        listed_by_other.is_empty(),
        "cross-owner document reach must stay empty"
    );
}

#[tokio::test]
async fn session_file_roundtrip_creates_lists_and_deletes_bindings() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
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
    assert!(
        document.workspace_id.is_none(),
        "session artifacts carry no workspace"
    );

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
    let other_ctx = AuthContext::new(
        UserId::from(Uuid::new_v4()),
        contracts::auth_runtime::SubjectKind::User,
    )
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
    let artifact_left =
        sqlx::query_scalar::<_, i64>("select count(*)::bigint from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        artifact_left, 1,
        "artifact deletion belongs to async GC, not binding delete"
    );
}

#[tokio::test]
async fn deleting_last_session_binding_sends_unbound_artifact_to_cleanup() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
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
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        (count, status)
    };
    assert_eq!(cleanup_tasks.0, 1, "exactly one idempotent cleanup task");
    assert_eq!(
        cleanup_tasks.1, "deleting",
        "artifact soft-deleted for the worker"
    );

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
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "dual gc workspace", "dual gc")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
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
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(document_uuid)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        status
    };
    assert_eq!(
        status, "pending",
        "artifact stays untouched while any binding remains"
    );
}

#[tokio::test]
async fn deleting_conversation_sweeps_unbound_artifacts_into_cleanup() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let session = repo
        .sessions()
        .create_session(&ctx, None, Some("gc sweep session"), "chat", "quick_chat")
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();
    let document = repo
        .bootstrap()
        .create_session_document(&ctx, session_id, "sweep.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();

    let deleted = repo
        .sessions()
        .delete_session(&ctx, session_id)
        .await
        .unwrap();
    assert!(deleted);

    let (status, tasks) = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        let tasks = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from document_cleanup_tasks where document_id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        (status, tasks)
    };
    assert_eq!(status, "deleting", "orphaned artifact must enter async GC");
    assert_eq!(tasks, 1, "exactly one cleanup task for the orphan");
}

/// Review round-6 Spec-3: deleting a workspace also cascades its SESSIONS —
/// session-only artifacts bound to those sessions must enter the orphan
/// sweep, not leak as unbound, still-non-deleting rows (W2e: workspace
/// deletion leaves zero-binding artifacts in the async cleanup flow).
#[tokio::test]
async fn deleting_workspace_sweeps_session_only_artifacts_of_its_sessions() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "sweep workspace", "sweep")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let session = repo
        .sessions()
        .create_session(
            &ctx,
            Some(workspace_id),
            Some("sweep session"),
            "chat",
            "agent",
        )
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();
    // Session-only artifact: bound ONLY via conversation_document_bindings.
    let session_doc = repo
        .bootstrap()
        .create_session_document(&ctx, session_id, "session-only.txt", 42, "text/plain")
        .await
        .unwrap();
    let session_doc_id = Uuid::parse_str(&session_doc.id).unwrap();
    // A workspace-bound artifact in the same workspace for contrast.
    let ws_doc = repo
        .bootstrap()
        .create_document(&ctx, workspace_id, "ws-only.txt", 42, "text/plain")
        .await
        .unwrap();
    let ws_doc_id = Uuid::parse_str(&ws_doc.id).unwrap();

    let deleted = repo
        .bootstrap()
        .delete_workspace(&ctx, workspace_id)
        .await
        .unwrap();
    assert!(deleted);

    let (session_doc_status, session_doc_tasks, ws_doc_tasks) = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(session_doc_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        let count_tasks =
            "select count(*)::bigint from document_cleanup_tasks where document_id = $1";
        let session_tasks = sqlx::query_scalar::<_, i64>(count_tasks)
            .bind(session_doc_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        let ws_tasks = sqlx::query_scalar::<_, i64>(count_tasks)
            .bind(ws_doc_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        (status, session_tasks, ws_tasks)
    };
    assert_eq!(
        session_doc_status, "deleting",
        "session-only artifact of a cascaded session must enter async GC, not leak"
    );
    assert_eq!(
        session_doc_tasks, 1,
        "exactly one cleanup task for the session-only orphan"
    );
    assert_eq!(
        ws_doc_tasks, 1,
        "workspace-bound orphan also swept exactly once"
    );
}

/// Review round-5 T1 / round-6+7+8 Spec-7: a dual-bound artifact (session +
/// workspace) deleted CONCURRENTLY from both scopes must not survive as a
/// zombie, under BOTH deterministic winner orders.
///
/// Determinism is built in three layers (review round-8: a bare spawn pair
/// does not control the lock queue, and a datname-wide waiter count can be
/// satisfied by unrelated sessions):
///
/// 1. Each race carries unique `application_name`s and records the two pool
///    backend PIDs. The barrier poll counts only those PIDs while they wait
///    for its binding-table lock.
/// 2. The winner task is spawned first and the test WAITS until it is
///    observed waiting on the binding-table lock before the loser task is
///    even spawned — PostgreSQL grants table locks FIFO, so the winner is
///    decided by observation, not scheduler luck.
/// 3. Only then does the loser spawn; when BOTH waiters are queued, the
///    helper barrier connection releases and the winner completes its whole
///    transaction (capture → scope delete → cascade → sweep → commit) before
///    the loser enters its capture window.
///
/// Serial execution cannot pass: the loser is spawned only after the winner
/// is provably blocked, so both transactions necessarily overlap.
#[tokio::test]
async fn concurrent_session_and_workspace_deletion_sweeps_dual_bound_artifact() {
    run_dual_deletion_barrier_race(DualDeletionWinner::SessionFirst).await;
}

/// The workspace-first winner order (review round-6 Spec-7): the workspace
/// delete is granted the lock first, cascades the session, and commits; the
/// session delete then runs against the already-cascaded session and MUST
/// report false — the precise outcome of this order, not a wildcard.
#[tokio::test]
async fn concurrent_deletion_workspace_first_order_sweeps_dual_bound_artifact() {
    run_dual_deletion_barrier_race(DualDeletionWinner::WorkspaceFirst).await;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DualDeletionWinner {
    SessionFirst,
    WorkspaceFirst,
}

async fn run_dual_deletion_barrier_race(winner: DualDeletionWinner) {
    use sqlx::ConnectOptions;
    use std::str::FromStr;
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    migration_role_context();
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository {
        pool: __bootstrap.pool.clone(),
    };
    repo.bootstrap().migrate().await.unwrap();

    let owner_user_id = UserId::from(Uuid::new_v4());
    let ctx = AuthContext::new(owner_user_id, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let workspace = repo
        .bootstrap()
        .create_workspace(&ctx, "dual delete workspace", "dual delete")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&workspace.id).unwrap();
    let session = repo
        .sessions()
        .create_session(
            &ctx,
            Some(workspace_id),
            Some("dual delete session"),
            "chat",
            "agent",
        )
        .await
        .unwrap();
    let session_id = Uuid::parse_str(&session.id).unwrap();
    let document = repo
        .bootstrap()
        .create_session_document(&ctx, session_id, "dual.txt", 42, "text/plain")
        .await
        .unwrap();
    let document_id = Uuid::parse_str(&document.id).unwrap();
    // Second binding: the artifact is visible through the workspace too.
    {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_user', $1, true)")
            .bind(ctx.user_id().into_uuid().to_string())
            .execute(tx.as_mut())
            .await
            .unwrap();
        sqlx::query(
            "insert into workspace_document_bindings (artifact_id, workspace_id, owner_user_id) values ($1, $2, $3)",
        )
        .bind(document_id)
        .bind(workspace_id)
        .bind(ctx.user_id().into_uuid())
        .execute(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // Barrier connection: holds SHARE ROW EXCLUSIVE on both binding tables.
    // The deleters block on the FIRST lock statement, before their capture
    // window — releasing it hands the grant to the FIFO-front deleter.
    let mut barrier = <sqlx::postgres::PgConnection as sqlx::Connection>::connect(&database_url)
        .await
        .unwrap();
    sqlx::query("begin").execute(&mut barrier).await.unwrap();
    let barrier_pid = sqlx::query_scalar::<_, i32>("select pg_backend_pid()")
        .fetch_one(&mut barrier)
        .await
        .unwrap();
    sqlx::query("lock table conversation_document_bindings in share row exclusive mode")
        .execute(&mut barrier)
        .await
        .unwrap();
    sqlx::query("lock table workspace_document_bindings in share row exclusive mode")
        .execute(&mut barrier)
        .await
        .unwrap();

    // One single-connection pool per deleter. Names are unique per race for
    // diagnostics; backend PIDs provide the actual barrier identity so a
    // concurrent copy of this test cannot satisfy our waiter count.
    let race_id = Uuid::new_v4().simple().to_string();
    let session_pool_name = format!("dd_session_{race_id}");
    let workspace_pool_name = format!("dd_workspace_{race_id}");
    let connect_opts = |name: &str| {
        sqlx::postgres::PgConnectOptions::from_str(&database_url)
            .unwrap()
            .application_name(name)
            .log_slow_statements(
                log::LevelFilter::Warn,
                std::time::Duration::from_millis(500),
            )
    };
    let session_pool = crate::pg_pool_options()
        .max_connections(1)
        .connect_with(connect_opts(&session_pool_name))
        .await
        .unwrap();
    let workspace_pool = crate::pg_pool_options()
        .max_connections(1)
        .connect_with(connect_opts(&workspace_pool_name))
        .await
        .unwrap();
    let session_backend_pid = sqlx::query_scalar::<_, i32>("select pg_backend_pid()")
        .fetch_one(&session_pool)
        .await
        .unwrap();
    let workspace_backend_pid = sqlx::query_scalar::<_, i32>("select pg_backend_pid()")
        .fetch_one(&workspace_pool)
        .await
        .unwrap();
    let session_repo = PgAppRepository::from_pool(session_pool);
    let workspace_repo = PgAppRepository::from_pool(workspace_pool);

    // Winner first: spawn and WAIT until it is observed waiting on the
    // binding-table lock — this (not scheduler timing) is what fixes the
    // FIFO queue order.
    let (first_pid, first_repo, first_ctx, first_op) = match winner {
        DualDeletionWinner::SessionFirst => (
            session_backend_pid,
            session_repo.clone(),
            ctx.clone(),
            DeleterOp::Session(session_id),
        ),
        DualDeletionWinner::WorkspaceFirst => (
            workspace_backend_pid,
            workspace_repo.clone(),
            ctx.clone(),
            DeleterOp::Workspace(workspace_id),
        ),
    };
    let first_handle = tokio::spawn({
        let repo = first_repo;
        let ctx = first_ctx;
        async move { first_op.execute(&repo, &ctx).await }
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let waiting = count_waiting_deleters(&repo, &[first_pid], barrier_pid).await;
        if waiting >= 1 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "winner deleter must be queued on the barrier lock within 30s"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // Loser second: now the queue order is already fixed (winner at front).
    let (second_pid, second_repo, second_ctx, second_op) = match winner {
        DualDeletionWinner::SessionFirst => (
            workspace_backend_pid,
            workspace_repo.clone(),
            ctx.clone(),
            DeleterOp::Workspace(workspace_id),
        ),
        DualDeletionWinner::WorkspaceFirst => (
            session_backend_pid,
            session_repo.clone(),
            ctx.clone(),
            DeleterOp::Session(session_id),
        ),
    };
    let second_handle = tokio::spawn({
        let repo = second_repo;
        let ctx = second_ctx;
        async move { second_op.execute(&repo, &ctx).await }
    });
    loop {
        let waiting = count_waiting_deleters(&repo, &[first_pid, second_pid], barrier_pid).await;
        if waiting >= 2 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "both deleters must block on the barrier within 30s (serial execution would fail here)"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    // Release: the FIFO-front deleter completes its whole transaction
    // (capture → scope delete → cascade → sweep → commit), then the other
    // enters its capture window against the post-commit state.
    sqlx::query("commit").execute(&mut barrier).await.unwrap();
    let first_result = first_handle.await.unwrap().unwrap();
    let second_result = second_handle.await.unwrap().unwrap();
    match winner {
        DualDeletionWinner::SessionFirst => {
            assert_eq!(
                first_result,
                DeleterResult::Session(true),
                "session-first winner must delete its scope row"
            );
            assert_eq!(
                second_result,
                DeleterResult::Workspace(true),
                "workspace scope row survives the session cascade"
            );
        }
        DualDeletionWinner::WorkspaceFirst => {
            assert_eq!(
                first_result,
                DeleterResult::Workspace(true),
                "workspace-first winner must delete its scope row"
            );
            assert_eq!(
                second_result,
                DeleterResult::Session(false),
                "session delete must report false: the workspace cascade already removed it"
            );
        }
    }

    let (binding_rows, status, tasks) = {
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(tx.as_mut())
            .await
            .unwrap();
        let bindings = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint
             from conversation_document_bindings where artifact_id = $1
             union all
             select count(*)::bigint
             from workspace_document_bindings where artifact_id = $1",
        )
        .bind(document_id)
        .fetch_all(tx.as_mut())
        .await
        .unwrap();
        let status = sqlx::query_scalar::<_, String>("select status from documents where id = $1")
            .bind(document_id)
            .fetch_one(tx.as_mut())
            .await
            .unwrap();
        let tasks = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from document_cleanup_tasks where document_id = $1",
        )
        .bind(document_id)
        .fetch_one(tx.as_mut())
        .await
        .unwrap();
        tx.commit().await.unwrap();
        (bindings, status, tasks)
    };
    assert_eq!(
        binding_rows.iter().sum::<i64>(),
        0,
        "both binding kinds must be gone after both deletions"
    );
    assert_eq!(
        status, "deleting",
        "the unbound dual artifact must enter async GC — no orphan leak past the race"
    );
    assert_eq!(
        tasks, 1,
        "exactly one cleanup task for the orphan (idempotency key dedupes concurrent sweeps)"
    );
}

/// The two scope-owner deletions, named so the barrier poll can assert on
/// exact per-operation outcomes (review round-8 S3: no stringly winner flag,
/// no wildcard branch, no discarded results).
#[derive(Debug, Clone, Copy)]
enum DeleterOp {
    Session(Uuid),
    Workspace(Uuid),
}

#[derive(Debug, PartialEq, Eq)]
enum DeleterResult {
    Session(bool),
    Workspace(bool),
}

impl DeleterOp {
    async fn execute(
        &self,
        repo: &PgAppRepository,
        ctx: &AuthContext,
    ) -> Result<DeleterResult, PgStorageError> {
        match self {
            DeleterOp::Session(session_id) => Ok(DeleterResult::Session(
                repo.sessions().delete_session(ctx, *session_id).await?,
            )),
            DeleterOp::Workspace(workspace_id) => Ok(DeleterResult::Workspace(
                repo.bootstrap()
                    .delete_workspace(ctx, *workspace_id)
                    .await?,
            )),
        }
    }
}

/// Count exactly the target backends waiting for this barrier's first binding
/// table lock. PID + blocker + relation + mode prevent another test instance
/// or an unrelated lock wait from satisfying the barrier.
async fn count_waiting_deleters(
    repo: &PgAppRepository,
    backend_pids: &[i32],
    barrier_pid: i32,
) -> i64 {
    let mut tx = repo.raw().begin().await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(tx.as_mut())
        .await
        .unwrap();
    let n = sqlx::query_scalar::<_, i64>(
        "select count(distinct activity.pid)::bigint
         from pg_stat_activity activity
         join pg_locks waiting
           on waiting.pid = activity.pid
          and waiting.granted = false
          and waiting.locktype = 'relation'
          and waiting.mode = 'ShareRowExclusiveLock'
          and waiting.relation = 'conversation_document_bindings'::regclass
         where activity.datname = current_database()
           and activity.pid = any($1)
           and activity.wait_event_type = 'Lock'
           and $2 = any(pg_blocking_pids(activity.pid))",
    )
    .bind(backend_pids)
    .bind(barrier_pid)
    .fetch_one(tx.as_mut())
    .await
    .unwrap();
    tx.commit().await.unwrap();
    n
}
