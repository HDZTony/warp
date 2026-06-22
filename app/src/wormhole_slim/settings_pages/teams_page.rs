//! Teams settings stub for `wormhole-slim` (cloud teams UI removed).

use serde::{Deserialize, Serialize};
use warpui::elements::{Container, Empty, ParentElement};
use warpui::Element;
use warpui::{AppContext, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::settings_page::{
    MatchData, PageType, SettingsPageMeta, SettingsPageViewHandle, SettingsWidget,
};
use super::SettingsSection;
use crate::view_components::ToastFlavor;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum TeamsInviteOption {
    #[default]
    Link,
    Email,
}

impl std::fmt::Display for TeamsInviteOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamsInviteOption::Link => write!(f, "Link"),
            TeamsInviteOption::Email => write!(f, "Email"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct OpenTeamsSettingsModalArgs {
    pub invite_email: Option<String>,
}

#[derive(Clone)]
pub enum TeamsPageViewEvent {
    TeamsChanged,
    OpenWarpDrive,
    ShowToast {
        message: String,
        flavor: ToastFlavor,
    },
}

#[derive(Debug, Clone)]
pub enum TeamsPageAction {}

pub struct TeamsPageView {
    page: PageType<Self>,
}

impl TeamsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        Self {
            page: PageType::new_monolith(TeamsPageWidget, Some("Teams"), false),
        }
    }

    pub fn open_team_members(&mut self, _email: Option<&String>, _ctx: &mut ViewContext<Self>) {}
}

impl Entity for TeamsPageView {
    type Event = TeamsPageViewEvent;
}

impl TypedActionView for TeamsPageView {
    type Action = TeamsPageAction;

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}

impl View for TeamsPageView {
    fn ui_name() -> &'static str {
        "TeamsPage"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn warpui::Element> {
        self.page.render(self, _app)
    }
}

impl SettingsPageMeta for TeamsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Teams
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        false
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id);
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<TeamsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<TeamsPageView>) -> Self {
        SettingsPageViewHandle::Teams(view_handle)
    }
}

#[derive(Default)]
struct TeamsPageWidget;

impl SettingsWidget for TeamsPageWidget {
    type View = TeamsPageView;

    fn search_terms(&self) -> &str {
        "teams invite members billing"
    }

    fn render(
        &self,
        _view: &TeamsPageView,
        _appearance: &crate::appearance::Appearance,
        _app: &AppContext,
    ) -> Box<dyn warpui::Element> {
        Container::new(Empty::new().finish()).finish()
    }
}
