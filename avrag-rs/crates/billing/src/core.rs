use anyhow::{Result, anyhow, bail};
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use common::UserId;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::stripe_client::StripeClient;
use crate::types::{
    ADMIN_ROLE_SUPER, BillingConfig, BillingProvider, DailyUsage, ExistingSubscriptionFields,
    LimitHits, PLAN_FREE, PLAN_PLUS, PLAN_PRO, STATUS_ACTIVE, STATUS_CANCELED, STATUS_PAST_DUE,
    STATUS_UNPAID, StripeSubscriptionSnapshot, Subscription, SubscriptionStatus,
    UsageHistoryResponse, UsageWindowBucket, UsageWindowResponse, WebhookClaim,
};

include!("core_usage.rs");
include!("core_webhooks.rs");
include!("core_support.rs");
