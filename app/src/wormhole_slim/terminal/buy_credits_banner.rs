//! Buy credits banner stub for `wormhole-slim`.

use warpui::elements::Empty;
use warpui::{AppContext, Element, Entity, View, ViewContext};

pub struct BuyCreditsBanner;

#[derive(Clone, Debug)]
pub enum BuyCreditsBannerEvent {
    OpenBillingAndUsage,
    RefocusInput,
    OpenAutoReloadModal { purchased_credits: i32 },
    ShowAutoReloadError { error_message: &'static str },
}

#[derive(Clone, Debug)]
pub enum Action {}

impl BuyCreditsBanner {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self
    }

    pub fn is_denomination_dropdown_open(&self, _app: &AppContext) -> bool {
        false
    }
}

impl Entity for BuyCreditsBanner {
    type Event = BuyCreditsBannerEvent;
}

impl View for BuyCreditsBanner {
    fn ui_name() -> &'static str {
        "BuyCreditsBanner"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

impl warpui::TypedActionView for BuyCreditsBanner {
    type Action = Action;
}
