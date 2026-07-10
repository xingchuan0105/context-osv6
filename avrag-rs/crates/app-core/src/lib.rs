pub mod adapters;
pub mod admin_domain;
pub mod admin_store;
pub mod analytics_context;
pub mod api_key;
pub mod auth_scope;
pub mod auth_store;
pub mod billing_domain;
pub mod billing_quota;
pub mod billing_store;
pub mod billing_usage_units;
pub mod chat_persistence;
pub mod config;
mod config_helpers;
pub mod document_store;
pub mod domain_ports;
pub mod domain_rows;
pub mod legal_versions;
pub mod object_store_port;
pub mod ports;
pub mod postgres_health;
pub mod prompt_loader;
pub mod share_domain;
pub mod share_store;
pub mod state_types;
pub mod storage_context;
pub mod util;

pub use adapters::{
    MemoryAdminStore, MemoryBillingQuotaPort, MemoryChatPersistence, MemoryDocumentStore,
    MemoryWorkspaceStore,
};
pub use admin_domain::{
    AdminAuditLogEntry, AdminAuditLogPage, AdminAuditLogQuery, AdminBillingOverview,
    AdminDegradationStatus, AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminAccountInfo,
    AdminRagHealthStatus, AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
    admin_audit_logs_to_csv, admin_audit_window_start, admin_clamp_audit_per_page,
    admin_clamp_account_list_per_page, admin_escape_ilike_pattern, admin_usage_period_start,
};
pub use admin_store::AdminStorePort;
pub use analytics_context::*;
pub use api_key::{
    MemoryApiKeyRecord, deactivate_memory_api_key, hash_api_key, register_memory_api_key,
    validate_memory_api_key,
};
pub use auth_scope::{current_owner_user_id, current_user_id};
pub use auth_store::{
    AuthStorePort, AuthUserCredentials, AuthUserProfile, CreatePasswordResetTicketInput,
    PasswordResetUser, RecordLegalAcceptanceInput, RegisterLegalAcceptance, RegisterUserInput,
    RegisterUserResult, UserLegalStatus,
};
pub use billing_domain::{
    ADMIN_ROLE_SUPER, BillableFeature, BillingConfig, BillingEvent, BillingPlan, BillingPlanQuota,
    BillingProvider, DailyUsage, ExistingSubscriptionFields, LimitHits, MeteringContext, PLAN_FREE,
    PLAN_PLUS, PLAN_PRO, STATUS_ACTIVE, STATUS_CANCELED, STATUS_PAST_DUE, STATUS_UNPAID,
    StripeSubscriptionSnapshot, Subscription, SubscriptionStatus, UsageForecastResponse,
    UsageHistoryResponse, UsageSource, UsageWindowBucket, UsageWindowResponse, WebhookClaim,
};
pub use billing_quota::BillingQuotaPort;
pub use billing_store::{
    BillingStorePort, UsageExportJobRow, UsageLimitOverrideRow, UsageLimitPlanPolicyRow,
    UsageLimitStorePort, UsageLimitUsageRecord,
};
pub use billing_usage_units::{compute_usage_units, compute_usage_units_with_rates};
pub use chat_persistence::{
    AppendChatTurn, ChatCatalogPort, ChatContentPort, ChatPersistencePort, ChatSideEffectPort,
    MessagePort, ProfilePort, SessionPort,
};
pub use config::*;
pub use document_store::DocumentStorePort;
pub use domain_ports::*;
pub use domain_rows::{
    ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, DocumentDeletionOutcome,
    DocumentScopeState, DocumentTaskSeed, DocumentUploadMutationOutcome,
    DocumentUploadQueueOutcome, IndexedChunk, MultimodalChunkRow, NotificationCreateParams,
    UserProfileRow,
};
pub use legal_versions::{
    PUBLISHED_PRIVACY_VERSION, PUBLISHED_TERMS_VERSION, validate_published_legal_versions,
};
pub use share_domain::{
    WorkspaceAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel, ShareAccessLogEntry,
    ShareAnalyticsEntry, ShareWorkspaceMember, ShareSettingsSnapshot, ShareTokenSnapshot,
    SharedKnowledgeBaseSnapshot, SharedWorkspaceSnapshot, SharedShareInfoSnapshot,
    SharedSourceSnapshot,
};
pub use share_store::ShareStorePort;
pub use state_types::{MemoryState, RetrievedContext, StoredDocument};
pub use storage_context::{
    MemoryStateHandles, ObjectStoreConfig, StorageContext, StorageContextParts, StorageInfra,
    StorageStores,
};

pub use config_helpers::parse_uuid_or_app_error;
pub use object_store_port::{
    ObjectStoreHeadError, ObjectStoreMetadata, ObjectStorePort, ObjectStoreUploadStream,
};
pub use postgres_health::PostgresHealthPort;
pub use prompt_loader::load_prompt_template;
