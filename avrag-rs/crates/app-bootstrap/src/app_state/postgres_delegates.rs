use common::{ApiResponse, UserId};
use sqlx::Row;
use uuid::Uuid;

use super::AppState;

impl AppState {
    pub async fn billing_get_plans(&self) -> ApiResponse<serde_json::Value> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_get_plans(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn billing_get_subscription(
        &self,
    ) -> ApiResponse<avrag_billing::SubscriptionResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_get_subscription(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn billing_get_usage(&self) -> ApiResponse<avrag_billing::UsageResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_get_usage(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn billing_get_usage_window(
        &self,
    ) -> ApiResponse<avrag_billing::UsageWindowResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_get_usage_window(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn billing_get_usage_history(
        &self,
        days: i32,
    ) -> ApiResponse<avrag_billing::UsageHistoryResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_get_usage_history(store, UserId::from(actor_id.into_uuid()), days)
            .await
    }

    pub async fn billing_get_usage_forecast(
        &self,
    ) -> ApiResponse<avrag_billing::UsageForecastResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_get_usage_forecast(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn billing_create_usage_export(
        &self,
        body: avrag_billing::CreateUsageExportRequest,
    ) -> ApiResponse<avrag_billing::UsageExportAccepted> {
        let Some(repo) = self.postgres_repo() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        let store: std::sync::Arc<dyn app_core::UsageLimitStorePort> =
            std::sync::Arc::new(crate::adapters::PgUsageLimitStoreAdapter::new(repo));
        let org_id = self.auth().org_id().into_uuid();
        let user_id = actor_id.into_uuid();
        let response =
            avrag_billing::handle_create_usage_export(store, org_id, user_id, body).await;
        if response.ok {
            if let Some(data) = response.data.as_ref() {
                tracing::info!(
                    target: "usage_export",
                    export_id = %data.export_id,
                    status = %data.status,
                    user_id = %user_id,
                    org_id = %org_id,
                    "usage export job created"
                );
            }
        }
        response
    }

    pub async fn billing_get_usage_export(
        &self,
        export_id: uuid::Uuid,
    ) -> ApiResponse<avrag_billing::UsageExportStatusResponse> {
        let Some(repo) = self.postgres_repo() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        let store: std::sync::Arc<dyn app_core::UsageLimitStorePort> =
            std::sync::Arc::new(crate::adapters::PgUsageLimitStoreAdapter::new(repo));
        avrag_billing::handle_get_usage_export(store, actor_id.into_uuid(), export_id).await
    }

    pub async fn billing_create_checkout(
        &self,
        body: avrag_billing::CreateCheckoutRequest,
    ) -> ApiResponse<avrag_billing::CheckoutResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err(
                "authenticated_user_required",
                "billing checkout requires an authenticated user",
            );
        };
        let user_id = UserId::from(actor_id.into_uuid());
        if let Some(auth_store) = self.auth_store() {
            match auth_store
                .has_payment_legal_acceptance(user_id.into_uuid())
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    return ApiResponse::err(
                        "consent_required",
                        "payment legal acceptance is required before checkout",
                    );
                }
                Err(error) => {
                    return ApiResponse::err(
                        "internal_error",
                        &format!("failed to verify payment legal acceptance: {error}"),
                    );
                }
            }
        }
        avrag_billing::handle_create_checkout(store, user_id, body).await
    }

    pub async fn billing_create_portal(&self) -> ApiResponse<avrag_billing::PortalResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err("authenticated_user_required", "authenticated user required");
        };
        avrag_billing::handle_create_portal(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn billing_handle_webhook(
        &self,
        provider: avrag_billing::BillingProvider,
        signature: Option<&str>,
        body: &[u8],
    ) -> common::ApiResponse<serde_json::Value> {
        let Some(store) = self.billing_store() else {
            return common::ApiResponse::err(
                "billing_unavailable",
                "billing repository unavailable",
            );
        };
        avrag_billing::handle_webhook(store, provider, signature, body).await
    }

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
        let user_row: Option<(Uuid, Uuid)> = sqlx::query_as::<_, (Uuid, Uuid)>(
            "select id, org_id from users where email = $1 limit 1",
        )
        .bind(email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E grant: database error during user lookup");
            "admin role grant failed".to_string()
        })?;
        let (user_id, org_id) = user_row.ok_or_else(|| "user not found".to_string())?;
        sqlx::query(
            r#"
            update users
            set role = 'super_admin',
                auth_version = case
                    when role is distinct from 'super_admin' then auth_version + 1
                    else auth_version
                end
            where id = $1 and org_id = $2
            "#,
        )
        .bind(user_id)
        .bind(org_id)
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
        let owner_row: Option<(Uuid, Uuid)> = sqlx::query_as::<_, (Uuid, Uuid)>(
            "select id, org_id from users where email = $1 limit 1",
        )
        .bind(owner_email)
        .fetch_optional(repo.raw())
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "E2E org member: owner lookup failed");
            "owner lookup failed".to_string()
        })?;
        let (_owner_id, org_id) = owner_row.ok_or_else(|| "owner not found".to_string())?;

        if let Some((existing_id,)) =
            sqlx::query_as::<_, (Uuid,)>("select id from users where email = $1 limit 1")
                .bind(member_email)
                .fetch_optional(repo.raw())
                .await
                .map_err(|error| {
                    tracing::warn!(error = %error, "E2E org member: member lookup failed");
                    "member lookup failed".to_string()
                })?
        {
            repo.auth().delete_user_cascade(self.auth(), existing_id)
                .await
                .map_err(|error| {
                    tracing::warn!(error = %error, "E2E org member: failed to delete existing member");
                    "member cleanup failed".to_string()
                })?;
        }

        let password_hash = hash(password, DEFAULT_COST).map_err(|error| {
            tracing::warn!(error = %error, "E2E org member: password hash failed");
            "password hash failed".to_string()
        })?;
        let user_id = Uuid::new_v4();
        let mut tx = repo.raw().begin().await.map_err(|error| {
            tracing::warn!(error = %error, "E2E org member: begin tx failed");
            "member provisioning failed".to_string()
        })?;
        sqlx::query("select set_config('app.current_role', 'super_admin', true)")
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E org member: set super_admin failed");
                "member provisioning failed".to_string()
            })?;
        sqlx::query(
            "insert into users (id, org_id, email, full_name, password_hash, role) values ($1, $2, $3, $4, $5, 'user')",
        )
        .bind(user_id)
        .bind(org_id)
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
        let auth_version = sqlx::query_scalar::<_, i32>(
            "select auth_version from users where id = $1 and org_id = $2",
        )
        .bind(user_uuid)
        .bind(org_uuid)
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
        let row = sqlx::query("select org_id, object_path from documents where id = $1")
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

        let org_id = row
            .try_get::<Uuid, _>("org_id")
            .map_err(|error| common::AppError::internal(error.to_string()))?;
        let object_path = row
            .try_get::<String, _>("object_path")
            .map_err(|error| common::AppError::internal(error.to_string()))?;
        let mut auth = contracts::auth_runtime::AuthContext::new(org_id.into(), contracts::auth_runtime::SubjectKind::System);
        if let Some(actor) = self.auth().actor_id() {
            auth = auth.with_actor_id(actor);
        }

        Ok((self.with_auth(auth), Some(object_path)))
    }
}
