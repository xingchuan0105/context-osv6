use sqlx::Row;
use uuid::Uuid;

use super::AppState;

impl AppState {
    pub async fn reset_e2e_user_data(&self, email: &str) -> Result<bool, String> {
        let repo = self
            .postgres_repo()
            .ok_or_else(|| "database not available".to_string())?;
        let user_id: Option<Uuid> = match sqlx::query_as::<_, (Option<Uuid>,)>(
            "select id from users where email = $1 limit 1",
        )
        .bind(email)
        .fetch_optional(repo.raw())
        .await
        {
            Ok(Some((Some(id),))) => Some(id),
            Ok(Some((None,))) | Ok(None) => None,
            Err(error) => {
                tracing::warn!(error = %error, "E2E reset: database error during user lookup");
                return Err("user lookup failed".to_string());
            }
        };
        let Some(user_id) = user_id else {
            return Ok(false);
        };
        repo.auth()
            .delete_user_cascade(self.auth(), user_id)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E reset: failed to delete user");
                "user deletion failed".to_string()
            })
    }

    pub async fn grant_e2e_admin_role(&self, email: &str) -> Result<(), String> {
        let repo = self
            .postgres_repo()
            .ok_or_else(|| "database not available".to_string())?;
        let mut tx = repo.raw().begin().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E grant: failed to begin transaction");
            "admin role grant failed".to_string()
        })?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E grant: failed to set super_admin role");
                "admin role grant failed".to_string()
            })?;
        let user_id: Option<Uuid> = sqlx::query_scalar(
            "select id from users where email = $1 limit 1",
        )
        .bind(email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E grant: database error during user lookup");
            "admin role grant failed".to_string()
        })?;
        let user_id = user_id.ok_or_else(|| "user not found".to_string())?;
        sqlx::query(
            r#"
            update users
            set role = 'super_admin',
                auth_version = case
                    when role is distinct from 'super_admin' then auth_version + 1
                    else auth_version
                end
            where id = $1
            "#,
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E grant: failed to update user role");
            "admin role grant failed".to_string()
        })?;
        tx.commit().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E grant: failed to commit transaction");
            "admin role grant failed".to_string()
        })
    }

    /// Place `member_email` into `owner_email`'s organization for invite-accept E2E.
    pub async fn ensure_e2e_org_member(
        &self,
        owner_email: &str,
        member_email: &str,
        password: &str,
        full_name: &str,
    ) -> Result<(), String> {
        use app_core::{PUBLISHED_PRIVACY_VERSION, PUBLISHED_TERMS_VERSION};
        use bcrypt::{DEFAULT_COST, hash};

        let repo = self
            .postgres_repo()
            .ok_or_else(|| "database not available".to_string())?;
        // Forced RLS: all cross-user lookups/inserts must use super_admin.
        let mut lookup_tx = repo.raw().begin().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E collaborator: begin lookup tx failed");
            "member provisioning failed".to_string()
        })?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *lookup_tx)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E collaborator: set super_admin failed");
                "member provisioning failed".to_string()
            })?;
        // Ensure owner exists (personal account). Member is a separate personal user.
        let owner_exists: bool = sqlx::query_scalar(
            "select exists(select 1 from users where email = $1)",
        )
        .bind(owner_email)
        .fetch_one(&mut *lookup_tx)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E collaborator: owner lookup failed");
            "owner lookup failed".to_string()
        })?;
        if !owner_exists {
            let _ = lookup_tx.rollback().await;
            return Err("owner not found".to_string());
        }

        let existing_member: Option<(Uuid,)> =
            sqlx::query_as::<_, (Uuid,)>("select id from users where email = $1 limit 1")
                .bind(member_email)
                .fetch_optional(&mut *lookup_tx)
                .await
                .map_err(|error| {
                    tracing::warn!(error = %error, "E2E collaborator: member lookup failed");
                    "member lookup failed".to_string()
                })?;
        lookup_tx.commit().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E collaborator: commit lookup failed");
            "member provisioning failed".to_string()
        })?;

        if let Some((existing_id,)) = existing_member {
            // Cascade SQL must run as super_admin (function touches cross-tenant rows).
            let mut del_tx = repo.raw().begin().await.map_err(|error| {
                tracing::warn!(error = %error, "E2E collaborator: begin delete tx failed");
                "member cleanup failed".to_string()
            })?;
            sqlx::query("select set_config('app.current_role', 'super_admin', true)")
                .execute(&mut *del_tx)
                .await
                .map_err(|error| {
                    tracing::warn!(error = %error, "E2E collaborator: delete elevate failed");
                    "member cleanup failed".to_string()
                })?;
            sqlx::query("select delete_user_cascade($1)")
                .bind(existing_id)
                .execute(&mut *del_tx)
                .await
                .map_err(|error| {
                    tracing::warn!(error = %error, "E2E collaborator: failed to delete existing member");
                    "member cleanup failed".to_string()
                })?;
            del_tx.commit().await.map_err(|error| {
                tracing::warn!(error = %error, "E2E collaborator: commit delete failed");
                "member cleanup failed".to_string()
            })?;
        }

        let password_hash = hash(password, DEFAULT_COST).map_err(|error| {
            tracing::warn!(error = %error, "E2E collaborator: password hash failed");
            "password hash failed".to_string()
        })?;
        let user_id = Uuid::new_v4();
        let mut tx = repo.raw().begin().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E collaborator: begin tx failed");
            "member provisioning failed".to_string()
        })?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E collaborator: set super_admin failed");
                "member provisioning failed".to_string()
            })?;
        sqlx::query(
            "insert into users (id, email, full_name, password_hash, role) values ($1, $2, $3, $4, 'user')",
        )
        .bind(user_id)
        .bind(member_email)
        .bind(full_name)
        .bind(password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E org member: insert user failed");
            "member provisioning failed".to_string()
        })?;
        sqlx::query(
            "insert into legal_acceptances (user_id, terms_version, privacy_version, context) values ($1, $2, $3, 'registration')",
        )
        .bind(user_id)
        .bind(PUBLISHED_TERMS_VERSION)
        .bind(PUBLISHED_PRIVACY_VERSION)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E org member: legal acceptance failed");
            "member provisioning failed".to_string()
        })?;
        tx.commit().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E org member: commit failed");
            "member provisioning failed".to_string()
        })
    }

    /// Checks if the JWT's auth_version matches the user's current auth_version.
    ///
    /// Returns `true` when PostgreSQL is not configured (memory mode is
    /// development-only with no security semantics).
    pub async fn jwt_auth_version_matches(
        &self,
        user_uuid: Uuid,
        org_uuid: Uuid,
        token_auth_version: i32,
    ) -> bool {
        let Some(repo) = self.postgres_repo() else {
            return true;
        };
        let Ok(mut tx) = repo.raw().begin().await else {
            return false;
        };
        let _ = sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await;
        let _ = org_uuid; // legacy JWT claim; personal account uses user id only
        let auth_version = sqlx::query_scalar::<_, i32>(
            "select auth_version from users where id = $1",
        )
        .bind(user_uuid)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();
        let _ = tx.commit().await;
        auth_version == Some(token_auth_version)
    }

    pub async fn upload_state_for_authenticated_document(
        &self,
        document_id: &str,
    ) -> Result<(Self, Option<String>), common::AppError> {
        let Some(_repo) = self.postgres_repo() else {
            return Ok((self.clone(), None));
        };
        if self.auth().actor_id().is_none() {
            return Err(common::AppError::Validation {
                code: "authenticated_user_required",
                message: "authenticated user required".to_string(),
                http_status: 401,
            });
        }
        let document_uuid = Uuid::parse_str(document_id).map_err(|_| {
            common::AppError::validation("document_not_found", "document not found")
        })?;
        let Some(store) = self.storage.document_store() else {
            return Err(common::AppError::internal(
                "document store is not configured",
            ));
        };
        let seed = store
            .get_document_task_seed(self.auth(), document_uuid)
            .await?
            .ok_or_else(|| {
                common::AppError::not_found("document_not_found", "document not found")
            })?;
        Ok((self.clone(), Some(seed.object_path)))
    }

    /// System lookup for signed uploads and object-storage webhooks.
    pub async fn upload_state_for_system_document(
        &self,
        document_id: &str,
    ) -> Result<(Self, Option<String>), common::AppError> {
        let Some(repo) = self.postgres_repo() else {
            return Ok((self.clone(), None));
        };
        let document_uuid = Uuid::parse_str(document_id).map_err(|_| {
            common::AppError::validation("document_not_found", "document not found")
        })?;
        let mut tx = repo
            .raw()
            .begin()
            .await
            .map_err(|error| common::AppError::internal(error.to_string()))?;
        let _ = sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await
            .map_err(|error| common::AppError::internal(error.to_string()))?;
        let row = sqlx::query("select owner_user_id, object_path from documents where id = $1")
            .bind(document_uuid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| common::AppError::internal(error.to_string()))?
            .ok_or_else(|| {
                common::AppError::not_found("document_not_found", "document not found")
            })?;
        tx.commit()
            .await
            .map_err(|error| common::AppError::internal(error.to_string()))?;

        let owner_user_id = row
            .try_get::<Uuid, _>("owner_user_id")
            .map_err(|error| common::AppError::internal(error.to_string()))?;
        let object_path = row
            .try_get::<String, _>("object_path")
            .map_err(|error| common::AppError::internal(error.to_string()))?;
        let mut auth = contracts::auth_runtime::AuthContext::new(owner_user_id.into(), contracts::auth_runtime::SubjectKind::System);
        if let Some(actor) = self.auth().actor_id() {
            auth = auth.with_actor_id(actor);
        }

        Ok((self.with_auth(auth), Some(object_path)))
    }
}
