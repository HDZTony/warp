//! Minimal auth client stub for `wormhole-slim` embed builds.

use std::result::Result as StdResult;
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use warp_core::errors::{AnyhowErrorExt as _, ErrorExt, register_error};
use warp_graphql::mutations::create_anonymous_user::{
    AnonymousUserType, CreateAnonymousUserResult,
};
use warp_graphql::mutations::expire_api_key::ExpireApiKeyResult;
use warp_graphql::mutations::generate_api_key::GenerateApiKeyResult;
use warp_graphql::mutations::update_user_settings::UpdateUserSettingsInput;
use warp_graphql::queries::api_keys::ApiKeyProperties;
use warp_graphql::queries::get_user::UserOutput as GqlUserOutput;

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct SyncedUserSettings {
    pub is_crash_reporting_enabled: bool,
    pub is_telemetry_enabled: bool,
    pub is_cloud_conversation_storage_enabled: bool,
}

#[derive(Error, Debug)]
pub enum UserAuthenticationError {
    #[error("Firebase returned a token error when fetching an ID token")]
    DeniedAccessToken(String),
    #[error("Firebase returned a user error when fetching an ID token")]
    UserAccountDisabled(String),
    #[error("invalid state parameter")]
    InvalidStateParameter,
    #[error("missing state parameter")]
    MissingStateParameter,
    #[error("unexpected error occurred when fetching an ID token: {0:#}")]
    Unexpected(#[from] anyhow::Error),
}

impl ErrorExt for UserAuthenticationError {
    fn is_actionable(&self) -> bool {
        match self {
            Self::DeniedAccessToken(_) | Self::UserAccountDisabled(_) => false,
            Self::Unexpected(error) => error.is_actionable(),
            Self::InvalidStateParameter | Self::MissingStateParameter => true,
        }
    }
}

register_error!(UserAuthenticationError);

#[derive(Error, Debug)]
pub enum MintCustomTokenError {
    #[error("mint custom token failed")]
    Failed,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct AgentIdentity {
    pub uid: String,
    pub name: String,
    pub available: bool,
}

#[derive(Debug)]
pub struct FetchUserResult {
    pub user_output: GqlUserOutput,
    pub credentials: crate::auth::credentials::Credentials,
    pub from_refresh: bool,
}

#[derive(Debug)]
pub enum AuthEvent {
    StagingAccessBlocked,
    AccessTokenRefreshed {
        token: crate::auth::credentials::AuthToken,
    },
}

#[async_trait]
pub trait AuthClient: Send + Sync {
    async fn create_anonymous_user(
        &self,
        _referral_code: Option<String>,
        _anonymous_user_type: AnonymousUserType,
    ) -> anyhow::Result<CreateAnonymousUserResult> {
        anyhow::bail!("wormhole-slim: cloud auth unavailable")
    }

    async fn get_or_refresh_access_token(
        &self,
    ) -> anyhow::Result<crate::auth::credentials::AuthToken> {
        anyhow::bail!("wormhole-slim: cloud auth token refresh unavailable")
    }

    async fn fetch_user(
        &self,
        _token: crate::auth::credentials::LoginToken,
        _for_refresh: bool,
    ) -> StdResult<FetchUserResult, UserAuthenticationError> {
        Err(UserAuthenticationError::Unexpected(anyhow::anyhow!(
            "wormhole-slim: cloud auth unavailable"
        )))
    }

    async fn fetch_new_custom_token(&self) -> anyhow::Result<MintCustomTokenResult> {
        anyhow::bail!("wormhole-slim: cloud auth unavailable")
    }

    fn on_custom_token_fetched(
        &self,
        _response: anyhow::Result<MintCustomTokenResult>,
    ) -> StdResult<String, MintCustomTokenError> {
        Err(MintCustomTokenError::Failed)
    }

    async fn fetch_user_properties<'a>(
        &self,
        _auth_token: Option<&'a str>,
    ) -> anyhow::Result<GqlUserOutput> {
        anyhow::bail!("wormhole-slim: cloud auth unavailable")
    }

    async fn get_user_settings(&self) -> anyhow::Result<Option<SyncedUserSettings>> {
        Ok(None)
    }

    async fn set_is_telemetry_enabled(&self, _value: bool) -> anyhow::Result<()> {
        Ok(())
    }

    async fn set_is_crash_reporting_enabled(&self, _value: bool) -> anyhow::Result<()> {
        Ok(())
    }

    async fn set_is_cloud_conversation_storage_enabled(&self, _value: bool) -> anyhow::Result<()> {
        Ok(())
    }

    async fn update_user_settings(&self, _input: UpdateUserSettingsInput) -> anyhow::Result<()> {
        Ok(())
    }

    async fn set_user_is_onboarded(&self) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn request_device_code(
        &self,
    ) -> StdResult<oauth2::StandardDeviceAuthorizationResponse, UserAuthenticationError> {
        Err(UserAuthenticationError::Unexpected(anyhow::anyhow!(
            "wormhole-slim: device authorization unavailable"
        )))
    }

    async fn exchange_device_access_token(
        &self,
        _details: &oauth2::StandardDeviceAuthorizationResponse,
        _timeout: Duration,
    ) -> StdResult<crate::auth::credentials::FirebaseToken, UserAuthenticationError> {
        Err(UserAuthenticationError::Unexpected(anyhow::anyhow!(
            "wormhole-slim: device authorization unavailable"
        )))
    }

    async fn list_api_keys(&self) -> anyhow::Result<Vec<ApiKeyProperties>> {
        Ok(Vec::new())
    }

    async fn create_api_key(
        &self,
        _name: String,
        _team_id: Option<cynic::Id>,
        _agent_uid: Option<cynic::Id>,
        _expires_at: Option<warp_graphql::scalars::Time>,
    ) -> anyhow::Result<GenerateApiKeyResult> {
        anyhow::bail!("wormhole-slim: cloud API keys unavailable")
    }

    async fn expire_api_key(&self, _uid: &str) -> anyhow::Result<ExpireApiKeyResult> {
        anyhow::bail!("wormhole-slim: cloud API keys unavailable")
    }

    async fn list_agent_identities(&self) -> anyhow::Result<Vec<AgentIdentity>> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug)]
pub enum MintCustomTokenResult {
    Success { custom_token: String },
    Unknown,
}

#[derive(Debug, Default)]
pub struct StubAuthClient;

#[async_trait]
impl AuthClient for StubAuthClient {}
