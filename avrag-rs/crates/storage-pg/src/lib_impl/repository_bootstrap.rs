use super::*;

pub fn pg_pool_options() -> PgPoolOptions {
    let mut options = PgPoolOptions::new();
    if std::env::var("E2E_ENABLED").unwrap_or_default() == "true" {
        // Real-LLM E2E runs API server + worker pools concurrently; default 10
        // connections per pool exhausts quickly and surfaces as 404/500 errors.
        options = options
            .max_connections(25)
            .acquire_timeout(std::time::Duration::from_secs(30));
    } else {
        // Bounded pool with explicit timeouts: slow-request pileups must surface
        // as acquire errors, not silent unbounded queueing.
        let max_conn: u32 = std::env::var("AVRAG_PG_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        options = options
            .max_connections(max_conn)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .idle_timeout(std::time::Duration::from_secs(300));
    }
    options
}

#[derive(Debug)]
struct RuntimeRoleReport {
    role: String,
    rolsuper: bool,
    rolbypassrls: bool,
    rolcreatedb: bool,
    rolcreaterole: bool,
    member_of: Vec<String>,
    owns_tenant_tables: Vec<String>,
    rls_missing: Vec<String>,
}

impl BootstrapRepository {
    pub async fn connect(database_url: &str) -> Result<Self, PgStorageError> {
        use sqlx::ConnectOptions;
        use sqlx::postgres::PgConnectOptions;
        use std::str::FromStr;
        let connect_opts = PgConnectOptions::from_str(database_url)?;
        // Slow-statement log: concurrency forensics need per-statement timing.
        let connect_opts = connect_opts.log_slow_statements(
            log::LevelFilter::Warn,
            std::time::Duration::from_millis(500),
        );
        let pool = pg_pool_options().connect_with(connect_opts).await?;
        let this = Self {
            pool: TenantPgPool::new(pool),
        };
        // Fail-closed tenant-isolation gate: a superuser or BYPASSRLS role, or
        // any public tenant table left without FORCED RLS, refuses to come up.
        // No env escape hatch — provisioning (`avrag_runtime`) is mandatory.
        verify_runtime_role_safety(this.pool.raw()).await?;
        Ok(this)
    }

    /// Invariant inputs for [`verify_runtime_role_safety`], from pg_catalog.
    async fn runtime_role_report(pool: &PgPool) -> Result<RuntimeRoleReport, PgStorageError> {
        let role_row = sqlx::query(
            r#"
            select current_user as role,
                   (select rolsuper from pg_roles where rolname = current_user) as rolsuper,
                   (select rolbypassrls from pg_roles where rolname = current_user) as rolbypassrls,
                   (select rolcreatedb from pg_roles where rolname = current_user) as rolcreatedb,
                   (select rolcreaterole from pg_roles where rolname = current_user) as rolcreaterole
            "#,
        )
        .fetch_one(pool)
        .await?;
        let role: String = role_row.try_get("role")?;
        let member_of: Vec<String> = sqlx::query_scalar(
            r#"
            select g.rolname
            from pg_auth_members m
            join pg_roles g on g.oid = m.roleid
            join pg_roles u on u.oid = m.member
            where u.rolname = current_user
            "#,
        )
        .fetch_all(pool)
        .await?;
        let owns_tenant_tables: Vec<String> = sqlx::query_scalar(
            r#"
            select c.relname
            from pg_class c
            join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'public'
              and c.relkind = 'r'
              and pg_get_userbyid(c.relowner) = current_user
              and exists (
                  select 1 from pg_attribute a
                  where a.attrelid = c.oid and a.attname = 'owner_user_id' and not a.attisdropped
              )
            "#,
        )
        .fetch_all(pool)
        .await?;
        let rls_missing: Vec<String> = sqlx::query_scalar(
            r#"
            select c.relname
            from pg_class c
            join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'public'
              and c.relkind = 'r'
              and exists (
                  select 1 from pg_attribute a
                  where a.attrelid = c.oid and a.attname = 'owner_user_id' and not a.attisdropped
              )
              and (not c.relrowsecurity or not c.relforcerowsecurity)
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(RuntimeRoleReport {
            role,
            rolsuper: role_row.try_get("rolsuper")?,
            rolbypassrls: role_row.try_get("rolbypassrls")?,
            rolcreatedb: role_row.try_get("rolcreatedb")?,
            rolcreaterole: role_row.try_get("rolcreaterole")?,
            member_of,
            owns_tenant_tables,
            rls_missing,
        })
    }
}

/// The migration/owner role (`avrag`) legitimately owns tenant tables and, on
/// dev databases created before the role split, still carries createdb and
/// pre-FORCE-RLS tables. Set AVRAG_MIGRATION_ROLE_ONLY=true exclusively in the
/// migrator environment (migrate.env, local dev shells, test harnesses) to
/// exempt environment-class problems; production API/worker env files never
/// set it, so a leaked owner DSN cannot boot the API or worker.
fn migration_role_only() -> bool {
    std::env::var("AVRAG_MIGRATION_ROLE_ONLY")
        .map(|value| matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ))
        .unwrap_or(false)
}

/// Fail-closed tenant-isolation gate for API/worker pools and (partially) the
/// migrator: a superuser or BYPASSRLS role, or any role membership, refuses to
/// start in every context. Environment-class problems (createdb/createrole/
/// table ownership/missing FORCE RLS) refuse in API/worker contexts; the
/// migrator context is exempt from those because migrations 0082 and the
/// provisioning SQL are themselves the fix — refusing would deadlock the
/// process that must apply it. Gate is `AVRAG_MIGRATION_ROLE_ONLY`, set only
/// in the migrator environment (migrate.env / local dev / test harnesses);
/// production API/worker env files never set it.
async fn verify_runtime_role_safety(pool: &PgPool) -> Result<(), PgStorageError> {
    let report = match BootstrapRepository::runtime_role_report(pool).await {
        Ok(report) => report,
        // Inspection errors (e.g. missing pg_catalog visibility) block startup:
        // an unverifiable role is treated as unsafe.
        Err(error) => {
            tracing::error!(target: "avrag_role_guard", error = %error, "runtime role report unavailable");
            return Err(PgStorageError::NotFound(format!(
                "runtime role safety could not be verified: {error}"
            )));
        }
    };
    let mut problems = Vec::new();
    if report.rolsuper {
        problems.push("role is superuser".to_string());
    }
    if report.rolbypassrls {
        problems.push("role bypasses RLS".to_string());
    }
    if !report.member_of.is_empty() {
        problems.push(format!("role is member of: {}", report.member_of.join(", ")));
    }
    if !problems.is_empty() {
        // Privilege-class problems are refused in EVERY context, including the
        // migrator — no gate may waive superuser or BYPASSRLS.
        tracing::error!(target: "avrag_role_guard", role = %report.role, ?problems, "runtime role safety check failed (privilege class)");
        return Err(PgStorageError::NotFound(format!(
            "runtime role '{}' failed safety checks: {}",
            report.role,
            problems.join("; ")
        )));
    }
    // Environment-class problems (createdb/createrole/ownership/missing FORCE
    // RLS) can exist transiently in migrator/dev contexts — migration 0082 and
    // the provisioning SQL are what fix them, so refusing there would deadlock
    // the very process that must apply the fix. All other contexts fail closed.
    if migration_role_only() {
        tracing::info!(target: "avrag_role_guard", role = %report.role, "runtime role safety verified (migration-role context)");
        return Ok(());
    }
    if report.rolcreatedb {
        problems.push("role can create databases".to_string());
    }
    if report.rolcreaterole {
        problems.push("role can create roles".to_string());
    }
    if !report.owns_tenant_tables.is_empty() {
        problems.push(format!(
            "runtime role must not own tenant tables: {}",
            report.owns_tenant_tables.join(", ")
        ));
    }
    if !report.rls_missing.is_empty() {
        problems.push(format!(
            "tenant tables without FORCED RLS: {}",
            report.rls_missing.join(", ")
        ));
    }
    if !problems.is_empty() {
        tracing::error!(target: "avrag_role_guard", role = %report.role, ?problems, "runtime role safety check failed");
        return Err(PgStorageError::NotFound(format!(
            "runtime role '{}' failed safety checks: {}",
            report.role,
            problems.join("; ")
        )));
    }
    tracing::info!(target: "avrag_role_guard", role = %report.role, "runtime role safety verified");
    Ok(())
}

impl BootstrapRepository {
    pub async fn migrate(&self) -> Result<(), PgStorageError> {
        // Prefer runtime path for packaged/VPS deploys; fall back to crate-relative path in dev.
        let migrations_path = std::env::var("AVRAG_MIGRATIONS_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
            });
        // pgvector retrieval data plane (0060 rag_pgvector / 0061 rag_bigm) is a
        // local/private backend option; Milvus production (the default) must not
        // run `CREATE EXTENSION vector` / `pg_bigm`. Match app-core's
        // RetrievalBackend parse of RETRIEVAL_BACKEND (env: milvus | pgvector).
        let retrieval_backend = std::env::var("RETRIEVAL_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let mut migrator = sqlx::migrate::Migrator::new(migrations_path.as_path()).await?;
        let pgvector_backend = matches!(
            retrieval_backend.as_str(),
            "pgvector" | "postgres" | "pg"
        );
        if !pgvector_backend {
            migrator.migrations = std::borrow::Cow::Owned(
                migrator
                    .iter()
                    .filter(|m| !matches!(m.version, 60 | 61))
                    .cloned()
                    .collect(),
            );
        }
        migrator.run(self.pool.raw()).await?;
        if std::env::var("AVRAG_SKIP_SEARCH_TOKEN_RESEGMENT")
            .map(|value| matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ))
            .unwrap_or(false)
        {
            return Ok(());
        }
        let updated = ConversationMemoryRepository { pool: self.pool.clone() }.resegment_chat_message_search_tokens().await?;
        if updated > 0 {
            tracing::info!(
                updated_rows = updated,
                "resegmented chat_messages.search_tokens with jieba"
            );
        }
        Ok(())
    }

    pub async fn ping(&self) -> Result<(), PgStorageError> {
        sqlx::query("select 1").execute(self.pool.raw()).await?;
        Ok(())
    }

    pub fn raw(&self) -> &PgPool {
        self.pool.raw()
    }

    pub async fn get_workspace(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<Workspace>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, owner_user_id, owner_id, title, description, created_at, updated_at
            from workspaces
            where id = $1 and owner_user_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_notebook).transpose()
    }

    pub async fn create_workspace(
        &self,
        context: &AuthContext,
        name: &str,
        description: &str,
    ) -> Result<Workspace, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = sqlx::query(
            r#"
            insert into workspaces (owner_user_id, owner_id, title, description)
            values ($1, $2, $3, $4)
            returning id, owner_user_id, owner_id, title, description, created_at, updated_at
            "#,
        )
        .bind(context.user_id().into_uuid())
        .bind(context.actor_id().map(ActorId::into_uuid))
        .bind(name)
        .bind(description)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        map_notebook(row)
    }

    pub async fn update_workspace(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
        name: &str,
        description: &str,
    ) -> Result<Option<Workspace>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            update workspaces
            set title = $2, description = $3, updated_at = now()
            where id = $1 and owner_user_id = $4
            returning id, owner_user_id, owner_id, title, description, created_at, updated_at
            "#,
        )
        .bind(workspace_id)
        .bind(name)
        .bind(description)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_notebook).transpose()
    }

    pub async fn delete_workspace(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let owner_user_id = context.user_id().into_uuid();
        // Capture the artifacts bound to this workspace before the cascade
        // removes their bindings (W2e: orphan sweep).
        let bound_artifacts: Vec<Uuid> = sqlx::query(
            "select artifact_id from workspace_document_bindings where workspace_id = $1 and owner_user_id = $2",
        )
        .bind(workspace_id)
        .bind(owner_user_id)
        .fetch_all(tx.inner())
        .await?
        .into_iter()
        .filter_map(|row: PgRow| row.try_get::<Uuid, _>("artifact_id").ok())
        .collect();

        let result = sqlx::query("delete from workspaces where id = $1 and owner_user_id = $2")
            .bind(workspace_id)
            .bind(owner_user_id)
            .execute(tx.inner())
            .await?;
        let deleted = result.rows_affected() > 0;
        if !deleted {
            tx.commit().await?;
            return Ok(false);
        }

        // Artifacts left with zero bindings of either kind enter the async
        // full-cleanup flow; anything still session-bound survives untouched.
        for document_id in bound_artifacts {
            let orphaned = sqlx::query_scalar::<_, bool>(
                r#"
                select not exists (
                    select 1 from conversation_document_bindings b where b.artifact_id = $1
                )
                and not exists (
                    select 1 from workspace_document_bindings b where b.artifact_id = $1
                )
                "#,
            )
            .bind(document_id)
            .fetch_one(tx.inner())
            .await?;
            if orphaned {
                sqlx::query(
                    r#"
                    update documents
                    set status = 'deleting',
                        deletion_requested_at = coalesce(deletion_requested_at, now()),
                        deletion_error = null,
                        updated_at = now()
                    where id = $1
                      and owner_user_id = $2
                      and status not in ('deleting', 'deleted')
                    "#,
                )
                .bind(document_id)
                .bind(owner_user_id)
                .execute(tx.inner())
                .await?;
                sqlx::query(
                    r#"
                    insert into document_cleanup_tasks (
                        owner_user_id, workspace_id, document_id, requested_by, idempotency_key, payload
                    )
                    values ($1, null, $2, $3, $4, $5)
                    on conflict (idempotency_key) do nothing
                    "#,
                )
                .bind(owner_user_id)
                .bind(document_id)
                .bind(context.actor_id().map(ActorId::into_uuid))
                .bind(format!("document-cleanup:{owner_user_id}:{document_id}"))
                .bind(serde_json::json!({
                    "owner_user_id": owner_user_id.to_string(),
                    "document_id": document_id.to_string(),
                    "reason": "workspace_delete_orphan",
                }))
                .execute(tx.inner())
                .await?;
                sqlx::query(
                    r#"
                    update ingestion_tasks
                    set status = 'dead_letter',
                        dead_lettered_at = coalesce(dead_lettered_at, now()),
                        last_failed_at = coalesce(last_failed_at, now()),
                        last_error = coalesce(last_error, 'document deletion requested'),
                        locked_at = null,
                        locked_by = null,
                        lock_token = null,
                        updated_at = now()
                    where owner_user_id = $1
                      and document_id = $2
                      and status in ('queued', 'processing')
                      and dead_lettered_at is null
                    "#,
                )
                .bind(owner_user_id)
                .bind(document_id)
                .execute(tx.inner())
                .await?;
            }
        }
        tx.commit().await?;
        Ok(true)
    }

    pub async fn create_document(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let document_id = Uuid::new_v4();
        // Parent-guarded: zero rows when the workspace belongs to someone else.
        let row = sqlx::query(
            r#"
            insert into documents (id, owner_user_id, file_name, mime_type, file_size, status, chunk_count, object_path, user_id)
            select $1, $2, $4, $5, $6, 'pending', 0, $7, $8
            where exists (
                select 1 from workspaces w
                where w.id = $3::uuid and w.owner_user_id = $2::uuid
            )
            returning id, owner_user_id, $3::uuid as workspace_id, file_name, mime_type, file_size, status, chunk_count, created_at, updated_at
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .bind(workspace_id)
        .bind(filename)
        .bind(mime_type)
        .bind(i64::try_from(file_size).unwrap_or(i64::MAX))
        .bind(build_object_path(context, workspace_id, document_id, filename))
        .bind(context.actor_id().map(ActorId::into_uuid))
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            return Err(PgStorageError::NotFound("resource not found".to_string()));
        };
        // Scope truth lives in the typed binding table, written in the same tx.
        sqlx::query(
            r#"
            insert into workspace_document_bindings (artifact_id, workspace_id, owner_user_id)
            values ($1, $2, $3)
            on conflict (workspace_id, artifact_id) do nothing
            "#,
        )
        .bind(document_id)
        .bind(workspace_id)
        .bind(context.user_id().into_uuid())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        map_document(row)
    }

    pub async fn upsert_published_document(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        workspace_id: Uuid,
        filename: &str,
        mime_type: &str,
        chunk_count: usize,
    ) -> Result<Document, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let object_path = build_object_path(context, workspace_id, document_id, filename);
        let chunk_count_i32 = i32::try_from(chunk_count).unwrap_or(i32::MAX);
        let row = sqlx::query(
            r#"
            insert into documents (
                id, owner_user_id, file_name, mime_type, file_size,
                status, chunk_count, object_path, user_id
            )
            values ($1, $2, $4, $5, 0, 'completed', $6, $7, $8)
            on conflict (id) do update set
                owner_user_id = excluded.owner_user_id,
                file_name = excluded.file_name,
                mime_type = excluded.mime_type,
                status = 'completed',
                chunk_count = excluded.chunk_count,
                updated_at = now()
            where documents.owner_user_id = excluded.owner_user_id
            returning id, owner_user_id, $3::uuid as workspace_id, file_name, mime_type, file_size, status, chunk_count, created_at, updated_at
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .bind(workspace_id)
        .bind(filename)
        .bind(mime_type)
        .bind(chunk_count_i32)
        .bind(object_path)
        .bind(context.actor_id().map(ActorId::into_uuid))
        .fetch_optional(tx.inner())
        .await?;
        sqlx::query(
            r#"
            insert into workspace_document_bindings (artifact_id, workspace_id, owner_user_id)
            values ($1, $2, $3)
            on conflict (workspace_id, artifact_id) do nothing
            "#,
        )
        .bind(document_id)
        .bind(workspace_id)
        .bind(context.user_id().into_uuid())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        let Some(row) = row else {
            return Err(PgStorageError::NotFound(
                "published document id belongs to another owner".to_string(),
            ));
        };
        map_document(row)
    }

    /// Chat-first W2b: artifact + conversation binding in one transaction.
    /// Parent-guarded on the session so a foreign conversation id yields NotFound.
    pub async fn create_session_document(
        &self,
        context: &AuthContext,
        conversation_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let document_id = Uuid::new_v4();
        let object_path = format!(
            "{}/_sessions/{}/{}",
            context.user_id(),
            document_id,
            sanitize_filename(filename)
        );
        let row = sqlx::query(
            r#"
            insert into documents (id, owner_user_id, file_name, mime_type, file_size, status, chunk_count, object_path, user_id)
            values ($1, $2, $3, $4, $5, 'pending', 0, $6, $7)
            returning id, owner_user_id, null::uuid as workspace_id, file_name, mime_type, file_size, status, chunk_count, created_at, updated_at
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .bind(filename)
        .bind(mime_type)
        .bind(i64::try_from(file_size).unwrap_or(i64::MAX))
        .bind(object_path)
        .bind(context.actor_id().map(ActorId::into_uuid))
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            return Err(PgStorageError::NotFound("resource not found".to_string()));
        };
        let inserted = sqlx::query(
            r#"
            insert into conversation_document_bindings (artifact_id, conversation_id, owner_user_id)
            select $1, $2, $3
            where exists (
                select 1 from chat_sessions s
                where s.id = $2 and s.owner_user_id = $3
            )
            on conflict (conversation_id, artifact_id) do nothing
            "#,
        )
        .bind(document_id)
        .bind(conversation_id)
        .bind(context.user_id().into_uuid())
        .execute(tx.inner())
        .await?;
        if inserted.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("session not found".to_string()));
        }
        tx.commit().await?;
        map_document(row)
    }

    pub async fn list_session_files(
        &self,
        context: &AuthContext,
        conversation_id: Uuid,
    ) -> Result<Vec<contracts::documents::SessionFileRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select b.id as binding_id, d.id as document_id, d.file_name, d.mime_type,
                   d.file_size, d.status, b.created_at
            from conversation_document_bindings b
            join documents d on d.id = b.artifact_id
            where b.conversation_id = $1
              and b.owner_user_id = $2
              and d.status not in ('deleting', 'deleted')
            order by b.created_at asc, b.id asc
            "#,
        )
        .bind(conversation_id)
        .bind(context.user_id().into_uuid())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter()
            .map(|row: PgRow| {
                Ok(contracts::documents::SessionFileRow {
                    binding_id: row.try_get::<Uuid, _>("binding_id")?.to_string(),
                    document_id: row.try_get::<Uuid, _>("document_id")?.to_string(),
                    file_name: row.try_get("file_name")?,
                    mime_type: row.try_get::<Option<String>, _>("mime_type")?
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    file_size: u64::try_from(row.try_get::<i64, _>("file_size")?)
                        .unwrap_or_default(),
                    status: row.try_get("status")?,
                    created_at: row.try_get::<DateTime<Utc>, _>("created_at")?.to_rfc3339(),
                })
            })
            .collect()
    }

    pub async fn delete_session_file_binding(
        &self,
        context: &AuthContext,
        conversation_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<String>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            delete from conversation_document_bindings
            where id = $1
              and conversation_id = $2
              and owner_user_id = $3
            returning artifact_id
            "#,
        )
        .bind(binding_id)
        .bind(conversation_id)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(row.and_then(|row| {
            row.try_get::<Uuid, _>("artifact_id")
                .ok()
                .map(|value| value.to_string())
        }))
    }

    pub async fn get_document_task_seed(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentTaskSeed>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select d.id, d.owner_user_id, wb.workspace_id, d.file_name, d.mime_type, d.file_size, d.object_path, d.status
            from documents d
            left join lateral (
                select b.workspace_id
                from workspace_document_bindings b
                where b.artifact_id = d.id
                order by b.created_at asc, b.id asc
                limit 1
            ) wb on true
            where d.id = $1 and d.owner_user_id = $2
            "#,
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_document_task_seed).transpose()
    }

    pub async fn store_document_body(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        content: &str,
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let body_items = build_preview_items(content);
        self.store_document_body_items(context, document_id, None, content, &body_items)
            .await
    }

    pub async fn store_document_body_chunks(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        parse_run_id: Option<Uuid>,
        content: &str,
        body_chunks: &[StoreDocumentChunkParams],
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let summary = build_summary(content);
        let mut indexed_chunks = Vec::new();

        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let result = sqlx::query(
            r#"
            update documents
            set file_size = $2, chunk_count = $3, updated_at = now()
            where id = $1
              and owner_user_id = $4
              and status not in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(i64::try_from(content.len()).unwrap_or(i64::MAX))
        .bind(i32::try_from(body_chunks.len()).unwrap_or(i32::MAX))
        .bind(context.user_id().into_uuid())
        .execute(tx.inner())
        .await?;

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document not found".to_string()));
        }

        // 全量重建检索族 chunk（body/summary/profile 等）；**保留 table_evidence**
        // （struct_query 表级证据由表格阶段独立维护——检索重建若把它一并清掉，
        // 表格阶段先跑时证据会被本方法随后擦掉，2026-07-31 本地验收实测）。
        sqlx::query("delete from chunks where document_id = $1 and owner_user_id = $2 and chunk_type <> 'table_evidence'")
            .bind(document_id)
            .bind(context.user_id().into_uuid())
            .execute(tx.inner())
            .await?;

        for chunk in body_chunks {
            let row = sqlx::query(
                r#"
                insert into chunks (owner_user_id, document_id, parse_run_id, chunk_type, page, content, metadata)
                values ($1, $2, $3, 'body', $4, $5, $6)
                returning id, document_id, page, content, metadata
                "#,
            )
            .bind(context.user_id().into_uuid())
            .bind(document_id)
            .bind(chunk.parse_run_id)
            .bind(chunk.page)
            .bind(&chunk.content)
            .bind(&chunk.metadata)
            .fetch_one(tx.inner())
            .await?;

            indexed_chunks.push(IndexedChunk {
                chunk_id: row
                    .try_get::<Uuid, _>("id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                doc_id: row
                    .try_get::<Uuid, _>("document_id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                page: row
                    .try_get::<Option<i32>, _>("page")
                    .ok()
                    .flatten()
                    .map(i64::from),
                content: row.try_get("content").unwrap_or_default(),
                score: None,
                metadata: row.try_get("metadata").unwrap_or_else(|_| json!({})),
            });
        }

        sqlx::query(
            r#"
            insert into chunks (owner_user_id, document_id, parse_run_id, chunk_type, page, content, metadata)
            values ($1, $2, $3, 'summary', 1, $4, '{}'::jsonb)
            "#,
        )
        .bind(context.user_id().into_uuid())
        .bind(document_id)
        .bind(parse_run_id)
        .bind(summary)
        .execute(tx.inner())
        .await?;

        tx.commit().await?;
        Ok(indexed_chunks)
    }

    pub async fn replace_document_toc(
        &self,
        context: &AuthContext,
        workspace_id: Option<Uuid>,
        document_id: Uuid,
        entries: &[TocEntry],
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;

        // Parent-guarded replacement: zero side effects when the document
        // belongs to another owner (surfaces via caller's NotFound checks).
        let guard = sqlx::query_scalar::<_, i64>(
            "select 1 from documents where id = $1 and owner_user_id = $2 for update",
        )
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        if guard.is_none() {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document not found".to_string()));
        }

        sqlx::query("delete from document_toc where document_id = $1 and owner_user_id = $2")
            .bind(document_id)
            .bind(context.user_id().into_uuid())
            .execute(tx.inner())
            .await?;

        for entry in entries {
            sqlx::query(
                r#"
                insert into document_toc (
                    id, owner_user_id, document_id, workspace_id, parent_id,
                    title, heading_level, page, chunk_id, rank, overview
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(entry.id)
            .bind(context.user_id().into_uuid())
            .bind(document_id)
            .bind(workspace_id)
            .bind(entry.parent_id)
            .bind(&entry.title)
            .bind(entry.heading_level)
            .bind(entry.page)
            .bind(entry.chunk_id)
            .bind(entry.rank)
            .bind(entry.overview.as_deref())
            .execute(tx.inner())
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_document_toc_entries(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, TocEntry)>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select document_id, id, parent_id, title, heading_level, page, chunk_id, rank, overview
            from document_toc
            where document_id = any($1) and owner_user_id = $2
            order by document_id, rank
            "#,
        )
        .bind(doc_ids)
        .bind(context.user_id().into_uuid())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let doc_id: Uuid = row.try_get("document_id").ok()?;
                let id: Uuid = row.try_get("id").ok()?;
                let parent_id: Option<Uuid> = row.try_get("parent_id").ok().flatten();
                let title: String = row.try_get("title").ok()?;
                let heading_level: i32 = row.try_get("heading_level").ok()?;
                let page: Option<i32> = row.try_get("page").ok().flatten();
                let chunk_id: Option<Uuid> = row.try_get("chunk_id").ok().flatten();
                let rank: i32 = row.try_get("rank").ok()?;
                let overview: Option<String> = row.try_get("overview").ok().flatten();
                Some((
                    doc_id,
                    TocEntry {
                        id,
                        parent_id,
                        title,
                        heading_level,
                        page,
                        chunk_id,
                        rank,
                        overview,
                    },
                ))
            })
            .collect())
    }

    pub async fn store_document_body_items(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        parse_run_id: Option<Uuid>,
        content: &str,
        body_items: &[ParsedPreviewItem],
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let body_chunks = body_items
            .iter()
            .map(|item| StoreDocumentChunkParams {
                parse_run_id,
                page: Some(i32::try_from(item.page).unwrap_or(1)),
                content: item.text.clone(),
                metadata: json!({
                    "kind": item.kind,
                    "cursor": item.cursor,
                    "page": item.page,
                }),
            })
            .collect::<Vec<_>>();

        self.store_document_body_chunks(context, document_id, parse_run_id, content, &body_chunks)
            .await
    }


}
