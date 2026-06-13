use std::sync::Arc;

use async_trait::async_trait;
use app_core::{
    share_domain::{
        NotebookAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel,
        ShareAccessLogEntry, ShareAnalyticsEntry, ShareNotebookMember, ShareTokenSnapshot,
        SharedKnowledgeBaseSnapshot, SharedNotebookSnapshot, SharedShareInfoSnapshot,
        SharedSourceSnapshot,
    },
    ShareStorePort,
};
use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

use crate::adapters::pg_session::{set_current_org, set_current_role, set_public_share_token};

pub struct PgShareStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgShareStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

include!("mappers.rs");
include!("port_impl.rs");
