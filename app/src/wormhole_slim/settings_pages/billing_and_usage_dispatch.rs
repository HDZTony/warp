//! Billing dispatch stub for `wormhole-slim`.

use warpui::elements::{Container, Empty, ParentElement};
use warpui::Element;
use warpui::{AppContext, Entity, View, ViewContext, ViewHandle};

use super::billing_and_usage_page::{BillingAndUsagePageEvent, BillingAndUsagePageView};
use super::billing_and_usage_page_v2::BillingAndUsagePageV2View;
use super::settings_page::{
    MatchData, PageType, SettingsPageMeta, SettingsPageViewHandle, SettingsWidget, HEADER_PADDING,
};
use super::SettingsSection;

pub struct BillingAndUsageDispatchView {
    page: PageType<Self>,
    v1: ViewHandle<BillingAndUsagePageView>,
    v2: ViewHandle<BillingAndUsagePageV2View>,
}

impl BillingAndUsageDispatchView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let v1 = ctx.add_typed_action_view(BillingAndUsagePageView::new);
        let v2 = ctx.add_typed_action_view(BillingAndUsagePageV2View::new);
        let page = PageType::new_monolith(BillingDispatchWidget, Some("Billing and Usage"), false);
        Self { page, v1, v2 }
    }

    pub fn get_modal_content(&self, _app: &AppContext) -> Option<Box<dyn warpui::Element>> {
        None
    }
}

impl Entity for BillingAndUsageDispatchView {
    type Event = BillingAndUsagePageEvent;
}

impl View for BillingAndUsageDispatchView {
    fn ui_name() -> &'static str {
        "Billing and usage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn warpui::Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for BillingAndUsageDispatchView {
    fn section() -> SettingsSection {
        SettingsSection::BillingAndUsage
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        false
    }

    fn on_page_selected(&mut self, allow_steal_focus: bool, ctx: &mut ViewContext<Self>) {
        self.v1.update(ctx, |view, ctx| {
            view.on_page_selected(allow_steal_focus, ctx);
        });
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

impl From<ViewHandle<BillingAndUsageDispatchView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<BillingAndUsageDispatchView>) -> Self {
        SettingsPageViewHandle::BillingAndUsage(view_handle)
    }
}

#[derive(Default)]
struct BillingDispatchWidget;

impl SettingsWidget for BillingDispatchWidget {
    type View = BillingAndUsageDispatchView;

    fn search_terms(&self) -> &str {
        "plan billing usage credits"
    }

    fn render(
        &self,
        _view: &BillingAndUsageDispatchView,
        _appearance: &crate::appearance::Appearance,
        _app: &AppContext,
    ) -> Box<dyn warpui::Element> {
        Container::new(Empty::new().finish())
            .with_margin_top(HEADER_PADDING)
            .finish()
    }
}
