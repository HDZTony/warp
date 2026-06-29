use warpui::elements::{Border, ConstrainedBox, Container, Flex, ParentElement, Shrinkable};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::panel_primitives::HUD_RADIUS;
use crate::ui::theme;
use crate::ui_text;

pub struct ChatBubbleView {
    font: FamilyId,
    mono: FamilyId,
    author: String,
    body: String,
}

impl ChatBubbleView {
    pub fn new(ctx: &mut ViewContext<Self>, author: String, body: String) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        Self {
            font,
            mono,
            author,
            body,
        }
    }

    pub fn system_hint(ctx: &mut ViewContext<Self>, body: String) -> Self {
        Self::new(ctx, "system".into(), body)
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
        let author_label = self.author.to_ascii_uppercase();
        let body_col = Flex::column()
            .with_child(
                ui_text::hud_title(author_label, self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_child(
                Container::new(
                    ui_text::mono(self.body.clone(), self.mono)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_margin_top(6.0)
                .finish(),
            );
        Container::new(
            Flex::row()
                .with_child(
                    Container::new(
                        ConstrainedBox::new(Flex::row().finish())
                            .with_width(2.0)
                            .finish(),
                    )
                    .with_background(theme::accent_cool())
                    .finish(),
                )
                .with_child(
                    Shrinkable::new(
                        1.0,
                        Container::new(body_col.finish())
                            .with_padding_left(12.0)
                            .with_padding_right(12.0)
                            .with_padding_top(10.0)
                            .with_padding_bottom(10.0)
                            .with_background(theme::panel_elevated())
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(HUD_RADIUS),
        ))
        .finish()
    }
}
