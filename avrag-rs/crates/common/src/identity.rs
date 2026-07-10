//! Identity helpers. Tenant root is [`contracts::UserId`] (account owner).

pub use contracts::UserId;

/// Default owner/account id for local fixtures.
pub fn default_owner_user_id() -> String {
    "00000000-0000-0000-0000-000000000001".to_string()
}

pub fn default_user_id() -> String {
    "00000000-0000-0000-0000-000000000002".to_string()
}

pub fn default_rag_agent() -> String {
    "rag".to_string()
}
