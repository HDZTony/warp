//! [`ServerApi`] / [`ServerApiProvider`] stubs for `wormhole-slim`.

use std::sync::Arc;

use futures::FutureExt as _;

use super::ai::{AIClient, StubAIClient};
use super::auth::{AuthClient, AuthEvent, StubAuthClient};
use super::block::{BlockClient, StubBlockClient};
use super::harness_support::{HarnessSupportClient, StubHarnessSupportClient};
use super::integrations::{IntegrationsClient, StubIntegrationsClient};
use super::object::ObjectClient;
use super::referral::{ReferralsClient, StubReferralsClient};
use super::team::TeamClient;
use super::workspace::WorkspaceClient;
use crate::auth::auth_state::AuthState;
use crate::server::experiments::{ServerExperiment, ServerExperiments};
use crate::server::iap::IapState;
use crate::server::telemetry::TelemetryApi;
use crate::settings::PrivacySettingsSnapshot;
use crate::settings_view;
use crate::wormhole_slim::object_client::StubObjectClient;
use warp_core::telemetry::TelemetryEvent;
use warp_managed_secrets::client::ManagedSecretsClient;
use warpui::{Entity, ModelContext, SingletonEntity};

pub struct ServerApi {
    client: Arc<http_client::Client>,
    telemetry_api: TelemetryApi,
}

impl ServerApi {
    fn new_stub() -> Self {
        Self {
            client: Arc::new(http_client::Client::new()),
            telemetry_api: TelemetryApi::new(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new_stub()
    }

    pub fn http_client(&self) -> &http_client::Client {
        &self.client
    }

    pub async fn send_telemetry_event(
        &self,
        _event: impl TelemetryEvent,
        _settings_snapshot: PrivacySettingsSnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn flush_telemetry_events(
        &self,
        _settings_snapshot: PrivacySettingsSnapshot,
    ) -> anyhow::Result<usize> {
        Ok(0)
    }

    pub async fn get_or_refresh_access_token(
        &self,
    ) -> anyhow::Result<crate::auth::credentials::AuthToken> {
        anyhow::bail!("wormhole-slim: cloud auth token refresh unavailable")
    }

    pub async fn generate_multi_agent_output(
        &self,
        _request: &warp_multi_agent_api::Request,
    ) -> std::result::Result<
        crate::server::server_api::AIOutputStream<warp_multi_agent_api::ResponseEvent>,
        Arc<crate::server::server_api::AIApiError>,
    > {
        Err(Arc::new(crate::server::server_api::AIApiError::Other(
            anyhow::anyhow!("wormhole-slim: cloud AI API unavailable"),
        )))
    }

    pub async fn stream_agent_events_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
        _run_ids: &[String],
        _since_sequence: i64,
    ) -> anyhow::Result<http_client::EventSourceStream> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub(crate) async fn send_agent_message_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
        _request: super::ai::SendAgentMessageRequest,
    ) -> anyhow::Result<super::ai::SendAgentMessageResponse> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub(crate) async fn list_agent_messages_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
        _run_id: &str,
        _request: super::ai::ListAgentMessagesRequest,
    ) -> anyhow::Result<Vec<super::ai::AgentMessageHeader>> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub(crate) async fn mark_message_delivered_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
        _message_id: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub(crate) async fn read_agent_message_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
        _message_id: &str,
    ) -> anyhow::Result<super::ai::ReadAgentMessageResponse> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub fn set_ambient_agent_task_id(
        &self,
        _task_id: Option<crate::ai::ambient_agents::AmbientAgentTaskId>,
    ) {
    }

    pub async fn stream_agent_events(
        &self,
        _run_ids: &[String],
        _since_sequence: i64,
    ) -> anyhow::Result<http_client::EventSourceStream> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub async fn resolve_prompt_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
        request: super::harness_support::ResolvePromptRequest,
    ) -> anyhow::Result<super::harness_support::ResolvedHarnessPrompt> {
        StubHarnessSupportClient.resolve_prompt(request).await
    }

    pub async fn fetch_transcript_for_task(
        &self,
        _task_id: &crate::ai::ambient_agents::AmbientAgentTaskId,
    ) -> anyhow::Result<bytes::Bytes> {
        StubHarnessSupportClient.fetch_transcript().await
    }

    pub fn send_graphql_request<'a, QF, O: warp_graphql::client::Operation<QF> + Send + 'a>(
        &'a self,
        _operation: O,
        _timeout: Option<std::time::Duration>,
    ) -> warpui::r#async::BoxFuture<'a, anyhow::Result<QF>>
    where
        QF: 'a,
    {
        async move { anyhow::bail!("wormhole-slim: graphql unavailable") }.boxed()
    }

    pub async fn fetch_channel_versions(
        &self,
        _include_changelogs: bool,
        _is_daily: bool,
    ) -> anyhow::Result<channel_versions::ChannelVersions> {
        anyhow::bail!("wormhole-slim: channel versions unavailable")
    }

    pub async fn stream_agent_events_for_ancestor(
        &self,
        _ancestor_run_id: &str,
        _include_self: bool,
        _since_sequence: i64,
    ) -> anyhow::Result<http_client::EventSourceStream> {
        anyhow::bail!("wormhole-slim: cloud AI API unavailable")
    }

    pub async fn generate_ai_input_suggestions(
        &self,
        _request: &crate::ai::predict::generate_ai_input_suggestions::GenerateAIInputSuggestionsRequest,
    ) -> std::result::Result<
        crate::ai::predict::generate_ai_input_suggestions::GenerateAIInputSuggestionsResponseV2,
        crate::server::server_api::AIApiError,
    > {
        Err(crate::server::server_api::AIApiError::Other(anyhow::anyhow!(
            "wormhole-slim: cloud AI API unavailable"
        )))
    }

    pub async fn generate_am_query_suggestions(
        &self,
        _request: &crate::ai::predict::generate_am_query_suggestions::GenerateAMQuerySuggestionsRequest,
    ) -> std::result::Result<
        crate::ai::predict::generate_am_query_suggestions::GenerateAMQuerySuggestionsResponse,
        crate::server::server_api::AIApiError,
    > {
        Err(crate::server::server_api::AIApiError::Other(anyhow::anyhow!(
            "wormhole-slim: cloud AI API unavailable"
        )))
    }

    pub async fn predict_am_queries(
        &self,
        _request: &crate::ai::predict::predict_am_queries::PredictAMQueriesRequest,
    ) -> std::result::Result<
        crate::ai::predict::predict_am_queries::PredictAMQueriesResponse,
        crate::server::server_api::AIApiError,
    > {
        Err(crate::server::server_api::AIApiError::Other(anyhow::anyhow!(
            "wormhole-slim: cloud AI API unavailable"
        )))
    }

    pub async fn get_relevant_files(
        &self,
        _request: &crate::ai::get_relevant_files::api::GetRelevantFiles,
    ) -> std::result::Result<
        crate::ai::get_relevant_files::api::GetRelevantFilesResponse,
        crate::server::server_api::AIApiError,
    > {
        Err(crate::server::server_api::AIApiError::Other(anyhow::anyhow!(
            "wormhole-slim: cloud AI API unavailable"
        )))
    }

    pub async fn transcribe(
        &self,
        _request: &crate::ai::voice::transcribe::TranscribeRequest,
    ) -> std::result::Result<
        crate::ai::voice::transcribe::TranscribeResponse,
        crate::server::server_api::AIApiError,
    > {
        Err(crate::server::server_api::AIApiError::Other(anyhow::anyhow!(
            "wormhole-slim: cloud AI API unavailable"
        )))
    }

    pub async fn server_time(&self) -> anyhow::Result<super::ServerTime> {
        anyhow::bail!("wormhole-slim: server time unavailable")
    }

    pub fn persist_telemetry_events(
        &self,
        _max_event_count: usize,
        _settings_snapshot: crate::settings::PrivacySettingsSnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn flush_persisted_events_to_rudder(
        &self,
        _path: &std::path::Path,
        _settings_snapshot: crate::settings::PrivacySettingsSnapshot,
    ) -> anyhow::Result<usize> {
        Ok(0)
    }
}

pub struct ServerApiProvider {
    server_api: Arc<ServerApi>,
    auth_client: Arc<dyn AuthClient>,
    ai_client: Arc<dyn AIClient>,
    object_client: Arc<dyn ObjectClient>,
    team_client: Arc<dyn TeamClient>,
    workspace_client: Arc<dyn WorkspaceClient>,
    referrals_client: Arc<dyn ReferralsClient>,
    block_client: Arc<dyn BlockClient>,
    integrations_client: Arc<dyn IntegrationsClient>,
    harness_client: Arc<dyn HarnessSupportClient>,
}

impl ServerApiProvider {
    pub fn new(
        _auth_state: Arc<AuthState>,
        _agent_source: Option<super::ai::AgentSource>,
        _iap_state: Option<Arc<IapState>>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self::stub()
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::stub()
    }

    fn stub() -> Self {
        let server_api = Arc::new(ServerApi::new_stub());
        Self {
            server_api: server_api.clone(),
            auth_client: Arc::new(StubAuthClient),
            ai_client: Arc::new(StubAIClient),
            object_client: Arc::new(StubObjectClient),
            team_client: Arc::new(super::team::StubTeamClient),
            workspace_client: Arc::new(super::workspace::StubWorkspaceClient),
            referrals_client: Arc::new(StubReferralsClient),
            block_client: Arc::new(StubBlockClient),
            integrations_client: Arc::new(StubIntegrationsClient),
            harness_client: Arc::new(StubHarnessSupportClient),
        }
    }

    pub fn get(&self) -> Arc<ServerApi> {
        self.server_api.clone()
    }

    pub fn get_auth_client(&self) -> Arc<dyn AuthClient> {
        self.auth_client.clone()
    }

    pub fn get_referrals_client(&self) -> Arc<dyn ReferralsClient> {
        self.referrals_client.clone()
    }

    pub fn get_block_client(&self) -> Arc<dyn BlockClient> {
        self.block_client.clone()
    }

    pub fn get_workspace_client(&self) -> Arc<dyn WorkspaceClient> {
        self.workspace_client.clone()
    }

    pub fn get_team_client(&self) -> Arc<dyn TeamClient> {
        self.team_client.clone()
    }

    pub fn get_ai_client(&self) -> Arc<dyn AIClient> {
        self.ai_client.clone()
    }

    pub fn get_cloud_objects_client(&self) -> Arc<dyn ObjectClient> {
        self.object_client.clone()
    }

    pub fn get_integrations_client(&self) -> Arc<dyn IntegrationsClient> {
        self.integrations_client.clone()
    }

    pub fn get_managed_secrets_client(&self) -> Arc<dyn ManagedSecretsClient> {
        Arc::new(warp_managed_secrets::noop_client::NoopManagedSecretsClient)
    }

    pub fn get_http_client(&self) -> Arc<http_client::Client> {
        Arc::new(http_client::Client::new())
    }

    pub fn get_harness_support_client(&self) -> Arc<dyn HarnessSupportClient> {
        self.harness_client.clone()
    }

    pub fn handle_experiments_fetched(
        &self,
        experiments: Vec<ServerExperiment>,
        ctx: &mut ModelContext<Self>,
    ) {
        ServerExperiments::handle(ctx).update(ctx, |state, ctx| {
            state.apply_latest_state(experiments, ctx);
        });
        settings_view::handle_experiment_change(ctx);
    }
}

impl Entity for ServerApiProvider {
    type Event = AuthEvent;
}

impl SingletonEntity for ServerApiProvider {}
