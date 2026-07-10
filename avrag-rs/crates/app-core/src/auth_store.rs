use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::AppError;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RegisterLegalAcceptance {
    pub terms_version: String,
    pub privacy_version: String,
    pub context: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegisterUserInput {
    pub email: String,
    pub password_hash: String,
    pub full_name: Option<String>,
    pub legal_acceptance: RegisterLegalAcceptance,
}

#[derive(Debug, Clone)]
pub struct RecordLegalAcceptanceInput {
    pub user_id: Uuid,
    pub terms_version: String,
    pub privacy_version: String,
    pub context: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserLegalStatus {
    pub needs_re_acceptance: bool,
    pub accepted_terms_version: Option<String>,
    pub accepted_privacy_version: Option<String>,
    pub published_terms_version: String,
    pub published_privacy_version: String,
}

#[derive(Debug, Clone)]
pub struct RegisterUserResult {
    pub user_id: Uuid,
    pub owner_user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub auth_version: i32,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct AuthUserCredentials {
    pub user_id: Uuid,
    pub owner_user_id: Uuid,
    pub email: String,
    pub full_name: Option<String>,
    pub password_hash: Option<String>,
    pub auth_version: i32,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct AuthUserProfile {
    pub user_id: Uuid,
    pub owner_user_id: Uuid,
    pub email: String,
    pub full_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PasswordResetUser {
    pub user_id: Uuid,
    pub owner_user_id: Uuid,
    pub email: String,
}

#[derive(Debug, Clone)]
pub struct CreatePasswordResetTicketInput {
    pub owner_user_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub purpose: String,
    pub ticket_hash: String,
    pub code_hash: String,
    pub expires_at: DateTime<Utc>,
    pub code_expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AuthStorePort: Send + Sync {
    async fn register_user(
        &self,
        input: &RegisterUserInput,
    ) -> Result<RegisterUserResult, AppError>;

    /// Standalone consent for payment or re-acceptance flows (not registration).
    async fn record_legal_acceptance(
        &self,
        input: &RecordLegalAcceptanceInput,
    ) -> Result<(), AppError>;

    /// Latest acceptance vs published versions — drives re-acceptance UI.
    async fn get_user_legal_status(&self, user_id: Uuid) -> Result<UserLegalStatus, AppError>;

    /// Whether the user recorded a `payment` context acceptance at current published versions.
    async fn has_payment_legal_acceptance(&self, user_id: Uuid) -> Result<bool, AppError>;

    async fn find_user_for_login(
        &self,
        email: &str,
    ) -> Result<Option<AuthUserCredentials>, AppError>;

    async fn invalidate_session(&self, user_id: Uuid) -> Result<bool, AppError>;

    async fn get_user_profile(&self, user_id: Uuid) -> Result<Option<AuthUserProfile>, AppError>;

    async fn update_user_profile(
        &self,
        user_id: Uuid,
        full_name: &str,
    ) -> Result<Option<AuthUserProfile>, AppError>;

    async fn get_password_hash(&self, user_id: Uuid) -> Result<Option<String>, AppError>;

    async fn change_password(&self, user_id: Uuid, password_hash: &str) -> Result<(), AppError>;

    async fn find_user_by_email_for_reset(
        &self,
        email: &str,
    ) -> Result<Option<PasswordResetUser>, AppError>;

    async fn create_password_reset_ticket(
        &self,
        input: &CreatePasswordResetTicketInput,
    ) -> Result<(), AppError>;

    async fn verify_reset_ticket_exists(&self, ticket_hash: &str) -> Result<bool, AppError>;

    async fn verify_and_rotate_reset_code(
        &self,
        email: &str,
        purpose: &str,
        code: &str,
        reset_code_secret: &str,
        new_ticket_hash: &str,
        max_attempts: i32,
    ) -> Result<Option<(Uuid, String)>, AppError>;

    async fn reset_password_with_ticket_hash(
        &self,
        ticket_hash: &str,
        purpose: &str,
        password_hash: &str,
    ) -> Result<Uuid, AppError>;
}
