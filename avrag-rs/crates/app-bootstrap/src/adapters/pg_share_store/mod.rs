use std::sync::Arc;

use app_core::{
    ShareStorePort,
    share_domain::{
        WorkspaceAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel,
        ShareAccessLogEntry, ShareAnalyticsEntry, ShareWorkspaceMember, ShareTokenSnapshot,
        SharedKnowledgeBaseSnapshot, SharedWorkspaceSnapshot, SharedShareInfoSnapshot,
        SharedSourceSnapshot,
    },
};
use async_trait::async_trait;
use contracts::auth_runtime::AuthContext;
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
// Rust 2024 forbids include! inside impl blocks; build.rs assembles shards.lst into OUT_DIR.
include!("port_impl.rs");

#[cfg(test)]
mod tests {
    use crate::adapters::port_shard_guard;

    const ADAPTER_DIR: &str = "src/adapters/pg_share_store";

    #[test]
    fn all_impl_shards_are_included() {
        port_shard_guard::assert_shards_lst_exists(ADAPTER_DIR);
        let shards = port_shard_guard::parse_shard_list(include_str!("shards.lst"));
        port_shard_guard::assert_shards_exist(ADAPTER_DIR, &shards);
        port_shard_guard::assert_port_impl_includes_out_dir(
            include_str!("port_impl.rs"),
            "pg_share_store_port_impl.rs",
        );
    }

    #[test]
    fn no_orphan_shard_files() {
        let shards = port_shard_guard::parse_shard_list(include_str!("shards.lst"));
        port_shard_guard::assert_no_orphan_rs_files(ADAPTER_DIR, &shards);
    }
}
