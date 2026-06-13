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
    notebook_id: Option<Uuid>,
    permissions: BTreeSet<String>,
    request_id: Option<String>,
}

impl AuthContext {
    pub fn new(org_id: OrgId, subject_kind: SubjectKind) -> Self {
        Self {
            org_id,
            actor_id: None,
            subject_kind,
            notebook_id: None,
            permissions: BTreeSet::new(),
            request_id: None,
        }
    }

    pub fn with_actor_id(mut self, actor_id: ActorId) -> Self {
        self.actor_id = Some(actor_id);
        self
    }

    pub fn with_notebook_scope(mut self, notebook_id: Uuid) -> Self {
        self.notebook_id = Some(notebook_id);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
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

    pub fn notebook_id(&self) -> Option<Uuid> {
        self.notebook_id
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
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

    pub fn ensure_notebook_scope(&self, notebook_id: Uuid) -> Result<(), AuthError> {
        match self.notebook_id {
            Some(expected) if expected == notebook_id => Ok(()),
            Some(expected) => Err(AuthError::NotebookScopeMismatch {
                expected,
                actual: notebook_id,
            }),
            None => Err(AuthError::MissingNotebookScope),
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("missing org scope")]
    MissingOrgScope,
    #[error("missing notebook scope")]
    MissingNotebookScope,
    #[error("missing permission: {permission}")]
    MissingPermission { permission: String },
    #[error("resource belongs to a different organization")]
    CrossTenantAccess,
    #[error("notebook scope mismatch: expected {expected}, got {actual}")]
    NotebookScopeMismatch { expected: Uuid, actual: Uuid },
}

pub fn ensure_same_org(context: &AuthContext, resource_org_id: OrgId) -> Result<(), AuthError> {
    if context.org_id() == resource_org_id {
        return Ok(());
    }

    Err(AuthError::CrossTenantAccess)
}
