//! Runtime auth scope types shared across service crates (not wire DTOs).

use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Account owner / tenant root for personal B2C (replaces former `UserId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Uuid);

impl UserId {
    pub fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }

    pub fn uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for UserId {
    fn from(value: Uuid) -> Self {
        Self::new(value)
    }
}

impl FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self::new)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(Uuid);

impl ActorId {
    pub fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }

    pub fn uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Display for ActorId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for ActorId {
    fn from(value: Uuid) -> Self {
        Self::new(value)
    }
}

impl FromStr for ActorId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self::new)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubjectKind {
    User,
    ApiKey,
    System,
}

/// Auth scope: account owner (`user_id`) + optional actor + workspace scope.
///
/// Personal B2C: `user_id` is the resource owner used for RLS
/// (`app.current_user` / `owner_user_id`). There is no organization tenant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    user_id: UserId,
    actor_id: Option<ActorId>,
    subject_kind: SubjectKind,
    workspace_id: Option<Uuid>,
    permissions: BTreeSet<String>,
    request_id: Option<String>,
    rate_limit_rpm: Option<u32>,
}

impl AuthContext {
    pub fn new(user_id: UserId, subject_kind: SubjectKind) -> Self {
        Self {
            user_id,
            actor_id: None,
            subject_kind,
            workspace_id: None,
            permissions: BTreeSet::new(),
            request_id: None,
            rate_limit_rpm: None,
        }
    }

    pub fn with_actor_id(mut self, actor_id: ActorId) -> Self {
        self.actor_id = Some(actor_id);
        self
    }

    pub fn with_workspace_scope(mut self, workspace_id: Uuid) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_rate_limit_rpm(mut self, rate_limit_rpm: u32) -> Self {
        self.rate_limit_rpm = Some(rate_limit_rpm);
        self
    }

    pub fn grant(mut self, permission: impl Into<String>) -> Self {
        self.permissions.insert(permission.into());
        self
    }

    /// Account owner id (tenant root for RLS / `owner_user_id`).
    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Alias for resource ownership checks (same as [`Self::user_id`]).
    pub fn owner_user_id(&self) -> UserId {
        self.user_id
    }

    pub fn actor_id(&self) -> Option<ActorId> {
        self.actor_id
    }

    pub fn subject_kind(&self) -> &SubjectKind {
        &self.subject_kind
    }

    pub fn workspace_id(&self) -> Option<Uuid> {
        self.workspace_id
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    pub fn rate_limit_rpm(&self) -> Option<u32> {
        self.rate_limit_rpm
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    pub fn ensure_permission(&self, permission: &str) -> Result<(), AuthError> {
        if self.has_permission(permission) {
            return Ok(());
        }

        Err(AuthError::MissingPermission {
            permission: permission.to_owned(),
        })
    }

    pub fn ensure_workspace_scope(&self, workspace_id: Uuid) -> Result<(), AuthError> {
        match self.workspace_id {
            Some(expected) if expected == workspace_id => Ok(()),
            Some(expected) => Err(AuthError::WorkspaceScopeMismatch {
                expected,
                actual: workspace_id,
            }),
            None => Err(AuthError::MissingWorkspaceScope),
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("missing user scope")]
    MissingUserScope,
    #[error("missing workspace scope")]
    MissingWorkspaceScope,
    #[error("missing permission: {permission}")]
    MissingPermission { permission: String },
    #[error("resource belongs to a different account owner")]
    CrossTenantAccess,
    #[error("workspace scope mismatch: expected {expected}, got {actual}")]
    WorkspaceScopeMismatch { expected: Uuid, actual: Uuid },
}

pub fn ensure_same_owner(context: &AuthContext, resource_owner: UserId) -> Result<(), AuthError> {
    if context.owner_user_id() == resource_owner {
        return Ok(());
    }

    Err(AuthError::CrossTenantAccess)
}
