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
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
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
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
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
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
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
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
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
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
        };
        avrag_billing::handle_get_usage_history(store, UserId::from(actor_id.into_uuid()), days).await
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
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
        };
        avrag_billing::handle_get_usage_forecast(store, UserId::from(actor_id.into_uuid())).await
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
        avrag_billing::handle_create_checkout(store, UserId::from(actor_id.into_uuid()), body).await
    }

    pub async fn billing_create_portal(&self) -> ApiResponse<avrag_billing::PortalResponse> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            );
        };
        let Some(actor_id) = self.auth().actor_id() else {
            return ApiResponse::err(
                "authenticated_user_required",
                "authenticated user required",
            );
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
        let user_id: Option<Uuid> =
            match sqlx::query_as::<_, (Option<Uuid>,)>("select id from users where email = $1 limit 1")
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
        repo.delete_user_cascade(self.auth(), user_id)
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
        let mut tx = repo
            .raw()
            .begin()
            .await
            .map_err(|error| {
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
        sqlx::query("update users set role = 'super_admin' where id = $1 and org_id = $2")
            .bind(user_id)
            .bind(org_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E grant: failed to update user role");
                "admin role grant failed".to_string()
            })?;
        tx.commit()
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "E2E grant: failed to commit transaction");
                "admin role grant failed".to_string()
            })
    }

    /// Checks if the JWT's auth_version matches the user's current auth_version.
    ///
    /// Returns `false` when PostgreSQL is not configured unless
    /// `AVRAG_AUTH_VERSION_BYPASS=true` is set explicitly for local development.
    pub async fn jwt_auth_version_matches(
        &self,
        user_uuid: Uuid,
        org_uuid: Uuid,
        token_auth_version: i32,
    ) -> bool {
        let Some(repo) = self.postgres_repo() else {
            return std::env::var("AVRAG_AUTH_VERSION_BYPASS")
                .ok()
                .is_some_and(|value| matches!(value.as_str(), "true" | "1"));
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
        let mut auth =
            avrag_auth::AuthContext::new(org_id.into(), avrag_auth::SubjectKind::System);
        if let Some(actor) = self.auth().actor_id() {
            auth = auth.with_actor_id(actor);
        }

        Ok((self.with_auth(auth), Some(object_path)))
    }
}
