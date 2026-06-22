//! Minimal [`TeamUpdateManager`] stub for `wormhole-slim` embed builds.

use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use futures::channel::oneshot::{self, Receiver};
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::persistence::ModelEvent;
use crate::server::ids::ServerId;
use crate::server::server_api::team;
use crate::workspaces::workspace::WorkspaceUid;

pub enum TeamUpdateManagerEvent {
    LeaveSuccess,
    LeaveError,
    RenameTeamSuccess,
    RenameTeamError,
}

pub struct TeamUpdateManager {
    #[allow(dead_code)]
    model_event_sender: Option<SyncSender<ModelEvent>>,
}

impl TeamUpdateManager {
    pub fn new(
        _team_client: Arc<dyn crate::server::server_api::team::TeamClient>,
        model_event_sender: Option<SyncSender<ModelEvent>>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self {
            model_event_sender,
        }
    }

    #[cfg(test)]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::new(Arc::new(team::StubTeamClient), None, ctx)
    }

    pub fn start_polling_for_workspace_metadata_updates(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn stop_polling_for_workspace_metadata_updates(&mut self) {}

    pub fn refresh_workspace_metadata(&mut self, _ctx: &mut ModelContext<Self>) -> Receiver<()> {
        let (_tx, rx) = oneshot::channel();
        rx
    }

    pub fn create_team(&mut self, _name: String, _ctx: &mut ModelContext<Self>) {}

    pub fn leave_team(&mut self, _team_uid: ServerId, _ctx: &mut ModelContext<Self>) {}

    pub fn rename_team(&mut self, _new_name: String, _ctx: &mut ModelContext<Self>) {}

    pub fn set_current_workspace_uid(
        &mut self,
        _workspace_uid: WorkspaceUid,
        _ctx: &mut ModelContext<Self>,
    ) {
    }
}

impl Entity for TeamUpdateManager {
    type Event = TeamUpdateManagerEvent;
}

impl SingletonEntity for TeamUpdateManager {}
