//! Billing & usage v2 stub for `wormhole-slim`.

use warpui::elements::{Container, Empty, ParentElement};
use warpui::Element;
use warpui::{AppContext, Entity, TypedActionView, View, ViewContext};

use super::billing_and_usage_page::BillingAndUsagePageEvent;
use super::settings_page::{MatchData, PageType, SettingsPageMeta, SettingsWidget};
use super::SettingsSection;
use crate::appearance::Appearance;

pub struct BillingAndUsagePageV2View {
    page: PageType<Self>,
}

#[derive(Debug, Clone)]
pub enum BillingAndUsagePageV2Action {}

impl TypedActionView for BillingAndUsagePageV2View {
    type Action = BillingAndUsagePageV2Action;

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}

impl BillingAndUsagePageV2View {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        Self {
            page: PageType::new_monolith(BillingV2Widget, Some("Billing and Usage"), false),
        }
    }

    pub fn get_modal_content(&self) -> Option<Box<dyn warpui::Element>> {
        None
    }

    pub fn on_page_selected(&mut self, _allow_steal_focus: bool, _ctx: &mut ViewContext<Self>) {}
}

impl Entity for BillingAndUsagePageV2View {
    type Event = BillingAndUsagePageEvent;
}

impl View for BillingAndUsagePageV2View {
    fn ui_name() -> &'static str {
        "Billing and usage v2"
    }

    fn render(&self, app: &AppContext) -> Box<dyn warpui::Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for BillingAndUsagePageV2View {
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
struct BillingV2Widget;

impl SettingsWidget for BillingV2Widget {
    type View = BillingAndUsagePageV2View;

    fn search_terms(&self) -> &str {
        "plan billing usage credits"
    }

    fn render(
        &self,
        _view: &BillingAndUsagePageV2View,
        _appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn warpui::Element> {
        Container::new(Empty::new().finish()).finish()
    }
}
