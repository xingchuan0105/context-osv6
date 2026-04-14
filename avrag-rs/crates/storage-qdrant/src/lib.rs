use async_trait::async_trait;
use avrag_auth::{AuthContext, AuthError, OrgId};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

include!("models.rs");
include!("http_backend.rs");
include!("tests_impl.rs");
