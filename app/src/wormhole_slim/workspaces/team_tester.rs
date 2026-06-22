//! Minimal team tester stub for `wormhole-slim` embed builds.

use warpui::{Entity, ModelContext, SingletonEntity};

#[derive(Clone)]
pub struct TeamTesterStatus {}

impl TeamTesterStatus {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {}
    }

    #[cfg(test)]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::new(ctx)
    }

    pub fn initiate_data_pollers(&mut self, _force_refresh: bool, _ctx: &mut ModelContext<Self>) {}
}

pub enum TeamTesterStatusEvent {
    InitiateDataPollers { force_refresh: bool },
}

impl Entity for TeamTesterStatus {
    type Event = TeamTesterStatusEvent;
}

impl SingletonEntity for TeamTesterStatus {}
