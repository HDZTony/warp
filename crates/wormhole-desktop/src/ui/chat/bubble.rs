use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, Empty, Expanded, Flex,
    MainAxisSize, ParentElement, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::chat::layout::bubble_corner_radius;
use crate::ui::panel_primitives::{
    chat_bubble_in_bg, chat_bubble_out_bg, chat_bubble_out_border, TG_BUBBLE_MAX_WIDTH,
};
use crate::ui::theme;
use crate::ui_text;

pub struct ChatBubbleView {
    font: FamilyId,
    body: String,
    outgoing: bool,
    timestamp: String,
    system: bool,
    grouped: bool,
    read: bool,
    search_hit: bool,
}

impl ChatBubbleView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        body: String,
        outgoing: bool,
        timestamp: String,
        grouped: bool,
        read: bool,
        search_hit: bool,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            font,
            body,
            outgoing,
            timestamp,
            system: false,
            grouped,
            read,
            search_hit,
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
            grouped: false,
            read: false,
            search_hit: false,
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
                .with_corner_radius(warpui::elements::CornerRadius::with_all(
                    warpui::elements::Radius::Pixels(999.0),
                ))
                .finish(),
            )
            .finish();
        }

        let (bg, border) = if self.outgoing {
            (chat_bubble_out_bg(), chat_bubble_out_border())
        } else {
            (chat_bubble_in_bg(), theme::border())
        };
        let radius = bubble_corner_radius(self.outgoing, self.grouped);

        let mut meta_row = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        if !self.timestamp.is_empty() {
            meta_row.add_child(
                ui_text::chat_bubble_meta(self.timestamp.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }
        if self.outgoing && self.read {
            meta_row.add_child(
                Container::new(
                    ui_text::chat_bubble_meta("✓✓", self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_margin_left(4.0)
                .finish(),
            );
        }

        let mut bubble_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        bubble_col.add_child(
            ui_text::chat_bubble_text(self.body.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if !self.timestamp.is_empty() || (self.outgoing && self.read) {
            bubble_col.add_child(
                Container::new(Align::new(meta_row.finish()).right().finish())
                    .with_margin_top(2.0)
                    .finish(),
            );
        }

        let mut bubble = Container::new(
            ConstrainedBox::new(bubble_col.finish())
                .with_max_width(TG_BUBBLE_MAX_WIDTH)
                .finish(),
        )
        .with_padding_left(11.0)
        .with_padding_right(11.0)
        .with_padding_top(7.0)
        .with_padding_bottom(5.0)
        .with_background(bg)
        .with_corner_radius(radius);
        let border_width = if self.search_hit { 2.0 } else { 1.0 };
        let border_color = if self.search_hit {
            theme::accent_cool()
        } else {
            border
        };
        bubble = bubble.with_border(Border::all(border_width).with_border_fill(border_color));
        let bubble = bubble.finish();

        let bubble_slot = Shrinkable::new(0.72, bubble).finish();
        let spacer = Expanded::new(1.0, Empty::new().finish()).finish();

        let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        if self.outgoing {
            row.add_child(spacer);
            row.add_child(bubble_slot);
        } else {
            row.add_child(bubble_slot);
            row.add_child(spacer);
        }
        row.finish()
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

pub fn outgoing_message_read(sent_at: u64, is_last_outgoing: bool) -> bool {
    if !is_last_outgoing {
        return true;
    }
    if sent_at == 0 {
        return false;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(sent_at) >= 1
}
