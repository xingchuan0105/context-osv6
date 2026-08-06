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
pub mod referral_domain;
pub mod referral_store;
pub mod wallet_domain;
pub mod wallet_store;
pub mod byok_crypto;
pub mod provider_secret_domain;
pub mod provider_secret_store;
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
    AdminAccountInfo, AdminAuditLogEntry, AdminAuditLogPage, AdminAuditLogQuery,
    AdminBillingOverview, AdminDegradationStatus, AdminFeatureFlagChangeRequest,
    AdminFeatureFlagEntry, AdminRagHealthStatus, AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
    admin_audit_logs_to_csv, admin_audit_window_start, admin_clamp_account_list_per_page,
    admin_clamp_audit_per_page, admin_escape_ilike_pattern, admin_usage_period_start,
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
    BillingProvider, DailyUsage, LimitHits, MeteringContext, ANNUAL_PRICE_MONTHS, PLAN_FREE,
    PLAN_PLUS, PLAN_PLUS_ANNUAL, PLAN_PRO, PLAN_PRO_ANNUAL, ProviderEvent, STATUS_ACTIVE,
    STATUS_CANCELED, STATUS_PAST_DUE,
    STATUS_UNPAID, Subscription, SubscriptionStatus, UsageForecastResponse,
    UsageHistoryResponse, UsageSource, UsageWindowBucket, UsageWindowResponse, WebhookClaim,
};
pub use billing_quota::BillingQuotaPort;
pub use billing_store::{
    BillingStorePort, UsageExportJobRow, UsageLimitOverrideRow, UsageLimitPlanPolicyRow,
    UsageLimitStorePort, UsageLimitUsageRecord,
};
pub use referral_domain::{
    REFERRAL_REJECT_CODE_INVALID, REFERRAL_REJECT_CODE_REVOKED, REFERRAL_REJECT_DAILY_CAP,
    REFERRAL_REJECT_QUOTA_EXHAUSTED, REFERRAL_REJECT_SELF_INVITE, REFERRAL_STATUS_PENDING,
    REFERRAL_STATUS_REJECTED,
    REFERRAL_STATUS_REWARDED, Referral, ReferralCode, ReferralStats, generate_referral_code,
    normalize_referral_code,
};
pub use referral_store::{InsertPendingResult, ReferralStorePort};
pub use wallet_domain::{
    ApplyLedgerInput, ApplyLedgerResult, CHECKOUT_KIND_SUBSCRIPTION, CHECKOUT_KIND_WALLET_TOPUP,
    DEFAULT_TOPUP_PACKS, PRODUCT_KIND_SUBSCRIPTION, PRODUCT_KIND_WALLET_TOPUP, REFERRAL_BASE_QUOTA,
    REFERRAL_BONUS_FEN, REFERRAL_TOPUP_STEP_FEN, SIGNUP_GRANT_FEN, TOPUP_PACK_50, TOPUP_PACK_100,
    TOPUP_PACK_200, TopupPack, WALLET_KIND_REFERRAL_BONUS, WALLET_KIND_SIGNUP_GRANT,
    WALLET_KIND_TOPUP, WALLET_KIND_USAGE_DEBIT, Wallet, WalletLedgerEntry, fen_to_decimal_amount,
    referral_bonus_invitee_idempotency_key, referral_bonus_inviter_idempotency_key, referral_quota,
    signup_grant_idempotency_key, topup_idempotency_key, topup_pack_by_id,
};
pub use wallet_store::WalletStorePort;
pub use byok_crypto::{BYOK_KEY_LEN, BYOK_NONCE_LEN, ByokMasterKey};
pub use provider_secret_domain::{
    ProviderSecretPurpose, ProviderSecretView, ResolvedProviderSecret, UpsertProviderSecretInput,
    key_fingerprint,
};
pub use provider_secret_store::ProviderSecretStorePort;
pub use billing_usage_units::{
    compute_usage_units, compute_usage_units_three_bucket, compute_usage_units_with_rates,
    tokens_approx_from_units,
};
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
    PublicShareChatContextSnapshot, ShareAccessLevel, ShareAccessLogEntry, ShareAnalyticsEntry,
    ShareSettingsSnapshot, ShareTokenSnapshot, ShareWorkspaceMember, SharedKnowledgeBaseSnapshot,
    SharedShareInfoSnapshot, SharedSourceSnapshot, SharedWorkspaceSnapshot,
    WorkspaceAccessSnapshot,
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
