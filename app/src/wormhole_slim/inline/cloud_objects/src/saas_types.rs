//! Local stand-ins for removed Warp GraphQL SaaS types (`wormhole-slim`).

use serde::{Deserialize, Serialize};

use crate::ids::ServerId;
pub use warp_server_auth::{OwnerType, ServerTimestamp};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum AccessLevel {
    Viewer,
    Editor,
    Full,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MCPTemplateVariable {
    pub key: String,
    pub allowed_values: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MCPJsonTemplate {
    pub json: String,
    pub variables: Vec<MCPTemplateVariable>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MCPGalleryTemplate {
    pub description: String,
    pub gallery_item_id: String,
    pub instructions_in_markdown: Option<String>,
    pub json_template: MCPJsonTemplate,
    pub template: String,
    pub title: String,
    pub version: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdatedObjectInput {
    pub uid: ServerId,
    pub revision_ts: Option<ServerTimestamp>,
    pub metadata_ts: Option<ServerTimestamp>,
    pub permissions_ts: Option<ServerTimestamp>,
    pub actions_ts: Option<ServerTimestamp>,
}
