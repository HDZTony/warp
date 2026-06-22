//! Billing & usage settings stub for `wormhole-slim`.

use warpui::elements::{Container, Empty, ParentElement};
use warpui::Element;
use warpui::{AppContext, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::settings_page::{
    MatchData, PageType, SettingsPageMeta, SettingsPageViewHandle, SettingsWidget,
};
use super::SettingsSection;
use crate::appearance::Appearance;
use crate::view_components::ToastFlavor;

pub fn create_discount_badge(_discount: u32, _appearance: &Appearance) -> Box<dyn Element> {
    Empty::new().finish()
}

#[derive(Debug, Clone)]
pub enum BillingAndUsagePageEvent {
    SignupAnonymousUser,
    ShowToast {
        message: String,
        flavor: ToastFlavor,
    },
    ShowModal,
    HideModal,
}

pub struct BillingAndUsagePageView {
    page: PageType<Self>,
}

#[derive(Debug, Clone)]
pub enum BillingAndUsagePageAction {}

impl TypedActionView for BillingAndUsagePageView {
    type Action = BillingAndUsagePageAction;

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}

impl BillingAndUsagePageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        Self {
            page: PageType::new_monolith(BillingPageWidget, Some("Billing and Usage"), false),
        }
    }

    pub fn get_modal_content(&self) -> Option<Box<dyn warpui::Element>> {
        None
    }

    pub fn on_page_selected(&mut self, _allow_steal_focus: bool, _ctx: &mut ViewContext<Self>) {}
}

impl Entity for BillingAndUsagePageView {
    type Event = BillingAndUsagePageEvent;
}

impl View for BillingAndUsagePageView {
    fn ui_name() -> &'static str {
        "Billing and usage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn warpui::Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for BillingAndUsagePageView {
    fn section() -> SettingsSection {
        SettingsSection::BillingAndUsage
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

#[derive(Default)]
struct BillingPageWidget;

impl SettingsWidget for BillingPageWidget {
    type View = BillingAndUsagePageView;

    fn search_terms(&self) -> &str {
        "plan billing usage credits"
    }

    fn render(
        &self,
        _view: &BillingAndUsagePageView,
        _appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn warpui::Element> {
        Container::new(Empty::new().finish()).finish()
    }
}
