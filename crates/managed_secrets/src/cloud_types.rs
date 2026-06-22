//! Local stand-ins for Warp GraphQL managed-secret types (`wormhole-slim`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ManagedSecretType {
    AnthropicApiKey,
    AnthropicBedrockAccessKey,
    AnthropicBedrockApiKey,
    Dotenvx,
    OpenaiApiKey,
    RawValue,
}

impl ManagedSecretType {
    pub fn envelope_name(&self) -> &str {
        match self {
            ManagedSecretType::AnthropicApiKey => "anthropic_api_key",
            ManagedSecretType::AnthropicBedrockAccessKey => "anthropic_bedrock_access_key",
            ManagedSecretType::AnthropicBedrockApiKey => "anthropic_bedrock_api_key",
            ManagedSecretType::Dotenvx => "dotenvx",
            ManagedSecretType::OpenaiApiKey => "openai_api_key",
            ManagedSecretType::RawValue => "raw_value",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagedSecretConfig {
    pub public_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ManagedSecretOwner {
    pub uid: String,
    pub is_team: bool,
}

#[derive(Debug, Clone)]
pub struct ManagedSecret {
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner: ManagedSecretOwner,
    pub type_: ManagedSecretType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentHarness {
    Oz,
    ClaudeCode,
    Gemini,
    Codex,
    Other(String),
}

#[derive(Debug, Clone)]
pub enum TaskManagedSecretValue {
    RawValue { value: String },
    AnthropicApiKey { api_key: String },
    AnthropicBedrockAccessKey {
        aws_access_key_id: String,
        aws_secret_access_key: String,
        aws_session_token: Option<String>,
        aws_region: String,
    },
    AnthropicBedrockApiKey {
        aws_bearer_token_bedrock: String,
        aws_region: String,
    },
    OpenaiApiKey {
        api_key: String,
        base_url: Option<String>,
    },
    Unknown,
}

pub type ManagedSecretConfigsMap = HashMap<String, ManagedSecretConfig>;
