//! Minimal [`RemoteServerManager`] stub for `wormhole-slim` embed builds.

use warpui::{Entity, ModelContext, SingletonEntity};

pub enum RemoteServerManagerEvent {}

pub struct RemoteServerManager {}

impl RemoteServerManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {}
    }

    pub fn update_codebase_index_limits(
        &mut self,
        _limits: Option<remote_server::proto::CodebaseIndexLimits>,
    ) {
    }
}

impl Entity for RemoteServerManager {
    type Event = RemoteServerManagerEvent;
}

impl SingletonEntity for RemoteServerManager {}
