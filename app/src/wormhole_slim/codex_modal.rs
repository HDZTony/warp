//! Stub Codex cloud modal for `wormhole-slim` embed builds (local `codex` CLI only).

use warpui::elements::Empty;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

pub fn init(_app: &mut AppContext) {}

pub struct CodexModal;

impl CodexModal {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self
    }
}

impl Entity for CodexModal {
    type Event = CodexModalEvent;
}

impl View for CodexModal {
    fn ui_name() -> &'static str {
        "CodexModalStub"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

impl TypedActionView for CodexModal {
    type Action = ();
}

#[derive(Copy, Clone, Debug)]
pub enum CodexModalEvent {
    Close,
    UseCodex,
}
