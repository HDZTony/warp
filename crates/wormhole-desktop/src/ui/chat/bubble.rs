use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Flex,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::theme;
use crate::ui_text;

const TG_BUBBLE_MAX_WIDTH: f32 = 520.0;
const TG_BUBBLE_RADIUS: f32 = 12.0;

pub struct ChatBubbleView {
    font: FamilyId,
    body: String,
    outgoing: bool,
    timestamp: String,
    system: bool,
}

impl ChatBubbleView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        body: String,
        outgoing: bool,
        timestamp: String,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            font,
            body,
            outgoing,
            timestamp,
            system: false,
        }
    }

    pub fn system_hint(ctx: &mut ViewContext<Self>, body: String) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            font,
            body,
            outgoing: false,
            timestamp: String::new(),
            system: true,
        }
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
        if self.system {
            return Align::new(
                Container::new(
                    ui_text::body(self.body.clone(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_horizontal_padding(12.0)
                .with_vertical_padding(4.0)
                .with_background(theme::panel())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
                .finish(),
            )
            .finish();
        }

        let (bg, border) = if self.outgoing {
            (theme::accent_cool_bg(40), theme::accent_cool())
        } else {
            (theme::panel_elevated(), theme::border())
        };
        let radius = CornerRadius::with_all(Radius::Pixels(TG_BUBBLE_RADIUS));

        let mut bubble_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        bubble_col.add_child(
            ui_text::body(self.body.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if !self.timestamp.is_empty() {
            bubble_col.add_child(
                Container::new(
                    Align::new(
                        ui_text::device_meta(self.timestamp.clone(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .right()
                    .finish(),
                )
                .with_margin_top(4.0)
                .finish(),
            );
        }

        let bubble = Container::new(
            ConstrainedBox::new(bubble_col.finish())
                .with_max_width(TG_BUBBLE_MAX_WIDTH)
                .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(radius)
        .finish();

        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(if self.outgoing {
                MainAxisAlignment::End
            } else {
                MainAxisAlignment::Start
            })
            .with_child(Shrinkable::new(1.0, bubble).finish())
            .finish()
    }
}

fn format_message_time(timestamp: u64) -> String {
    if timestamp == 0 {
        return String::new();
    }
    let secs = timestamp as i64;
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    format!("{hours:02}:{minutes:02}")
}

pub fn format_message_time_pub(timestamp: u64) -> String {
    format_message_time(timestamp)
}
