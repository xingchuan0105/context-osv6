//! Test utilities for avrag-rs

use common::{OrgId, UserId};
use uuid::Uuid;

/// Generate a unique test org ID
pub fn test_org_id() -> OrgId {
    // OrgId is a type alias from avrag_auth, we need to parse from string
    "00000000-0000-0000-0000-000000000001"
        .to_string()
        .parse()
        .unwrap()
}

/// Generate a unique test user ID
pub fn test_user_id() -> UserId {
    UserId::new(Uuid::new_v4())
}

/// Generate a unique test notebook ID (as String since NotebookId doesn't exist in common)
pub fn test_notebook_id() -> String {
    Uuid::new_v4().to_string()
}

/// Test fixtures for common scenarios
pub mod fixtures {
    use super::*;

    pub fn test_org() -> OrgId {
        test_org_id()
    }

    pub fn test_user() -> UserId {
        test_user_id()
    }

    pub fn test_notebook() -> String {
        test_notebook_id()
    }
}
