//! Build plan migration modal stub for `wormhole-slim`.

use warpui::elements::{Empty, ParentElement};
use warpui::Element;
use warpui::{AppContext, Entity, TypedActionView, View, ViewContext};

use crate::view_components::ToastFlavor;

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum BuildPlanMigrationModalViewAction {
    SelectReloadDenomination(usize),
    EnableAutoReloadToggled(bool),
    GetStartedClicked,
    Close,
    OpenUrl(&'static str),
}

pub struct BuildPlanMigrationModal;

impl BuildPlanMigrationModal {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self
    }
}

#[derive(Clone, Debug)]
pub enum BuildPlanMigrationModalEvent {
    Close,
    ShowToast {
        message: String,
        flavor: ToastFlavor,
    },
}

impl Entity for BuildPlanMigrationModal {
    type Event = BuildPlanMigrationModalEvent;
}

impl TypedActionView for BuildPlanMigrationModal {
    type Action = BuildPlanMigrationModalViewAction;

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}

impl View for BuildPlanMigrationModal {
    fn ui_name() -> &'static str {
        "BuildPlanMigrationModal"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn warpui::Element> {
        Empty::new().finish()
    }
}
