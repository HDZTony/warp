use chrono::TimeZone;
use std::collections::HashMap;
use std::path::Path;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded,
    Flex, Image, MainAxisSize, ParentElement, Radius, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::chat::image_asset::insert_attachment_image_asset;
use crate::ui::chat::layout::bubble_corner_radius;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    chat_bubble_in_bg, chat_bubble_out_bg, chat_bubble_out_border, TG_BUBBLE_MAX_WIDTH,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::ChatAttachmentDto;

const ATTACH_CHIP_ICON: f32 = 16.0;
const ATTACH_MEDIA_PLACEHOLDER_HEIGHT: f32 = 120.0;
const ATTACH_MEDIA_PREVIEW_HEIGHT: f32 = 180.0;

pub struct ChatBubbleView {
    font: FamilyId,
    emoji_font: FamilyId,
    body: String,
    attachments: Vec<ChatAttachmentDto>,
    /// attachment_id → AssetCache id for decoded image previews
    image_assets: HashMap<String, String>,
    outgoing: bool,
    timestamp: String,
    system: bool,
    grouped: bool,
    read: bool,
    search_hit: bool,
    max_bubble_width: f32,
}

impl ChatBubbleView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        body: String,
        attachments: Vec<ChatAttachmentDto>,
        outgoing: bool,
        timestamp: String,
        grouped: bool,
        read: bool,
        search_hit: bool,
        max_bubble_width: f32,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        let mut image_assets = HashMap::new();
        for attachment in &attachments {
            if attachment.kind != "image" {
                continue;
            }
            let Some(local_path) = attachment.local_path.as_deref() else {
                continue;
            };
            if let Ok(asset_id) =
                insert_attachment_image_asset(ctx, &attachment.id, Path::new(local_path))
            {
                image_assets.insert(attachment.id.clone(), asset_id);
            }
        }
        Self {
            font,
            emoji_font,
            body,
            attachments,
            image_assets,
            outgoing,
            timestamp,
            system: false,
            grouped,
            read,
            search_hit,
            max_bubble_width,
        }
    }

    pub fn system_hint(ctx: &mut ViewContext<Self>, body: String) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        Self {
            font,
            emoji_font,
            body,
            attachments: Vec::new(),
            image_assets: HashMap::new(),
            outgoing: false,
            timestamp: String::new(),
            system: true,
            grouped: false,
            read: false,
            search_hit: false,
            max_bubble_width: TG_BUBBLE_MAX_WIDTH,
        }
    }

    fn body_font(&self) -> FamilyId {
        crate::ui::fonts::chat_message_font(self.font, self.emoji_font, &self.body)
    }

    pub fn set_body(&mut self, body: String) {
        self.body = body;
    }

    fn render_attachments(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        for attachment in &self.attachments {
            col.add_child(
                Container::new(self.render_attachment_chip(attachment))
                    .with_margin_bottom(4.0)
                    .finish(),
            );
        }
        col.finish()
    }

    fn render_attachment_chip(&self, attachment: &ChatAttachmentDto) -> Box<dyn Element> {
        if attachment.kind == "image" || attachment.kind == "video" {
            return self.render_media_attachment(attachment);
        }
        self.render_file_attachment(attachment)
    }

    fn render_media_attachment(&self, attachment: &ChatAttachmentDto) -> Box<dyn Element> {
        if attachment.kind == "image" {
            if let Some(asset_id) = self.image_assets.get(&attachment.id) {
                return ConstrainedBox::new(
                    Container::new(
                        Image::new(
                            AssetSource::Raw {
                                id: asset_id.clone(),
                            },
                            CacheOption::BySize,
                        )
                        .finish(),
                    )
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                    .finish(),
                )
                .with_min_width(160.0)
                .with_max_width(self.max_bubble_width)
                .with_height(ATTACH_MEDIA_PREVIEW_HEIGHT)
                .finish();
            }
        }
        let label = attachment_kind_label(&attachment.kind);
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        col.add_child(
            Container::new(
                ConstrainedBox::new(
                    Flex::row()
                        .with_main_axis_alignment(warpui::elements::MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(
                            ui_text::chat_bubble_meta(label.to_string(), self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .finish(),
                )
                .with_height(ATTACH_MEDIA_PLACEHOLDER_HEIGHT - 28.0)
                .finish(),
            )
            .with_uniform_padding(8.0)
            .with_background(theme::accent_bg(24))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::chat_bubble_meta(attachment.name.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_top(4.0)
            .finish(),
        );
        ConstrainedBox::new(col.finish())
            .with_min_width(160.0)
            .with_max_width(self.max_bubble_width)
            .finish()
    }

    fn render_file_attachment(&self, attachment: &ChatAttachmentDto) -> Box<dyn Element> {
        let icon_path = if attachment.kind == "document" {
            "chat-attach-document.svg"
        } else {
            "chat-compose-attach.svg"
        };
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(
                Container::new(
                    ConstrainedBox::new(
                        Flex::row()
                            .with_main_axis_alignment(warpui::elements::MainAxisAlignment::Center)
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_child(icons::icon(
                                icon_path,
                                ATTACH_CHIP_ICON,
                                theme::accent_cool(),
                            ))
                            .finish(),
                    )
                    .with_width(28.0)
                    .with_height(28.0)
                    .finish(),
                )
                .with_background(theme::accent_bg(30))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(14.0)))
                .finish(),
            )
            .with_child(
                Container::new({
                    let mut text_col = Flex::column().with_main_axis_size(MainAxisSize::Min);
                    text_col.add_child(
                        ui_text::chat_bubble_meta(attachment.name.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    );
                    text_col.add_child(
                        ui_text::chat_bubble_meta(format_file_size(attachment.size), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    );
                    text_col.finish()
                })
                .with_margin_left(8.0)
                .finish(),
            )
            .finish()
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

        let mut bubble_col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        if !self.attachments.is_empty() {
            bubble_col.add_child(
                Container::new(self.render_attachments())
                    .with_margin_bottom(4.0)
                    .finish(),
            );
        }
        if !self.body.is_empty() {
            bubble_col.add_child(
                ui_text::chat_bubble_text(self.body.clone(), self.body_font())
                    .with_color(theme::text())
                    .finish(),
            );
        }
        if !self.timestamp.is_empty() || (self.outgoing && self.read) {
            bubble_col.add_child(
                Container::new(Align::new(meta_row.finish()).right().finish())
                    .with_margin_top(2.0)
                    .finish(),
            );
        }

        let mut bubble = Container::new(
            ConstrainedBox::new(bubble_col.finish())
                .with_max_width(self.max_bubble_width)
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

fn attachment_kind_label(kind: &str) -> &'static str {
    match kind {
        "image" => "图片",
        "video" => "视频",
        "document" => "文档",
        _ => "附件",
    }
}

pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn normalize_timestamp_ms(timestamp: u64) -> i64 {
    if timestamp == 0 {
        return 0;
    }
    if timestamp < 1_000_000_000_000 {
        (timestamp as i64) * 1000
    } else {
        timestamp as i64
    }
}

fn format_message_time(timestamp: u64) -> String {
    let millis = normalize_timestamp_ms(timestamp);
    if millis == 0 {
        return String::new();
    }
    let secs = millis.div_euclid(1000);
    let nsec = (millis.rem_euclid(1000) * 1_000_000) as u32;
    chrono::Local
        .timestamp_opt(secs, nsec)
        .single()
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_default()
}

pub fn format_message_time_pub(timestamp: u64) -> String {
    format_message_time(timestamp)
}

pub fn outgoing_message_read(sent_at: u64, is_last_outgoing: bool) -> bool {
    if !is_last_outgoing {
        return true;
    }
    let sent_at_ms = normalize_timestamp_ms(sent_at);
    if sent_at_ms == 0 {
        return false;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    now_ms.saturating_sub(sent_at_ms) >= 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn format_file_size_formats_units() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(2048), "2.0 KB");
        assert_eq!(format_file_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn normalize_timestamp_ms_treats_legacy_seconds() {
        assert_eq!(normalize_timestamp_ms(1_731_637_500), 1_731_637_500_000);
        assert_eq!(normalize_timestamp_ms(1_731_637_500_000), 1_731_637_500_000);
    }

    #[test]
    fn format_message_time_matches_local_hhmm() {
        let millis = 1_731_637_500_123i64;
        let secs = millis.div_euclid(1000);
        let nsec = (millis.rem_euclid(1000) * 1_000_000) as u32;
        let expected = chrono::Local
            .timestamp_opt(secs, nsec)
            .single()
            .unwrap()
            .format("%H:%M")
            .to_string();
        assert_eq!(format_message_time(millis as u64), expected);
    }

    #[test]
    fn format_message_time_accepts_second_precision_legacy_values() {
        let secs = 1_731_637_500u64;
        assert_eq!(format_message_time(secs), format_message_time(secs * 1000));
    }

    #[test]
    fn format_message_time_zero_is_empty() {
        assert!(format_message_time(0).is_empty());
    }
}
