//! Auth API client

use crate::{ApiClient, dtos::*};

impl ApiClient {
    /// POST /api/auth/register
    pub async fn register(&self, req: &RegisterRequest) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/register", req).await
    }

    /// POST /api/auth/login
    pub async fn login(&self, req: &LoginRequest) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/login", req).await
    }

    /// POST /api/auth/logout
    pub async fn logout(&self) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/logout", &EmptyResponse {}).await
    }

    /// GET /api/auth/me
    pub async fn me(&self) -> anyhow::Result<AuthEnvelope> {
        self.get("/api/auth/me").await
    }

    /// PUT /api/auth/profile
    pub async fn update_profile(&self, full_name: Option<String>) -> anyhow::Result<AuthEnvelope> {
        #[derive(serde::Serialize)]
        struct Body {
            full_name: Option<String>,
        }
        self.put("/api/auth/profile", &Body { full_name }).await
    }

    /// GET /api/auth/preferences
    pub async fn get_user_preferences(&self) -> anyhow::Result<UserPreferences> {
        self.get("/api/auth/preferences").await
    }

    /// PUT /api/auth/preferences
    pub async fn update_user_preferences(
        &self,
        preferences: &UserPreferences,
    ) -> anyhow::Result<UserPreferences> {
        self.put("/api/auth/preferences", preferences).await
    }

    /// POST /api/auth/change-password
    pub async fn change_password(
        &self,
        req: &ChangePasswordRequest,
    ) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/change-password", req).await
    }

    /// POST /api/auth/reset/send-code
    pub async fn send_reset_code(
        &self,
        req: &SendResetCodeRequest,
    ) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/reset/send-code", req).await
    }

    /// POST /api/auth/reset/verify-code
    pub async fn verify_reset_code(
        &self,
        req: &VerifyResetCodeRequest,
    ) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/reset/verify-code", req).await
    }

    /// POST /api/auth/reset/confirm
    pub async fn confirm_reset_password(
        &self,
        req: &ConfirmResetPasswordRequest,
    ) -> anyhow::Result<AuthEnvelope> {
        self.post("/api/auth/reset/confirm", req).await
    }
}
