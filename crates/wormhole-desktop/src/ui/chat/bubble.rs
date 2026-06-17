use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::theme;
use crate::ui_text;

pub struct ChatBubbleView {
    font: FamilyId,
    author: String,
    body: String,
}

impl ChatBubbleView {
    pub fn new(ctx: &mut ViewContext<Self>, author: String, body: String) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self { font, author, body }
    }
}

impl Entity for ChatBubbleView {
    type Event = ();
}

impl View for ChatBubbleView {
    fn ui_name() -> &'static str {
        "ChatBubbleView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let col = Flex::column()
            .with_child(ui_text::body(format!("@{}", self.author), self.font).finish())
            .with_child(ui_text::mono(self.body.clone(), self.font).finish());
        Container::new(col.finish())
            .with_background(theme::accent_bg(24))
            .with_uniform_padding(8.0)
            .finish()
    }
}
