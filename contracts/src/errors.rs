use typeshare::typeshare;
use serde::{Deserialize, Serialize};

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: String,
    pub message: String,
}
