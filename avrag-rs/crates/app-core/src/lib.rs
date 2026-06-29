pub mod adapters;
pub mod api_key;
pub mod admin_domain;
pub mod admin_store;
pub mod auth_store;
pub mod billing_domain;
pub mod billing_store;
pub mod billing_usage_units;
pub mod share_domain;
pub mod share_store;
pub mod analytics_context;
pub mod billing_quota;
pub mod chat_persistence;
pub mod config;
mod config_helpers;
pub mod domain_ports;
pub mod domain_rows;
pub mod document_store;
pub mod legal_versions;
pub mod object_store_port;
pub mod postgres_health;
pub mod ports;
pub mod prompt_loader;
pub mod state_types;
pub mod storage_context;
pub mod util;

pub use analytics_context::*;
pub use config::*;
pub use domain_ports::*;
pub use state_types::{MemoryState, RetrievedContext, StoredDocument};
pub use admin_domain::{
    admin_audit_logs_to_csv, admin_audit_window_start, admin_clamp_audit_per_page,
    admin_clamp_org_list_per_page, admin_escape_ilike_pattern, admin_usage_period_start,
    AdminAuditLogEntry, AdminAuditLogPage, AdminAuditLogQuery,
    AdminBillingOverview, AdminDegradationStatus, AdminFeatureFlagChangeRequest,
    AdminFeatureFlagEntry, AdminOrgInfo, AdminRagHealthStatus, AdminUsageStats, AdminUserInfo,
    AdminWorkerStatus,
};
pub use admin_store::AdminStorePort;
pub use auth_store::{
    AuthStorePort, AuthUserCredentials, AuthUserProfile, CreatePasswordResetTicketInput,
    PasswordResetUser, RecordLegalAcceptanceInput, RegisterLegalAcceptance, RegisterUserInput,
    RegisterUserResult, UserLegalStatus,
};
pub use legal_versions::{
    validate_published_legal_versions, PUBLISHED_PRIVACY_VERSION, PUBLISHED_TERMS_VERSION,
};
pub use billing_quota::BillingQuotaPort;
pub use billing_domain::{
    BillableFeature, BillingConfig, BillingEvent, BillingPlan, BillingPlanQuota, BillingProvider,
    DailyUsage, ExistingSubscriptionFields, LimitHits, MeteringContext, StripeSubscriptionSnapshot,
    Subscription, SubscriptionStatus, UsageForecastResponse, UsageHistoryResponse, UsageSource,
    UsageWindowBucket, UsageWindowResponse, WebhookClaim,
    ADMIN_ROLE_SUPER, PLAN_FREE, PLAN_PLUS, PLAN_PRO, STATUS_ACTIVE, STATUS_CANCELED,
    STATUS_PAST_DUE, STATUS_UNPAID,
};
pub use billing_store::{
    BillingStorePort, UsageLimitOverrideRow, UsageLimitPlanPolicyRow, UsageLimitStorePort,
    UsageLimitUsageRecord,
};
pub use billing_usage_units::{compute_usage_units, compute_usage_units_with_rates};
pub use chat_persistence::{AppendChatTurn, ChatPersistencePort};
pub use share_domain::{
    NotebookAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel,
    ShareAccessLogEntry, ShareAnalyticsEntry, ShareNotebookMember, ShareSettingsSnapshot,
    ShareTokenSnapshot, SharedKnowledgeBaseSnapshot, SharedNotebookSnapshot,
    SharedShareInfoSnapshot, SharedSourceSnapshot,
};
pub use share_store::ShareStorePort;
pub use document_store::DocumentStorePort;
pub use domain_rows::{
    DocumentAssetRow, DocumentDeletionOutcome, DocumentScopeState, DocumentTaskSeed,
    DocumentUploadMutationOutcome, DocumentUploadQueueOutcome, IndexedChunk, MultimodalChunkRow,
    NotificationCreateParams, ConversationHistoryHit, ConversationHistoryScope, UserProfileRow,
};
pub use api_key::{
    deactivate_memory_api_key, hash_api_key, register_memory_api_key, validate_memory_api_key,
    MemoryApiKeyRecord,
};
pub use storage_context::StorageContext;

pub use prompt_loader::load_prompt_template;
pub use config_helpers::parse_uuid_or_app_error;
pub use object_store_port::{
    ObjectStoreHeadError, ObjectStoreMetadata, ObjectStorePort, ObjectStoreUploadStream,
};
pub use postgres_health::PostgresHealthPort;
