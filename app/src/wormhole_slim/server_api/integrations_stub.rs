//! Integrations client stub for `wormhole-slim`.

use anyhow::Result;
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

pub use warp_graphql::mutations::create_simple_integration::CreateSimpleIntegrationOutput;
pub use warp_graphql::queries::get_integrations_using_environment::GetIntegrationsUsingEnvironmentOutput;
pub use warp_graphql::queries::get_oauth_connect_tx_status::OauthConnectTxStatus;
pub use warp_graphql::queries::get_simple_integrations::SimpleIntegrationsOutput;
pub use warp_graphql::queries::suggest_cloud_environment_image::SuggestCloudEnvironmentImageResult;
pub use warp_graphql::queries::user_github_info::UserGithubInfoResult;
pub use warp_graphql::queries::user_repo_auth_status::UserRepoAuthStatusOutput;

pub trait IntegrationsClientBounds: Send + Sync {}

#[cfg(not(target_family = "wasm"))]
impl<T: 'static + Send + Sync> IntegrationsClientBounds for T {}

#[cfg(target_family = "wasm")]
pub trait IntegrationsClientBounds {}

#[cfg(target_family = "wasm")]
impl<T: 'static> IntegrationsClientBounds for T {}

#[cfg_attr(test, automock)]
#[cfg_attr(target_family = "wasm", allow(dead_code))]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
pub trait IntegrationsClient: 'static + IntegrationsClientBounds {
    async fn check_user_repo_auth_status(
        &self,
        _repos: Vec<(String, String)>,
    ) -> Result<UserRepoAuthStatusOutput>;

    #[allow(clippy::too_many_arguments)]
    async fn create_or_update_simple_integration(
        &self,
        _integration_type: String,
        _is_update: bool,
        _environment_uid: Option<String>,
        _base_prompt: Option<String>,
        _model_id: Option<String>,
        _mcp_servers_json: Option<String>,
        _remove_mcp_server_names: Option<Vec<String>>,
        _worker_host: Option<String>,
        _enabled: bool,
    ) -> Result<CreateSimpleIntegrationOutput>;

    async fn list_simple_integrations(
        &self,
        _providers: Vec<String>,
    ) -> Result<SimpleIntegrationsOutput>;

    async fn poll_oauth_connect_status(&self, _tx_id: String) -> Result<OauthConnectTxStatus>;

    async fn get_integrations_using_environment(
        &self,
        _environment_id: String,
    ) -> Result<GetIntegrationsUsingEnvironmentOutput>;

    async fn get_user_github_info(&self) -> Result<UserGithubInfoResult>;

    async fn suggest_cloud_environment_image(
        &self,
        _repos: Vec<(String, String)>,
    ) -> Result<SuggestCloudEnvironmentImageResult>;
}

#[derive(Debug, Default)]
pub struct StubIntegrationsClient;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl IntegrationsClient for StubIntegrationsClient {
    async fn check_user_repo_auth_status(
        &self,
        _repos: Vec<(String, String)>,
    ) -> Result<UserRepoAuthStatusOutput> {
        Ok(UserRepoAuthStatusOutput::default())
    }

    async fn create_or_update_simple_integration(
        &self,
        _integration_type: String,
        _is_update: bool,
        _environment_uid: Option<String>,
        _base_prompt: Option<String>,
        _model_id: Option<String>,
        _mcp_servers_json: Option<String>,
        _remove_mcp_server_names: Option<Vec<String>>,
        _worker_host: Option<String>,
        _enabled: bool,
    ) -> Result<CreateSimpleIntegrationOutput> {
        Ok(CreateSimpleIntegrationOutput::default())
    }

    async fn list_simple_integrations(
        &self,
        _providers: Vec<String>,
    ) -> Result<SimpleIntegrationsOutput> {
        Ok(SimpleIntegrationsOutput::default())
    }

    async fn poll_oauth_connect_status(&self, _tx_id: String) -> Result<OauthConnectTxStatus> {
        Ok(OauthConnectTxStatus::Unknown)
    }

    async fn get_integrations_using_environment(
        &self,
        _environment_id: String,
    ) -> Result<GetIntegrationsUsingEnvironmentOutput> {
        Ok(GetIntegrationsUsingEnvironmentOutput::default())
    }

    async fn get_user_github_info(&self) -> Result<UserGithubInfoResult> {
        Ok(UserGithubInfoResult::Unknown)
    }

    async fn suggest_cloud_environment_image(
        &self,
        _repos: Vec<(String, String)>,
    ) -> Result<SuggestCloudEnvironmentImageResult> {
        Ok(SuggestCloudEnvironmentImageResult::Unknown)
    }
}
