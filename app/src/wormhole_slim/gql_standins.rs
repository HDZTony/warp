//! Local stand-ins for `warp_graphql` types when building with `wormhole-slim`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BonusGrantType {
    AmbientOnly,
    Any,
}

pub type Time = DateTime<Utc>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyProperties {
    pub uid: String,
    pub name: String,
    pub created_at: Time,
    pub expires_at: Option<Time>,
    pub last_used_at: Option<Time>,
}

#[derive(Clone, Debug)]
pub enum ExpireApiKeyResult {
    Success,
    Unknown,
}

#[derive(Clone, Debug)]
pub enum GenerateApiKeyResult {
    Success { api_key: String },
    Unknown,
}

pub use warp_graphql::queries::user_repo_auth_status::UserRepoAuthStatusEnum;

pub use cloud_objects::OwnerType;
pub use cloud_objects::ServerTimestamp;
