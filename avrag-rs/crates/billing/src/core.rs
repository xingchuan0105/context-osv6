use anyhow::{Result, anyhow, bail};
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use common::{OrgId, UserId};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::stripe_client::StripeClient;
use crate::types::{
    ADMIN_ROLE_SUPER, BillingConfig, ExistingSubscriptionFields, PLAN_ENTERPRISE, PLAN_FREE,
    PLAN_PRO, STATUS_ACTIVE, STATUS_CANCELED, STATUS_PAST_DUE, STATUS_UNPAID,
    StripeSubscriptionSnapshot, Subscription, WebhookClaim,
};

include!("core_usage.rs");
include!("core_webhooks.rs");
include!("core_support.rs");
