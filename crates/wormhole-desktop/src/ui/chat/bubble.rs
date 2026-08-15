use chrono::TimeZone;
use std::collections::HashMap;
use std::path::PathBuf;
use pathfinder_geometry::vector::vec2f;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Align, AutomationTarget, Border, ChildAnchor, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, Empty, EventHandler, Expanded, Flex, Image,
    MainAxisAlignment, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Radius, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::chat::layout::{
    attachment_loading_chip_size, attachment_preview_width, bubble_corner_radius,
    fit_attachment_preview, ATTACH_PREVIEW_MAX_EDGE, ATTACH_PREVIEW_MAX_HEIGHT,
};
use crate::ui::chat::shell_state::{
    AttachmentContextMenu, ForwardDraft, ImageViewerState, ReplyDraft, SharedChatShellState,
};
use crate::ui::icons;
use crate::ui::panel_primitives::{
    chat_bubble_in_bg, chat_bubble_out_bg, StatusTone, TG_BUBBLE_MAX_WIDTH,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::ChatAttachmentDto;
use wormhole_desktop_core::cluster_commands::open_path_with_system;

const ATTACH_CHIP_ICON: f32 = 16.0;
const BUBBLE_PAD_X: f32 = 11.0;
const BUBBLE_PAD_TOP: f32 = 8.0;
const BUBBLE_PAD_BOTTOM: f32 = 8.0;
/// Space reserved under bubble content so positioned timestamp does not cover text.
const META_RESERVE: f32 = 18.0;

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
    OpenAttachmentMenu {
        attachment_id: String,
        kind: String,
        local_path: Option<String>,
        name: String,
        size: u64,
        asset_id: Option<String>,
        x: f32,
        y: f32,
    },
    OpenFile {
        local_path: Option<String>,
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

    /// Single image (or image+caption) Telegram photo bubble without side padding on the image.
    fn is_photo_bubble(&self) -> bool {
        self.attachments.len() == 1 && self.attachments[0].kind == "image"
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
            return self.render_media_attachment(attachment, false);
        }
        self.render_file_attachment(attachment)
    }

    fn render_media_attachment(
        &self,
        attachment: &ChatAttachmentDto,
        edge_to_edge: bool,
    ) -> Box<dyn Element> {
        let max_w = attachment_preview_width(self.max_bubble_width).min(ATTACH_PREVIEW_MAX_EDGE);
        let decoded = self.image_sizes.get(&attachment.id).copied();
        let (preview_w, preview_h) = if let Some((sw, sh)) = decoded {
            fit_attachment_preview(sw, sh, max_w, ATTACH_PREVIEW_MAX_HEIGHT)
        } else {
            attachment_loading_chip_size(self.max_bubble_width)
        };

        if attachment.kind == "image" {
            if let Some(asset_id) = self.image_assets.get(&attachment.id) {
                let attachment_id = attachment.id.clone();
                let local_path = attachment.local_path.clone();
                let name = attachment.name.clone();
                let asset_id_for_open = asset_id.clone();
                let size = decoded;
                let radius = if edge_to_edge {
                    bubble_corner_radius(self.outgoing, self.grouped)
                } else {
                    CornerRadius::with_all(Radius::Pixels(8.0))
                };
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
                    .with_corner_radius(radius)
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
                        let kind = attachment.kind.clone();
                        let size_bytes = attachment.size;
                        move |ctx, _, position| {
                            ctx.dispatch_typed_action(ChatBubbleAction::OpenAttachmentMenu {
                                attachment_id: attachment_id.clone(),
                                kind: kind.clone(),
                                local_path: local_path.clone(),
                                name: name.clone(),
                                size: size_bytes,
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

        // Loading / video placeholder — compact chip, never a fixed 280×210 slab.
        let label = if attachment.kind == "video" {
            attachment_kind_label(&attachment.kind).to_string()
        } else {
            wormhole_i18n::t("chat.preview.image")
        };
        let chip = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(
                ui_text::chat_bubble_meta(label, self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_child(
                Container::new(
                    ui_text::chat_bubble_meta(attachment.name.clone(), self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_margin_left(8.0)
                .finish(),
            )
            .finish();
        let boxed = ConstrainedBox::new(
            Container::new(chip)
                .with_uniform_padding(8.0)
                .with_background(theme::accent_bg(24))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish(),
        )
        .with_width(preview_w)
        .with_height(preview_h)
        .finish();

        if attachment.kind == "video" {
            return self.wrap_file_interactions(attachment, boxed);
        }
        boxed
    }

    fn render_file_attachment(&self, attachment: &ChatAttachmentDto) -> Box<dyn Element> {
        let icon_path = if attachment.kind == "document" {
            "chat-attach-document.svg"
        } else {
            "chat-compose-attach.svg"
        };
        let chip = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(
                Container::new(
                    ConstrainedBox::new(
                        Flex::row()
                            .with_main_axis_alignment(MainAxisAlignment::Center)
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
            .finish();

        let padded = Container::new(chip)
            .with_uniform_padding(4.0)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish();
        self.wrap_file_interactions(attachment, padded)
    }

    fn wrap_file_interactions(
        &self,
        attachment: &ChatAttachmentDto,
        child: Box<dyn Element>,
    ) -> Box<dyn Element> {
        let local_path_open = attachment.local_path.clone();
        let local_path_menu = attachment.local_path.clone();
        let attachment_id = attachment.id.clone();
        let kind = attachment.kind.clone();
        let name = attachment.name.clone();
        let size = attachment.size;
        let interactive = EventHandler::new(child)
            .skip_automation()
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatBubbleAction::OpenFile {
                    local_path: local_path_open.clone(),
                });
                DispatchEventResult::StopPropagation
            })
            .on_right_mouse_down(move |ctx, _, position| {
                ctx.dispatch_typed_action(ChatBubbleAction::OpenAttachmentMenu {
                    attachment_id: attachment_id.clone(),
                    kind: kind.clone(),
                    local_path: local_path_menu.clone(),
                    name: name.clone(),
                    size,
                    asset_id: None,
                    x: position.x(),
                    y: position.y(),
                });
                DispatchEventResult::StopPropagation
            })
            .finish();

        AutomationTarget::new(interactive)
            .with_label(wormhole_i18n::t("chat.file.open"))
            .with_id(format!("chat:file_{}", attachment.id))
            .finish()
    }

    fn has_meta(&self) -> bool {
        !self.timestamp.is_empty() || (self.outgoing && self.read)
    }

    fn meta_row(&self) -> Box<dyn Element> {
        let mut meta_row = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        let time_color = if self.outgoing {
            theme::accent_cool()
        } else {
            theme::muted()
        };
        if !self.timestamp.is_empty() {
            meta_row.add_child(
                ui_text::chat_bubble_meta(self.timestamp.clone(), self.font)
                    .with_color(time_color)
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
        meta_row.finish()
    }

    /// Timestamp badge that does **not** expand Stack size (unlike Align).
    ///
    /// Must use `add_positioned_child` (not overlay): WarpUI Overlay paints with
    /// `ClipBounds::None` and escapes `ClippedScrollable`, floating over compose.
    /// Aligns with tdesktop `InfoDisplayType::Image` (`msgDateImgDelta` /
    /// `msgDateImgPadding`).
    fn positioned_meta_badge(&self, on_photo: bool) -> Box<dyn Element> {
        let badge = if on_photo {
            Container::new(self.meta_row())
                .with_horizontal_padding(8.0)
                .with_vertical_padding(2.0)
                .with_background(pathfinder_color::ColorU::new(0, 0, 0, 110))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish()
        } else {
            self.meta_row()
        };
        badge
    }

    fn with_bottom_right_meta(&self, content: Box<dyn Element>, on_photo: bool) -> Box<dyn Element> {
        if !self.has_meta() {
            return content;
        }
        let mut stack = Stack::new();
        stack.add_child(content);
        // tdesktop msgDateImgDelta = 4px; text/file bubbles sit flush in padding.
        let inset = if on_photo { 4.0 } else { 0.0 };
        stack.add_positioned_child(
            self.positioned_meta_badge(on_photo),
            OffsetPositioning::offset_from_parent(
                vec2f(-inset, -inset),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::BottomRight,
                ChildAnchor::BottomRight,
            ),
        );
        stack.finish()
    }

    fn photo_preview_size(&self, attachment: &ChatAttachmentDto) -> (f32, f32) {
        let max_w = attachment_preview_width(self.max_bubble_width).min(ATTACH_PREVIEW_MAX_EDGE);
        if let Some((sw, sh)) = self.image_sizes.get(&attachment.id) {
            fit_attachment_preview(*sw, *sh, max_w, ATTACH_PREVIEW_MAX_HEIGHT)
        } else {
            attachment_loading_chip_size(self.max_bubble_width)
        }
    }

    fn header_bits(&self, bubble_col: &mut Flex, max_width: Option<f32>) {
        if let Some(forwarded) = &self.forwarded_from {
            let label = Container::new(
                ui_text::chat_bubble_meta(
                    format!(
                        "{} {}",
                        wormhole_i18n::t("chat.image.forwarded_from"),
                        forwarded
                    ),
                    self.font,
                )
                .with_color(theme::accent_cool())
                .finish(),
            )
            .with_margin_bottom(4.0)
            .with_padding_left(BUBBLE_PAD_X)
            .with_padding_right(BUBBLE_PAD_X)
            .with_padding_top(BUBBLE_PAD_TOP)
            .finish();
            bubble_col.add_child(match max_width {
                Some(w) => ConstrainedBox::new(label).with_max_width(w).finish(),
                None => label,
            });
        }

        if let Some(reply) = &self.reply_preview {
            let reply_block = Container::new(
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
            .with_margin_left(BUBBLE_PAD_X)
            .with_margin_right(BUBBLE_PAD_X)
            .with_border(Border::left(2.0).with_border_fill(theme::accent_cool()))
            .finish();
            bubble_col.add_child(match max_width {
                Some(w) => ConstrainedBox::new(reply_block).with_max_width(w).finish(),
                None => reply_block,
            });
        }
    }

    fn render_photo_bubble(&self, bg: pathfinder_color::ColorU, border: pathfinder_color::ColorU) -> Box<dyn Element> {
        let attachment = &self.attachments[0];
        let photo_only = self.body.trim().is_empty();
        let (preview_w, _preview_h) = self.photo_preview_size(attachment);
        let photo = self.render_media_attachment(attachment, true);
        let photo = if photo_only {
            self.with_bottom_right_meta(photo, true)
        } else {
            photo
        };

        let mut bubble_col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        self.header_bits(&mut bubble_col, Some(preview_w));
        bubble_col.add_child(photo);

        if !photo_only {
            bubble_col.add_child(
                Container::new(
                    ui_text::chat_bubble_text(self.body.clone(), self.body_font())
                        .with_color(theme::chat_bubble_text())
                        .finish(),
                )
                .with_padding_left(BUBBLE_PAD_X)
                .with_padding_right(BUBBLE_PAD_X)
                .with_padding_top(6.0)
                .with_padding_bottom(if self.has_meta() {
                    META_RESERVE
                } else {
                    BUBBLE_PAD_BOTTOM
                })
                .finish(),
            );
            let content = ConstrainedBox::new(bubble_col.finish())
                .with_max_width(self.max_bubble_width)
                .finish();
            // Caption bubbles: hug content width, meta overlay bottom-right.
            let content = if self.has_meta() {
                self.with_bottom_right_meta(
                    Container::new(content)
                        .with_padding_right(BUBBLE_PAD_X)
                        .with_padding_bottom(BUBBLE_PAD_BOTTOM)
                        .finish(),
                    false,
                )
            } else {
                content
            };
            let radius = bubble_corner_radius(self.outgoing, self.grouped);
            let mut bubble = Container::new(content)
                .with_background(bg)
                .with_corner_radius(radius);
            if self.search_hit {
                bubble = bubble.with_border(Border::all(2.0).with_border_fill(theme::accent_cool()));
            }
            return bubble.finish();
        }

        // Photo-only: column width follows image (+ optional capped header).
        let content = ConstrainedBox::new(bubble_col.finish())
            .with_max_width(preview_w)
            .finish();
        let radius = bubble_corner_radius(self.outgoing, self.grouped);
        let mut bubble = Container::new(content)
            .with_background(bg)
            .with_corner_radius(radius);
        if self.search_hit {
            bubble = bubble.with_border(Border::all(2.0).with_border_fill(theme::accent_cool()));
        }
        bubble.finish()
    }

    fn render_text_bubble(&self, bg: pathfinder_color::ColorU, _border: pathfinder_color::ColorU) -> Box<dyn Element> {
        let radius = bubble_corner_radius(self.outgoing, self.grouped);
        let mut bubble_col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);

        if let Some(forwarded) = &self.forwarded_from {
            bubble_col.add_child(
                Container::new(
                    ui_text::chat_bubble_meta(
                        format!(
                            "{} {}",
                            wormhole_i18n::t("chat.image.forwarded_from"),
                            forwarded
                        ),
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
                    .with_color(theme::chat_bubble_text())
                    .finish(),
            );
        }
        // Reserve space for positioned timestamp so it does not cover the last line.
        if self.has_meta() {
            bubble_col.add_child(
                ConstrainedBox::new(Empty::new().finish())
                    .with_height(META_RESERVE)
                    .finish(),
            );
        }

        let inner = ConstrainedBox::new(bubble_col.finish())
            .with_max_width(self.max_bubble_width)
            .finish();
        let content = if self.has_meta() {
            self.with_bottom_right_meta(inner, false)
        } else {
            inner
        };

        let mut bubble = Container::new(content)
            .with_padding_left(BUBBLE_PAD_X)
            .with_padding_right(BUBBLE_PAD_X)
            .with_padding_top(BUBBLE_PAD_TOP)
            .with_padding_bottom(BUBBLE_PAD_BOTTOM)
            .with_background(bg)
            .with_corner_radius(radius);
        if self.search_hit {
            bubble = bubble.with_border(Border::all(2.0).with_border_fill(theme::accent_cool()));
        }
        bubble.finish()
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
                        .with_color(theme::text())
                        .finish(),
                )
                .with_horizontal_padding(12.0)
                .with_vertical_padding(4.0)
                .with_background(crate::ui::panel_primitives::chat_date_bg())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
                .finish(),
            )
            .finish();
        }

        let bg = if self.outgoing {
            chat_bubble_out_bg()
        } else {
            chat_bubble_in_bg()
        };

        let bubble = if self.is_photo_bubble() {
            self.render_photo_bubble(bg, theme::border())
        } else {
            self.render_text_bubble(bg, theme::border())
        };

        let bubble = if self.system || self.message_id.is_empty() {
            bubble
        } else {
            EventHandler::new(bubble)
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
            ChatBubbleAction::OpenAttachmentMenu {
                attachment_id,
                kind,
                local_path,
                name,
                size,
                asset_id,
                x,
                y,
            } => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.open_attachment_menu(AttachmentContextMenu {
                        attachment_id: attachment_id.clone(),
                        message_id: self.message_id.clone(),
                        kind: kind.clone(),
                        local_path: local_path.clone(),
                        name: name.clone(),
                        size: *size,
                        asset_id: asset_id.clone(),
                        x: *x,
                        y: *y,
                    });
                }
                ctx.notify();
            }
            ChatBubbleAction::OpenFile { local_path } => {
                let Some(path) = local_path.as_ref() else {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t("chat.file.not_ready"),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                    return;
                };
                match open_path_with_system(PathBuf::from(path).as_path()) {
                    Ok(()) => {}
                    Err(err) => {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.show_toast(err, StatusTone::Danger);
                        }
                    }
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
            ChatBubbleAction::OpenAttachmentMenu { kind, .. } if kind == "image" => {
                wormhole_i18n::t("chat.image.menu")
            }
            ChatBubbleAction::OpenAttachmentMenu { .. } => wormhole_i18n::t("chat.file.menu"),
            ChatBubbleAction::OpenFile { .. } => wormhole_i18n::t("chat.file.open"),
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

/// Sidebar last-message time: today `HH:MM`, yesterday `昨天`, else `YYYY/M/D`.
pub fn format_sidebar_time(timestamp: u64) -> String {
    use chrono::{Datelike, Duration, Local, TimeZone};
    let millis = normalize_timestamp_ms(timestamp);
    if millis == 0 {
        return String::new();
    }
    let secs = millis.div_euclid(1000);
    let nsec = (millis.rem_euclid(1000) * 1_000_000) as u32;
    let Some(dt) = Local.timestamp_opt(secs, nsec).single() else {
        return String::new();
    };
    let date = dt.date_naive();
    let today = Local::now().date_naive();
    if date == today {
        return dt.format("%H:%M").to_string();
    }
    if date == today - Duration::days(1) {
        return "昨天".into();
    }
    format!("{}/{}/{}", date.year(), date.month(), date.day())
}

#[cfg(test)]
mod sidebar_time_tests {
    use super::{format_message_time, format_sidebar_time, normalize_timestamp_ms};
    use chrono::{Datelike, Duration, Local};

    fn ms_for_local_date(days_ago: i64, hour: u32, min: u32) -> u64 {
        let date = Local::now().date_naive() - Duration::days(days_ago);
        let dt = date
            .and_hms_opt(hour, min, 0)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .expect("local datetime");
        dt.timestamp_millis() as u64
    }

    #[test]
    fn format_sidebar_time_today_hhmm() {
        let ms = ms_for_local_date(0, 14, 5);
        assert_eq!(format_sidebar_time(ms), "14:05");
        assert_eq!(format_message_time(ms), "14:05");
    }

    #[test]
    fn format_sidebar_time_yesterday_label() {
        let ms = ms_for_local_date(1, 9, 30);
        assert_eq!(format_sidebar_time(ms), "昨天");
    }

    #[test]
    fn format_sidebar_time_older_ymd() {
        let date = Local::now().date_naive() - Duration::days(10);
        let dt = date
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .unwrap();
        let ms = dt.timestamp_millis() as u64;
        assert_eq!(
            format_sidebar_time(ms),
            format!("{}/{}/{}", date.year(), date.month(), date.day())
        );
    }

    #[test]
    fn format_sidebar_time_empty_zero() {
        assert_eq!(format_sidebar_time(0), "");
        assert_eq!(normalize_timestamp_ms(0), 0);
    }
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
