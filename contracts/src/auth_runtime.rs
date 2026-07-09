//! Runtime auth scope types shared across service crates (not wire DTOs).

use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrgId(Uuid);

impl OrgId {
    pub fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Display for OrgId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for OrgId {
    fn from(value: Uuid) -> Self {
        Self::new(value)
    }
}

impl FromStr for OrgId {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubjectKind {
    User,
    ApiKey,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    org_id: OrgId,
    actor_id: Option<ActorId>,
    subject_kind: SubjectKind,
    workspace_id: Option<Uuid>,
    permissions: BTreeSet<String>,
    request_id: Option<String>,
    rate_limit_rpm: Option<u32>,
}

impl AuthContext {
    pub fn new(org_id: OrgId, subject_kind: SubjectKind) -> Self {
        Self {
            org_id,
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

    pub fn org_id(&self) -> OrgId {
        self.org_id
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
    #[error("missing org scope")]
    MissingOrgScope,
    #[error("missing workspace scope")]
    MissingWorkspaceScope,
    #[error("missing permission: {permission}")]
    MissingPermission { permission: String },
    #[error("resource belongs to a different organization")]
    CrossTenantAccess,
    #[error("workspace scope mismatch: expected {expected}, got {actual}")]
    WorkspaceScopeMismatch { expected: Uuid, actual: Uuid },
}

pub fn ensure_same_org(context: &AuthContext, resource_org_id: OrgId) -> Result<(), AuthError> {
    if context.org_id() == resource_org_id {
        return Ok(());
    }

    Err(AuthError::CrossTenantAccess)
}
