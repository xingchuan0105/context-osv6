use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum AppError {
    #[error("{message}")]
    Validation {
        code: &'static str,
        message: String,
        http_status: u16,
    },
    #[error("{message}")]
    NotFound {
        code: &'static str,
        message: String,
        http_status: u16,
    },
    #[error("{message}")]
    Conflict {
        code: &'static str,
        message: String,
        http_status: u16,
    },
    #[error("{message}")]
    Internal {
        code: &'static str,
        message: String,
        http_status: u16,
    },
    #[error("{message}")]
    RateLimited {
        code: &'static str,
        message: String,
        http_status: u16,
        retry_after_secs: u64,
    },
}

impl AppError {
    pub fn validation(code: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            code,
            message: message.into(),
            http_status: 400,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Validation {
            code: "unauthorized",
            message: message.into(),
            http_status: 401,
        }
    }

    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            code,
            message: message.into(),
            http_status: 403,
        }
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::NotFound {
            code,
            message: message.into(),
            http_status: 404,
        }
    }

    /// HTTP 410 Gone — permanent removal of a product surface (e.g. deprecated API).
    pub fn gone(code: &'static str, message: impl Into<String>) -> Self {
        Self::NotFound {
            code,
            message: message.into(),
            http_status: 410,
        }
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: message.into(),
            http_status: 409,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            code: "internal_error",
            message: message.into(),
            http_status: 500,
        }
    }

    pub fn rate_limited(
        code: &'static str,
        message: impl Into<String>,
        retry_after_secs: u64,
    ) -> Self {
        Self::RateLimited {
            code,
            message: message.into(),
            http_status: 429,
            retry_after_secs,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation { code, .. }
            | Self::NotFound { code, .. }
            | Self::Conflict { code, .. }
            | Self::Internal { code, .. }
            | Self::RateLimited { code, .. } => code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Validation { message, .. }
            | Self::NotFound { message, .. }
            | Self::Conflict { message, .. }
            | Self::Internal { message, .. }
            | Self::RateLimited { message, .. } => message,
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::Validation { http_status, .. }
            | Self::NotFound { http_status, .. }
            | Self::Conflict { http_status, .. }
            | Self::Internal { http_status, .. }
            | Self::RateLimited { http_status, .. } => *http_status,
        }
    }

    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            Self::RateLimited {
                retry_after_secs, ..
            } => Some(*retry_after_secs),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub error: String,
    pub http_status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    #[serde(default)]
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            data: Some(data),
            error: None,
            ok: true,
        }
    }

    pub fn err(code: &str, message: &str) -> Self {
        Self {
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.to_string(),
            }),
            ok: false,
        }
    }
}
