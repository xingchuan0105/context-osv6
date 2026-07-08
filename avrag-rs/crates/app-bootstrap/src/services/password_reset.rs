use std::str::FromStr;

use app_core::{AuthStorePort, CreatePasswordResetTicketInput};
use bcrypt::{DEFAULT_COST, hash as bcrypt_hash};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sha2::{Digest, Sha256};
use tracing::warn;
use uuid::Uuid;

const PASSWORD_RESET_PURPOSE: &str = "password_reset";
const RESET_CODE_TTL_MINUTES: i64 = 10;
const RESET_TICKET_TTL_MINUTES: i64 = 15;
const RESET_MAX_ATTEMPTS: i32 = 5;

#[derive(Clone)]
pub struct PasswordResetConfig {
    email_provider: String,
    smtp_host: String,
    smtp_port: u16,
    smtp_user: String,
    smtp_pass: String,
    smtp_from: String,
    smtp_from_name: Option<String>,
    smtp_tls: bool,
    reset_code_secret: String,
}

impl PasswordResetConfig {
    pub fn from_env() -> Self {
        Self {
            email_provider: env_first(&["EMAIL_PROVIDER"], "smtp"),
            smtp_host: env_first(&["MAIL_HOST", "SMTP_HOST"], "smtp.163.com"),
            smtp_port: env_first(&["MAIL_PORT", "SMTP_PORT"], "465")
                .parse()
                .unwrap_or(465),
            smtp_user: env_first(&["MAIL_USER", "SMTP_USER", "SMTP_USERNAME"], ""),
            smtp_pass: env_first(&["MAIL_PASS", "SMTP_PASS", "SMTP_PASSWORD"], ""),
            smtp_from: env_first(&["MAIL_FROM", "SMTP_FROM"], ""),
            smtp_from_name: non_empty(env_first(&["SMTP_FROM_NAME"], "")),
            smtp_tls: parse_bool(&env_first(&["SMTP_TLS"], "true"), true),
            reset_code_secret: env_first(
                &["RESET_CODE_SECRET"],
                "context-osv6-local-reset-secret",
            ),
        }
    }

    fn smtp_ready(&self) -> bool {
        self.email_provider.eq_ignore_ascii_case("smtp")
            && !self.smtp_host.trim().is_empty()
            && !self.smtp_from.trim().is_empty()
    }
}

fn env_first(keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| default.to_string())
}

fn parse_bool(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn generate_reset_code() -> String {
    format!("{:06}", Uuid::new_v4().as_u128() % 1_000_000)
}

fn generate_reset_ticket() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug)]
pub enum PasswordResetError {
    NotEnabled,
    StoreLookupFailed,
    TicketCreateFailed,
    EmailSendFailed,
    CodeVerifyFailed,
    TicketVerifyFailed,
    PasswordHashFailed,
    PasswordResetFailed,
    InvalidResetTicket,
}

pub struct SendResetCodeOutcome {
    pub user_id: Uuid,
    pub email: String,
    pub delivery: &'static str,
    pub reset_ticket: String,
    pub code: String,
}

pub struct VerifyResetCodeOutcome {
    pub user_id: Uuid,
    pub email: String,
    pub reset_ticket: String,
}

#[derive(Clone)]
pub struct PasswordResetService {
    config: PasswordResetConfig,
}

impl PasswordResetService {
    pub fn from_env() -> Self {
        Self {
            config: PasswordResetConfig::from_env(),
        }
    }

    pub fn smtp_ready(&self) -> bool {
        self.config.smtp_ready()
    }

    pub fn normalize_email(email: &str) -> Result<String, &'static str> {
        let trimmed = email.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Err("email is required");
        }
        Address::from_str(&trimmed).map_err(|_| "invalid email")?;
        Ok(trimmed)
    }

    fn hash_reset_value(&self, scope: &str, value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.config.reset_code_secret.as_bytes());
        hasher.update(b":");
        hasher.update(scope.as_bytes());
        hasher.update(b":");
        hasher.update(value.as_bytes());
        hex::encode(hasher.finalize())
    }

    async fn send_reset_email(
        &self,
        to: &str,
        code: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<()> {
        let from_address = Address::from_str(self.config.smtp_from.trim())?;
        let to_address = Address::from_str(to.trim())?;
        let from = Mailbox::new(self.config.smtp_from_name.clone(), from_address);
        let email = Message::builder()
            .from(from)
            .to(Mailbox::new(None, to_address))
            .subject("Context OSv6 password reset code")
            .body(format!(
                "Your password reset code is: {code}\n\nThis code expires at {}.\n",
                expires_at.to_rfc3339()
            ))?;

        let mut transport = if self.config.smtp_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.smtp_host)?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.smtp_host)
        };
        transport = transport.port(self.config.smtp_port);
        if !self.config.smtp_user.trim().is_empty() {
            transport = transport.credentials(Credentials::new(
                self.config.smtp_user.clone(),
                self.config.smtp_pass.clone(),
            ));
        }
        transport.build().send(email).await?;
        Ok(())
    }

    pub async fn verify_reset_token(
        &self,
        store: &dyn AuthStorePort,
        token: &str,
    ) -> Result<bool, PasswordResetError> {
        let ticket_hash = self.hash_reset_value(PASSWORD_RESET_PURPOSE, token.trim());
        match store.verify_reset_ticket_exists(&ticket_hash).await {
            Ok(exists) => Ok(exists),
            Err(error) => {
                warn!(error = %error, "failed to verify reset ticket");
                Err(PasswordResetError::TicketVerifyFailed)
            }
        }
    }

    pub async fn send_reset_code(
        &self,
        store: &dyn AuthStorePort,
        email: &str,
    ) -> Result<Option<SendResetCodeOutcome>, PasswordResetError> {
        let user_row = match store.find_user_by_email_for_reset(email).await {
            Ok(row) => row,
            Err(error) => {
                warn!(error = %error, "failed to resolve password reset user");
                return Err(PasswordResetError::StoreLookupFailed);
            }
        };
        let Some(user_row) = user_row else {
            return Ok(None);
        };

        let user_id = user_row.user_id;
        let org_id = user_row.org_id;
        let resolved_email = user_row.email;
        let code = generate_reset_code();
        let reset_ticket = generate_reset_ticket();
        let code_hash = self.hash_reset_value(
            PASSWORD_RESET_PURPOSE,
            &format!("{resolved_email}:{code}"),
        );
        let ticket_hash = self.hash_reset_value(PASSWORD_RESET_PURPOSE, &reset_ticket);
        let code_expires_at = chrono::Utc::now() + chrono::Duration::minutes(RESET_CODE_TTL_MINUTES);
        let ticket_expires_at =
            chrono::Utc::now() + chrono::Duration::minutes(RESET_TICKET_TTL_MINUTES);

        if let Err(error) = store
            .create_password_reset_ticket(&CreatePasswordResetTicketInput {
                org_id,
                user_id,
                email: resolved_email.clone(),
                purpose: PASSWORD_RESET_PURPOSE.to_string(),
                ticket_hash,
                code_hash,
                expires_at: ticket_expires_at,
                code_expires_at,
            })
            .await
        {
            warn!(error = %error, "failed to persist password reset ticket");
            return Err(PasswordResetError::TicketCreateFailed);
        }

        let delivery = if self.smtp_ready() { "smtp" } else { "debug" };
        if self.smtp_ready() {
            if let Err(error) = self
                .send_reset_email(&resolved_email, &code, code_expires_at)
                .await
            {
                warn!(error = %error, "failed to send reset code email");
                return Err(PasswordResetError::EmailSendFailed);
            }
        }

        Ok(Some(SendResetCodeOutcome {
            user_id,
            email: resolved_email,
            delivery,
            reset_ticket,
            code,
        }))
    }

    pub async fn verify_reset_code(
        &self,
        store: &dyn AuthStorePort,
        email: &str,
        code: &str,
    ) -> Result<Option<VerifyResetCodeOutcome>, PasswordResetError> {
        let reset_ticket = generate_reset_ticket();
        let ticket_hash = self.hash_reset_value(PASSWORD_RESET_PURPOSE, &reset_ticket);
        match store
            .verify_and_rotate_reset_code(
                email,
                PASSWORD_RESET_PURPOSE,
                code,
                &self.config.reset_code_secret,
                &ticket_hash,
                RESET_MAX_ATTEMPTS,
            )
            .await
        {
            Ok(Some((user_id, resolved_email))) => Ok(Some(VerifyResetCodeOutcome {
                user_id,
                email: resolved_email,
                reset_ticket,
            })),
            Ok(None) => Ok(None),
            Err(error) => {
                warn!(error = %error, "failed to verify reset code");
                Err(PasswordResetError::CodeVerifyFailed)
            }
        }
    }

    pub async fn confirm_reset_password(
        &self,
        store: &dyn AuthStorePort,
        ticket: &str,
        new_password: &str,
    ) -> Result<Uuid, PasswordResetError> {
        let ticket_hash = self.hash_reset_value(PASSWORD_RESET_PURPOSE, ticket);
        let password_hash = match bcrypt_hash(new_password, DEFAULT_COST) {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, "password hashing failed");
                return Err(PasswordResetError::PasswordHashFailed);
            }
        };
        match store
            .reset_password_with_ticket_hash(&ticket_hash, PASSWORD_RESET_PURPOSE, &password_hash)
            .await
        {
            Ok(user_id) => Ok(user_id),
            Err(error) if error.http_status() == 400 => {
                Err(PasswordResetError::InvalidResetTicket)
            }
            Err(error) => {
                warn!(error = %error, "failed to reset password");
                Err(PasswordResetError::PasswordResetFailed)
            }
        }
    }
}
