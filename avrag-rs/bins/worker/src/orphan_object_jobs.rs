use anyhow::Result;
use avrag_storage_pg::ObjectStoreHandle;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const DEFAULT_INTERVAL_SECS: u64 = 86400; // 24 hours

pub struct OrphanObjectJobRunner {
    pool: PgPool,
    object_store: Arc<ObjectStoreHandle>,
    interval: Duration,
    last_run_at: Option<Instant>,
}

impl OrphanObjectJobRunner {
    pub fn from_env(pool: PgPool, object_store: Arc<ObjectStoreHandle>) -> Option<Self> {
        if !env_bool("ORPHAN_OBJECT_CLEANUP_ENABLED", true) {
            return None;
        }

        let interval_secs = std::env::var("ORPHAN_OBJECT_CLEANUP_INTERVAL_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_INTERVAL_SECS);

        Some(Self {
            pool,
            object_store,
            interval: Duration::from_secs(interval_secs),
            last_run_at: None,
        })
    }

    pub async fn maybe_run(&mut self) -> Result<()> {
        let now = Instant::now();
        if let Some(last_run_at) = self.last_run_at
            && now.duration_since(last_run_at) < self.interval
        {
            return Ok(());
        }
        self.last_run_at = Some(now);

        info!("starting orphan object scan");

        let object_paths = match self.object_store.list().await {
            Ok(paths) => paths,
            Err(error) => {
                warn!(error = %error, "failed to list objects from store");
                return Err(error);
            }
        };

        if object_paths.is_empty() {
            info!("no objects found in store, skipping orphan scan");
            return Ok(());
        }

        let db_paths: HashSet<String> = match self.fetch_document_object_paths().await {
            Ok(paths) => paths.into_iter().collect(),
            Err(error) => {
                warn!(error = %error, "failed to fetch document object paths from db");
                return Err(error);
            }
        };

        let mut deleted = 0usize;
        let mut skipped = 0usize;

        for path in object_paths {
            if db_paths.contains(&path) {
                continue;
            }

            // Safety: only delete objects that match the expected path pattern
            // {org_id}/{workspace_id}/{document_id}/{filename}
            if path.split('/').count() < 4 {
                skipped += 1;
                continue;
            }

            if let Err(error) = self.object_store.delete(&path).await {
                warn!(path = %path, error = %error, "failed to delete orphan object");
            } else {
                info!(path = %path, "deleted orphan object");
                deleted += 1;
            }
        }

        if deleted > 0 || skipped > 0 {
            info!(deleted, skipped, "orphan object scan completed");
        } else {
            info!("orphan object scan completed, no orphans found");
        }

        Ok(())
    }

    async fn fetch_document_object_paths(&self) -> Result<Vec<String>> {
        // `documents` has forced row-level security keyed on `app.current_org`. The worker
        // pool has no org context, so a plain select sees zero rows and every object would
        // be misclassified as orphan and deleted — including in-flight uploads. Run the
        // scan as `super_admin` (allowed by `admin_access_documents`) inside a transaction
        // so the setting is scoped to this query only.
        let mut tx = self.pool.begin().await?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await?;
        let rows = sqlx::query_as::<_, (String,)>(
            "select object_path from documents where object_path is not null and object_path != ''",
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows.into_iter().map(|row| row.0).collect())
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::auth_runtime::{ActorId, AuthContext, OrgId, SubjectKind};
    use avrag_storage_pg::{BootstrapRepository, PgAppRepository};
    use uuid::Uuid;

    // Regression: `documents` has forced RLS keyed on `app.current_org`. The scan runs on
    // a raw pool with no org context, so it must escalate to `super_admin` to see in-flight
    // document object_paths — otherwise freshly uploaded objects get deleted mid-ingest.
    #[tokio::test]
    async fn orphan_scan_preserves_in_flight_document_object() {
        let Some(database_url) = std::env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = { let __b = BootstrapRepository::connect(&database_url).await.unwrap(); __b.migrate().await.unwrap(); PgAppRepository::from_pool(__b.raw().clone()) };

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, SubjectKind::User).with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .bootstrap().create_workspace(&ctx, "orphan-scan-test", "orphan scan test")
            .await
            .unwrap();
        let workspace_id = Uuid::parse_str(&notebook.id).unwrap();
        let document = repo
            .bootstrap().create_document(&ctx, workspace_id, "in-flight.txt", 7, "text/plain")
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();
        let seed = repo
            .bootstrap().get_document_task_seed(&ctx, document_id)
            .await
            .unwrap()
            .expect("document seed");
        let object_path = seed.object_path.clone();
        assert!(!object_path.is_empty());

        let tmp = tempfile::tempdir().unwrap();
        let store = ObjectStoreHandle::local(tmp.path().to_path_buf());
        store.put(&object_path, b"in-flight-bytes").await.unwrap();
        // A second object with no matching document row is a true orphan.
        let orphan_path = format!(
            "{}/{}/{}/orphan.bin",
            org_id.into_uuid(),
            workspace_id,
            Uuid::new_v4()
        );
        store
            .put(&orphan_path, b"orphan-bytes")
            .await
            .unwrap();

        let mut runner = OrphanObjectJobRunner {
            pool: repo.raw().clone(),
            object_store: Arc::new(store),
            interval: Duration::ZERO,
            last_run_at: None,
        };
        runner.maybe_run().await.unwrap();

        // In-flight document object must survive the scan.
        runner
            .object_store
            .get(&object_path)
            .await
            .expect("in-flight document object was deleted by orphan scan");
        // True orphan must be removed.
        let listing = runner.object_store.list().await.unwrap();
        assert!(
            !listing.contains(&orphan_path),
            "orphan object was not deleted: {listing:?}"
        );

        // Cleanup: drop the test document/notebook outside RLS via super_admin.
        let mut tx = repo.raw().begin().await.unwrap();
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("delete from documents where org_id = $1")
            .bind(org_id.into_uuid())
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("delete from workspaces where org_id = $1")
            .bind(org_id.into_uuid())
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
}
