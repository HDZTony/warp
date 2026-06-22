//! Minimal [`ServerApiProvider`] stub for `wormhole-slim` embed builds.

pub mod ai;
pub mod auth;
pub mod object;
pub mod team;
pub mod workspace;

use std::sync::Arc;

use warpui::{Entity, ModelContext, SingletonEntity};

use crate::wormhole_slim::object_client::{CloudObjectClient, StubObjectClient};

pub enum ServerApiProviderEvent {}

pub struct ServerApi {}

impl ServerApi {
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self
    }
}

pub struct ServerApiProvider {
    auth_client: Arc<dyn auth::AuthClient>,
    ai_client: Arc<dyn ai::AIClient>,
    object_client: Arc<dyn CloudObjectClient>,
    team_client: Arc<dyn team::TeamClient>,
    workspace_client: Arc<dyn workspace::WorkspaceClient>,
}

impl ServerApiProvider {
    pub fn new(
        _auth_state: Arc<crate::auth::AuthState>,
        _agent_source: Option<ai::AgentSource>,
        _iap_state: Option<Arc<crate::server::iap::IapState>>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self::stub()
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::stub()
    }

    fn stub() -> Self {
        Self {
            auth_client: Arc::new(auth::StubAuthClient),
            ai_client: Arc::new(ai::StubAIClient),
            object_client: Arc::new(StubObjectClient),
            team_client: Arc::new(team::StubTeamClient),
            workspace_client: Arc::new(workspace::StubWorkspaceClient),
        }
    }

    pub fn get(&self) -> Arc<ServerApi> {
        Arc::new(ServerApi {})
    }

    pub fn get_auth_client(&self) -> Arc<dyn auth::AuthClient> {
        self.auth_client.clone()
    }

    pub fn get_ai_client(&self) -> Arc<dyn ai::AIClient> {
        self.ai_client.clone()
    }

    pub fn get_cloud_objects_client(&self) -> Arc<dyn CloudObjectClient> {
        self.object_client.clone()
    }

    pub fn get_team_client(&self) -> Arc<dyn team::TeamClient> {
        self.team_client.clone()
    }

    pub fn get_workspace_client(&self) -> Arc<dyn workspace::WorkspaceClient> {
        self.workspace_client.clone()
    }

    pub fn handle_experiments_fetched(
        &self,
        _experiments: Vec<crate::server::experiments::ServerExperiment>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }
}

impl Entity for ServerApiProvider {
    type Event = ServerApiProviderEvent;
}

impl SingletonEntity for ServerApiProvider {}
