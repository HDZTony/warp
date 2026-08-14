use chrono::TimeZone;
use std::collections::HashMap;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Align, AutomationTarget, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Empty, EventHandler, Expanded, Flex, Image, MainAxisSize, ParentElement,
    Radius, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::chat::layout::{
    attachment_placeholder_size, attachment_preview_width, bubble_corner_radius,
    fit_attachment_preview, ATTACH_PREVIEW_MAX_EDGE, ATTACH_PREVIEW_MAX_HEIGHT,
};
use crate::ui::chat::shell_state::{
    ForwardDraft, ImageContextMenu, ImageViewerState, ReplyDraft, SharedChatShellState,
};
use crate::ui::icons;
use crate::ui::panel_primitives::{
    chat_bubble_in_bg, chat_bubble_out_bg, chat_bubble_out_border, TG_BUBBLE_MAX_WIDTH,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::ChatAttachmentDto;

const ATTACH_CHIP_ICON: f32 = 16.0;

#[derive(Debug, Clone)]
pub enum ChatBubbleAction {
    OpenImage {
        attachment_id: String,
        local_path: Option<String>,
        name: String,
        asset_id: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
    },
    OpenImageMenu {
        attachment_id: String,
        local_path: Option<String>,
        name: String,
        asset_id: Option<String>,
        x: f32,
        y: f32,
    },
    Reply,
}

pub struct ChatBubbleView {
    font: FamilyId,
    emoji_font: FamilyId,
    message_id: String,
    body: String,
    attachments: Vec<ChatAttachmentDto>,
    /// attachment_id → AssetCache id for decoded image previews
    image_assets: HashMap<String, String>,
    image_sizes: HashMap<String, (u32, u32)>,
    outgoing: bool,
    timestamp: String,
    system: bool,
    grouped: bool,
    read: bool,
    search_hit: bool,
    max_bubble_width: f32,
    reply_preview: Option<String>,
    forwarded_from: Option<String>,
    shell_state: SharedChatShellState,
}

impl ChatBubbleView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        message_id: String,
        body: String,
        attachments: Vec<ChatAttachmentDto>,
        outgoing: bool,
        timestamp: String,
        grouped: bool,
        read: bool,
        search_hit: bool,
        max_bubble_width: f32,
        image_assets: HashMap<String, String>,
        image_sizes: HashMap<String, (u32, u32)>,
        reply_preview: Option<String>,
        forwarded_from: Option<String>,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        Self {
            font,
            emoji_font,
            message_id,
            body,
            attachments,
            image_assets,
            image_sizes,
            outgoing,
            timestamp,
            system: false,
            grouped,
            read,
            search_hit,
            max_bubble_width,
            reply_preview,
            forwarded_from,
            shell_state,
        }
    }

    pub fn system_hint(ctx: &mut ViewContext<Self>, body: String) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        Self {
            font,
            emoji_font,
            message_id: String::new(),
            body,
            attachments: Vec::new(),
            image_assets: HashMap::new(),
            image_sizes: HashMap::new(),
            outgoing: false,
            timestamp: String::new(),
            system: true,
            grouped: false,
            read: false,
            search_hit: false,
            max_bubble_width: TG_BUBBLE_MAX_WIDTH,
            reply_preview: None,
            forwarded_from: None,
            shell_state: crate::ui::chat::shell_state::new_shared_shell_state(),
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
        let max_w = attachment_preview_width(self.max_bubble_width).min(ATTACH_PREVIEW_MAX_EDGE);
        let (preview_w, preview_h) = if let Some((sw, sh)) = self.image_sizes.get(&attachment.id) {
            fit_attachment_preview(*sw, *sh, max_w, ATTACH_PREVIEW_MAX_HEIGHT)
        } else {
            attachment_placeholder_size(self.max_bubble_width)
        };

        if attachment.kind == "image" {
            if let Some(asset_id) = self.image_assets.get(&attachment.id) {
                let attachment_id = attachment.id.clone();
                let local_path = attachment.local_path.clone();
                let name = attachment.name.clone();
                let asset_id_for_open = asset_id.clone();
                let size = self.image_sizes.get(&attachment.id).copied();
                let image = ConstrainedBox::new(
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
                .with_width(preview_w)
                .with_height(preview_h)
                .finish();

                let interactive = EventHandler::new(image)
                    .skip_automation()
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(ChatBubbleAction::OpenImage {
                            attachment_id: attachment_id.clone(),
                            local_path: local_path.clone(),
                            name: name.clone(),
                            asset_id: Some(asset_id_for_open.clone()),
                            width: size.map(|(w, _)| w),
                            height: size.map(|(_, h)| h),
                        });
                        DispatchEventResult::StopPropagation
                    })
                    .on_right_mouse_down({
                        let attachment_id = attachment.id.clone();
                        let local_path = attachment.local_path.clone();
                        let name = attachment.name.clone();
                        let asset_id = asset_id.clone();
                        move |ctx, _, position| {
                            ctx.dispatch_typed_action(ChatBubbleAction::OpenImageMenu {
                                attachment_id: attachment_id.clone(),
                                local_path: local_path.clone(),
                                name: name.clone(),
                                asset_id: Some(asset_id.clone()),
                                x: position.x(),
                                y: position.y(),
                            });
                            DispatchEventResult::StopPropagation
                        }
                    })
                    .finish();

                return AutomationTarget::new(interactive)
                    .with_label(wormhole_i18n::t("chat.image.open"))
                    .with_id(format!("chat:image_{}", attachment.id))
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
                .with_height((preview_h - 28.0).max(40.0))
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
            .with_width(preview_w)
            .with_height(preview_h)
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
                .with_background(theme::accent_bg(24))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish(),
            )
            .with_child(
                Container::new(
                    Flex::column()
                        .with_main_axis_size(MainAxisSize::Min)
                        .with_cross_axis_alignment(CrossAxisAlignment::Start)
                        .with_child(
                            ui_text::chat_bubble_meta(attachment.name.clone(), self.font)
                                .with_color(theme::text())
                                .finish(),
                        )
                        .with_child(
                            ui_text::chat_bubble_meta(format_file_size(attachment.size), self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .finish(),
                )
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
                    ui_text::chat_bubble_meta(self.body.clone(), self.font)
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

        if let Some(forwarded) = &self.forwarded_from {
            bubble_col.add_child(
                Container::new(
                    ui_text::chat_bubble_meta(
                        format!("{} {}", wormhole_i18n::t("chat.image.forwarded_from"), forwarded),
                        self.font,
                    )
                    .with_color(theme::accent_cool())
                    .finish(),
                )
                .with_margin_bottom(4.0)
                .finish(),
            );
        }

        if let Some(reply) = &self.reply_preview {
            bubble_col.add_child(
                Container::new(
                    Flex::column()
                        .with_main_axis_size(MainAxisSize::Min)
                        .with_child(
                            ui_text::chat_bubble_meta(
                                wormhole_i18n::t("chat.image.reply"),
                                self.font,
                            )
                            .with_color(theme::accent_cool())
                            .finish(),
                        )
                        .with_child(
                            ui_text::chat_bubble_meta(reply.clone(), self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .finish(),
                )
                .with_padding_left(8.0)
                .with_margin_bottom(6.0)
                .with_border(Border::left(2.0).with_border_fill(theme::accent_cool()))
                .finish(),
            );
        }

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

        let bubble = if self.system || self.message_id.is_empty() {
            bubble.finish()
        } else {
            EventHandler::new(bubble.finish())
                .skip_automation()
                .on_right_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(ChatBubbleAction::Reply);
                    DispatchEventResult::PropagateToParent
                })
                .finish()
        };

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

impl TypedActionView for ChatBubbleView {
    type Action = ChatBubbleAction;

    fn handle_action(&mut self, action: &ChatBubbleAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatBubbleAction::OpenImage {
                attachment_id,
                local_path,
                name,
                asset_id,
                width,
                height,
            } => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.open_image_viewer(ImageViewerState {
                        attachment_id: attachment_id.clone(),
                        message_id: self.message_id.clone(),
                        asset_id: asset_id.clone(),
                        local_path: local_path.clone(),
                        name: name.clone(),
                        source_width: *width,
                        source_height: *height,
                    });
                }
                ctx.notify();
            }
            ChatBubbleAction::OpenImageMenu {
                attachment_id,
                local_path,
                name,
                asset_id,
                x,
                y,
            } => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.image_context_menu = Some(ImageContextMenu {
                        attachment_id: attachment_id.clone(),
                        message_id: self.message_id.clone(),
                        local_path: local_path.clone(),
                        name: name.clone(),
                        asset_id: asset_id.clone(),
                        x: *x,
                        y: *y,
                    });
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            ChatBubbleAction::Reply => {
                let has_image = self.attachments.iter().any(|a| a.kind == "image");
                let preview = if self.body.trim().is_empty() && has_image {
                    wormhole_i18n::t("chat.preview.image")
                } else if self.body.trim().is_empty() {
                    wormhole_i18n::t("chat.preview.file")
                } else {
                    self.body.clone()
                };
                if let Ok(mut state) = self.shell_state.lock() {
                    state.set_reply_draft(ReplyDraft {
                        message_id: self.message_id.clone(),
                        preview,
                        has_image,
                    });
                }
                ctx.notify();
            }
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &ChatBubbleAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let label = match action {
            ChatBubbleAction::OpenImage { .. } => wormhole_i18n::t("chat.image.open"),
            ChatBubbleAction::OpenImageMenu { .. } => wormhole_i18n::t("chat.image.menu"),
            ChatBubbleAction::Reply => wormhole_i18n::t("chat.image.reply"),
        };
        ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
            label,
            WarpA11yRole::ButtonRole,
        ))
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
    let _ = sent_at;
    is_last_outgoing
}

/// Helper used by image context / viewer forward action.
pub fn forward_draft_from_attachment(
    attachment: &ChatAttachmentDto,
    forwarded_from: String,
) -> Option<ForwardDraft> {
    let local_path = attachment.local_path.clone()?;
    Some(ForwardDraft {
        local_path,
        name: attachment.name.clone(),
        kind: attachment.kind.clone(),
        size: attachment.size,
        forwarded_from,
    })
}
