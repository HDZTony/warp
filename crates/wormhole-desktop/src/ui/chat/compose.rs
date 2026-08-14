use std::path::PathBuf;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    AutomationTarget, Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex, Image, MainAxisAlignment,
    MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius,
    Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::chat::attach_panel::{
    picker_config_for_kind, prepare_dropped_file, prepare_staged_file, prepare_staged_file_with_kind,
    AttachKind, PreparedStagedFile,
};
use crate::ui::chat::bubble::format_file_size;
use crate::ui::chat::image_asset::{
    decode_image_asset_from_path, insert_attachment_image_payload,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::{PendingOutgoingAttachment, SharedChatShellState};
use crate::ui::chat::sticker_picker::{StickerPickerEvent, StickerPickerView};
use crate::ui::clipboard::{
    clipboard_image_kind_and_ext, paths_from_clipboard_content, read_clipboard_existing_paths,
    read_clipboard_image_png, read_clipboard_text,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons::{self, CHAT_COMPOSE_BTN};
use crate::ui::multiline_input;
use crate::ui::panel_primitives::{
    popover_icon_label_option, popover_shell_with_radius, status_line, StatusTone,
};
use crate::ui::text_field_input::{
    compose_input_height, render_compose_field_with_caret, sync_caret_blink, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_send_message, SendChatAttachmentDto, SendChatMessageParams,
};

const TG_COMPOSE_GAP: f32 = 8.0;
const ATTACH_POPOVER_WIDTH: f32 = 196.0;
const ATTACH_POPOVER_RADIUS: f32 = 12.0;
const ATTACH_ICON_SIZE: f32 = 36.0;
const ATTACH_ICON_GLYPH: f32 = 18.0;
const STAGED_THUMB: f32 = 56.0;
const STAGED_STRIP_PAD: f32 = 8.0;

#[derive(Debug, Clone)]
pub enum ChatComposeAction {
    Send,
    FocusInput,
    ToggleFocus,
    ToggleStickerPicker,
    ToggleAttachPanel,
    PickAttachment(AttachKind),
    StagePicked {
        kind: AttachKind,
        paths: Vec<PathBuf>,
    },
    PickerFailed(String),
    RemoveStaged(usize),
    PasteClipboard,
    ClearAndUnfocus,
    ClearReply,
    TextEdit(TextFieldEditAction),
}

#[derive(Debug, Clone)]
struct StagedAttachment {
    id: String,
    path: PathBuf,
    kind: String,
    name: String,
    size: u64,
    preview_asset_id: Option<String>,
    delete_on_clear: bool,
}

pub struct ChatComposeView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    emoji_font: FamilyId,
    draft: String,
    field_state: TextFieldState,
    status: String,
    status_tone: StatusTone,
    input_focused: bool,
    caret_blink: CaretBlink,
    sending: bool,
    sticker_open: bool,
    attach_open: bool,
    sticker_picker: warpui::ViewHandle<StickerPickerView>,
    staged: Vec<StagedAttachment>,
    staged_conv_id: Option<String>,
    picking: bool,
}

impl ChatComposeView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        let sticker_picker = ctx.add_typed_action_view(StickerPickerView::new);
        ctx.subscribe_to_view(&sticker_picker, |view, _, event, ctx| {
            let StickerPickerEvent::InsertEmoji(emoji) = event;
            view.field_state.apply(
                &mut view.draft,
                &TextFieldEditAction::TypedCharacters(emoji.clone()),
            );
            view.input_focused = true;
            sync_caret_blink(view, ctx);
            ctx.notify();
        });
        Self {
            core,
            selection,
            shell_state,
            font,
            emoji_font,
            draft: String::new(),
            field_state: TextFieldState::new(),
            status: String::new(),
            status_tone: StatusTone::Neutral,
            input_focused: false,
            caret_blink: CaretBlink::new(),
            sending: false,
            sticker_open: false,
            attach_open: false,
            sticker_picker,
            staged: Vec::new(),
            staged_conv_id: None,
            picking: false,
        }
    }

    /// Resolve the active conversation id for send paths. Distinguishes pending open
    /// and open failures from a true "no session selected" state.
    fn require_selected_conversation(&mut self, ctx: &mut ViewContext<Self>) -> Option<String> {
        if let Some(id) = self.selection.lock().ok().and_then(|g| g.clone()) {
            self.clear_opening_status();
            return Some(id);
        }
        let (pending, open_error) = self
            .shell_state
            .lock()
            .map(|state| (state.pending_open.is_some(), state.open_error.clone()))
            .unwrap_or((false, None));
        if pending {
            self.status = wormhole_i18n::t("chat.header.opening");
            self.status_tone = StatusTone::Warn;
            ctx.notify();
            return None;
        }
        if let Some(err) = open_error {
            self.status = err;
            self.status_tone = StatusTone::Danger;
            ctx.notify();
            return None;
        }
        self.status = wormhole_i18n::t("chat.compose.select_conversation");
        self.status_tone = StatusTone::Warn;
        ctx.notify();
        None
    }

    pub fn selection_changed(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = self.selection.lock().ok().and_then(|g| g.clone());
        if conv_id.is_some() {
            self.clear_opening_status();
        }
        if conv_id != self.staged_conv_id {
            if self.staged_conv_id.is_none() && conv_id.is_some() && !self.staged.is_empty() {
                self.staged_conv_id = conv_id;
            } else {
                self.clear_staged();
            }
            ctx.notify();
        }
    }

    fn clear_opening_status(&mut self) {
        if self.status == wormhole_i18n::t("chat.header.opening") {
            self.status.clear();
            self.status_tone = StatusTone::Neutral;
        }
    }

    pub fn stage_dropped_paths(&mut self, paths: Vec<String>, ctx: &mut ViewContext<Self>) {
        let Some(conv_id) = self.require_selected_conversation(ctx) else {
            return;
        };
        self.bind_staged_conv(conv_id);
        let mut files = Vec::new();
        for path in paths {
            match prepare_dropped_file(PathBuf::from(path)) {
                Ok(file) => files.push(file),
                Err(err) => {
                    self.status = err.message();
                    self.status_tone = StatusTone::Danger;
                }
            }
        }
        self.push_staged_files(files, ctx);
    }

    fn bind_staged_conv(&mut self, conv_id: String) {
        if self.staged_conv_id.as_ref() != Some(&conv_id) {
            self.clear_staged();
            self.staged_conv_id = Some(conv_id);
        }
    }

    fn clear_staged(&mut self) {
        for item in self.staged.drain(..) {
            if item.delete_on_clear {
                let _ = std::fs::remove_file(&item.path);
            }
        }
        self.staged_conv_id = None;
    }

    fn push_staged_files(&mut self, files: Vec<PreparedStagedFile>, ctx: &mut ViewContext<Self>) {
        if files.is_empty() {
            ctx.notify();
            return;
        }
        self.status.clear();
        self.status_tone = StatusTone::Neutral;
        self.attach_open = false;
        let mut pending_previews = Vec::new();
        for file in files {
            let id = uuid::Uuid::new_v4().to_string();
            if file.kind == "image" {
                pending_previews.push((id.clone(), file.path.clone()));
            }
            self.staged.push(StagedAttachment {
                id,
                path: file.path,
                kind: file.kind,
                name: file.name,
                size: file.size,
                preview_asset_id: None,
                delete_on_clear: false,
            });
        }
        self.input_focused = true;
        sync_caret_blink(self, ctx);
        ctx.notify();
        for (id, path) in pending_previews {
            self.queue_preview_decode(id, path, ctx);
        }
    }

    fn queue_preview_decode(
        &mut self,
        id: String,
        path: PathBuf,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || decode_image_asset_from_path(&path))
                    .await
                    .ok()
                    .and_then(Result::ok)
            },
            move |view, decoded, ctx| {
                if let Some(decoded) = decoded {
                    if let Some(item) = view.staged.iter_mut().find(|item| item.id == id) {
                        item.preview_asset_id = Some(insert_attachment_image_payload(
                            ctx,
                            &id,
                            decoded.payload,
                        ));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn send(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(conv_id) = self.require_selected_conversation(ctx) else {
            return;
        };
        let body = self.draft.trim().to_string();
        if body.is_empty() && self.staged.is_empty() {
            return;
        }

        let client_id = format!("pending:{}", uuid::Uuid::new_v4());
        let sent_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);

        let staged = std::mem::take(&mut self.staged);
        let staged_conv_id = self.staged_conv_id.take();
        self.draft.clear();
        self.field_state.clear_marked();
        self.sending = true;
        self.status = wormhole_i18n::t("chat.compose.sending");
        self.status_tone = StatusTone::Neutral;

        let pending_attachments = staged
            .iter()
            .map(|item| PendingOutgoingAttachment {
                kind: item.kind.clone(),
                name: item.name.clone(),
                size: item.size,
                local_path: Some(item.path.to_string_lossy().into_owned()),
            })
            .collect::<Vec<_>>();
        let attachments = staged
            .iter()
            .map(|item| SendChatAttachmentDto {
                kind: item.kind.clone(),
                path: item.path.to_string_lossy().into_owned(),
            })
            .collect::<Vec<_>>();

        let reply_to = self
            .shell_state
            .lock()
            .ok()
            .and_then(|mut state| state.reply_draft.take().map(|draft| draft.message_id));

        if let Ok(mut state) = self.shell_state.lock() {
            state.push_pending_outgoing(
                client_id.clone(),
                conv_id.clone(),
                body.clone(),
                sent_at,
                pending_attachments,
            );
            state.bump_message_tick();
        }
        ctx.notify();

        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let client_id_for_spawn = client_id.clone();
        let body_for_restore = body.clone();
        let staged_for_restore = staged.clone();
        let conv_id_for_restore = conv_id.clone();
        let staged_conv_id_for_restore = staged_conv_id;
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = SendChatMessageParams {
                    conv_id,
                    body,
                    backend: None,
                    peer_bootstrap_addrs: Vec::new(),
                    sticker: None,
                    attachments,
                    reply_to,
                    forwarded_from: None,
                };
                chat_send_message(runtime.ctx.as_ref(), &runtime.state, params).await
            },
            move |view, output, ctx| {
                view.sending = false;
                if view.status == wormhole_i18n::t("chat.compose.sending") {
                    view.status.clear();
                    view.status_tone = StatusTone::Neutral;
                }
                if let Ok(mut state) = shell_state.lock() {
                    state.remove_pending(&client_id_for_spawn);
                }
                match output {
                    Ok(_) => {
                        for item in &staged_for_restore {
                            if item.delete_on_clear {
                                let _ = std::fs::remove_file(&item.path);
                            }
                        }
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.bump_message_tick();
                        }
                    }
                    Err(e) => {
                        let current = view.selection.lock().ok().and_then(|g| g.clone());
                        if current.as_ref() == Some(&conv_id_for_restore) {
                            view.draft = body_for_restore;
                            view.staged = staged_for_restore;
                            view.staged_conv_id = staged_conv_id_for_restore
                                .or(Some(conv_id_for_restore));
                        } else {
                            for item in &staged_for_restore {
                                if item.delete_on_clear {
                                    let _ = std::fs::remove_file(&item.path);
                                }
                            }
                        }
                        view.status = if view.staged.is_empty() {
                            wormhole_i18n::t_args("chat.send_failed", &[("err", &e.to_string())])
                        } else {
                            wormhole_i18n::t_args(
                                "chat.attachment_send_failed",
                                &[("err", &e.to_string())],
                            )
                        };
                        view.status_tone = StatusTone::Danger;
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.bump_message_tick();
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn pick_attachment(&mut self, kind: AttachKind, ctx: &mut ViewContext<Self>) {
        self.attach_open = false;
        if kind == AttachKind::Location {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(
                    wormhole_i18n::t("chat.location_unsupported"),
                    StatusTone::Muted,
                );
            }
            ctx.notify();
            return;
        }
        if self.picking {
            return;
        }
        let conv_id = self.selection.lock().ok().and_then(|g| g.clone());
        if let Some(conv_id) = conv_id {
            self.bind_staged_conv(conv_id);
            self.clear_opening_status();
        } else {
            let pending = self
                .shell_state
                .lock()
                .map(|state| state.pending_open.is_some())
                .unwrap_or(false);
            if !pending {
                let _ = self.require_selected_conversation(ctx);
                return;
            }
        }
        let Some(config) = picker_config_for_kind(kind) else {
            return;
        };
        self.picking = true;
        ctx.notify();
        ctx.open_file_picker(
            move |result, ctx| {
                let action = match result {
                    Ok(paths) => ChatComposeAction::StagePicked {
                        kind,
                        paths: paths.into_iter().map(PathBuf::from).collect(),
                    },
                    Err(err) => ChatComposeAction::PickerFailed(err.to_string()),
                };
                ctx.dispatch_typed_action(&action);
            },
            config,
        );
    }

    fn stage_picked_paths(
        &mut self,
        kind: AttachKind,
        paths: Vec<PathBuf>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.picking = false;
        if let Some(conv_id) = self.selection.lock().ok().and_then(|g| g.clone()) {
            self.bind_staged_conv(conv_id);
            self.clear_opening_status();
        } else if self.staged_conv_id.is_none() {
            let pending = self
                .shell_state
                .lock()
                .map(|state| state.pending_open.is_some())
                .unwrap_or(false);
            if !pending {
                let _ = self.require_selected_conversation(ctx);
                return;
            }
        }
        let mut files = Vec::new();
        for path in paths {
            match prepare_staged_file(path, kind) {
                Ok(file) => files.push(file),
                Err(err) => {
                    self.status = err.message();
                    self.status_tone = StatusTone::Danger;
                }
            }
        }
        self.push_staged_files(files, ctx);
    }

    fn paste_clipboard(&mut self, ctx: &mut ViewContext<Self>) {
        let content = ctx.clipboard().read();
        let mut paths = paths_from_clipboard_content(&content);
        if paths.is_empty() {
            paths = read_clipboard_existing_paths();
        }
        if !paths.is_empty() {
            self.stage_dropped_paths(
                paths
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
                ctx,
            );
            return;
        }
        if let Some(images) = content.images.filter(|images| !images.is_empty()) {
            let Some(conv_id) = self.require_selected_conversation(ctx) else {
                return;
            };
            self.bind_staged_conv(conv_id);
            for image in images {
                let (kind, ext) = clipboard_image_kind_and_ext(&image.mime_type);
                let name = image
                    .filename
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| wormhole_i18n::t("chat.attachment.clipboard_image"));
                self.stage_clipboard_bytes(image.data, name, kind, ext, ctx);
            }
            return;
        }
        if let Some((png, name)) = read_clipboard_image_png() {
            let Some(conv_id) = self.require_selected_conversation(ctx) else {
                return;
            };
            self.bind_staged_conv(conv_id);
            self.stage_clipboard_bytes(png, name, "image", "png", ctx);
            return;
        }
        let text = {
            let trimmed = content.plain_text.trim();
            if trimmed.is_empty() {
                read_clipboard_text()
            } else {
                Some(trimmed.to_string())
            }
        };
        if let Some(text) = text {
            self.field_state
                .apply(&mut self.draft, &TextFieldEditAction::Paste(text));
            self.input_focused = true;
            sync_caret_blink(self, ctx);
            ctx.notify();
        }
    }

    fn stage_clipboard_bytes(
        &mut self,
        bytes: Vec<u8>,
        name: String,
        kind: &str,
        ext: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        match self.write_clipboard_staging_file(&bytes, ext) {
            Ok(path) => match prepare_staged_file_with_kind(path.clone(), kind.into()) {
                Ok(file) => {
                    let id = uuid::Uuid::new_v4().to_string();
                    let preview_path = file.path.clone();
                    let preview_kind = file.kind.clone();
                    self.staged.push(StagedAttachment {
                        id: id.clone(),
                        path: file.path,
                        kind: file.kind,
                        name,
                        size: file.size,
                        preview_asset_id: None,
                        delete_on_clear: true,
                    });
                    self.input_focused = true;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                    if preview_kind == "image" {
                        self.queue_preview_decode(id, preview_path, ctx);
                    }
                }
                Err(err) => {
                    let _ = std::fs::remove_file(&path);
                    self.status = err.message();
                    self.status_tone = StatusTone::Danger;
                    ctx.notify();
                }
            },
            Err(err) => {
                self.status = err;
                self.status_tone = StatusTone::Danger;
                ctx.notify();
            }
        }
    }

    fn write_clipboard_staging_file(&self, bytes: &[u8], ext: &str) -> Result<PathBuf, String> {
        let dir = self.core.data_dir().join("chat").join("compose-staging");
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join(format!("{}.{ext}", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).map_err(|err| err.to_string())?;
        Ok(path)
    }

    fn remove_staged(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if index < self.staged.len() {
            let item = self.staged.remove(index);
            if item.delete_on_clear {
                let _ = std::fs::remove_file(&item.path);
            }
            if self.staged.is_empty() {
                self.staged_conv_id = None;
            }
        }
        ctx.notify();
    }

    fn compose_plain_btn(
        icon_path: &'static str,
        color: ColorU,
        action: ChatComposeAction,
        automation_label: &str,
        automation_id: &str,
    ) -> Box<dyn Element> {
        let inner = EventHandler::new(
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(icons::chat_compose_icon(icon_path, color))
                    .finish(),
            )
            .with_background(ColorU::transparent_black())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                CHAT_COMPOSE_BTN / 2.0,
            )))
            .finish(),
        )
        .skip_automation()
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish();

        AutomationTarget::new(
            ConstrainedBox::new(inner)
                .with_width(CHAT_COMPOSE_BTN)
                .with_height(CHAT_COMPOSE_BTN)
                .finish(),
        )
        .with_label(automation_label)
        .with_id(automation_id)
        .finish()
    }

    fn compose_send_btn(
        icon_path: &'static str,
        color: ColorU,
        filled: bool,
        action: ChatComposeAction,
    ) -> Box<dyn Element> {
        let (bg, border_color) = if filled {
            (theme::accent_cool(), theme::accent_cool())
        } else {
            (ColorU::transparent_black(), theme::border())
        };
        let inner = EventHandler::new(
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(icons::chat_compose_icon(icon_path, color))
                    .finish(),
            )
            .with_background(bg)
            .with_border(Border::all(1.0).with_border_fill(border_color))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                CHAT_COMPOSE_BTN / 2.0,
            )))
            .finish(),
        )
        .skip_automation()
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish();

        AutomationTarget::new(
            ConstrainedBox::new(inner)
                .with_width(CHAT_COMPOSE_BTN)
                .with_height(CHAT_COMPOSE_BTN)
                .finish(),
        )
        .with_label("发送消息")
        .with_id("chat:send")
        .finish()
    }

    fn attach_kind_icon(&self, kind: AttachKind) -> Box<dyn Element> {
        let icon_tint = match kind {
            AttachKind::Media => theme::accent(),
            AttachKind::Document => theme::accent_cool(),
            AttachKind::Location => theme::success(),
        };
        let icon_bg = match kind {
            AttachKind::Media => theme::accent_bg(36),
            AttachKind::Document => theme::accent_bg(30),
            AttachKind::Location => ColorU::new(70, 120, 90, 36),
        };
        // Match compose_plain_btn: Flex Max fills the circle so the glyph is centered.
        ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(icons::icon(kind.icon_path(), ATTACH_ICON_GLYPH, icon_tint))
                    .finish(),
            )
            .with_background(icon_bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                ATTACH_ICON_SIZE / 2.0,
            )))
            .finish(),
        )
        .with_width(ATTACH_ICON_SIZE)
        .with_height(ATTACH_ICON_SIZE)
        .finish()
    }

    fn attach_panel(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for kind in [
            AttachKind::Media,
            AttachKind::Document,
            AttachKind::Location,
        ] {
            col.add_child(popover_icon_label_option(
                self.font,
                self.attach_kind_icon(kind),
                kind.label(),
                move |ctx, _, _| {
                    ctx.dispatch_typed_action(ChatComposeAction::PickAttachment(kind));
                    DispatchEventResult::StopPropagation
                },
            ));
        }
        popover_shell_with_radius(ATTACH_POPOVER_WIDTH, ATTACH_POPOVER_RADIUS, col.finish())
    }

    fn attach_button(&self) -> Box<dyn Element> {
        Self::compose_plain_btn(
            "chat-compose-attach.svg",
            theme::muted(),
            ChatComposeAction::ToggleAttachPanel,
            "附件菜单",
            "chat:attach",
        )
    }

    fn input_pill(&self, input_height: f32) -> Box<dyn Element> {
        let draft = self.draft.clone();
        let marked = self.field_state.marked_text.clone();
        let placeholder = if self.sending {
            wormhole_i18n::t("chat.compose.sending")
        } else if !self.staged.is_empty() {
            wormhole_i18n::t("chat.compose.caption")
        } else {
            wormhole_i18n::t("chat.compose.placeholder")
        };
        let draft_font = crate::ui::fonts::chat_message_font(self.font, self.emoji_font, &draft);
        let field = render_compose_field_with_caret(
            &draft,
            &marked,
            &placeholder,
            draft_font,
            self.input_focused,
            self.sending,
            self.caret_blink.visible,
            self.field_state.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ChatComposeAction::TextEdit(action));
        })
        .focused(self.input_focused)
        .disabled(self.sending)
        .ime_preedit(!marked.is_empty())
        .on_paste(|ctx| {
            ctx.dispatch_typed_action(ChatComposeAction::PasteClipboard);
            DispatchEventResult::StopPropagation
        })
        .on_keydown({
            let sending = self.sending;
            move |ctx, keystroke| {
                if sending {
                    return DispatchEventResult::PropagateToParent;
                }
                match keystroke.key.as_str() {
                    "enter" | "return" => {
                        if keystroke.shift {
                            ctx.dispatch_typed_action(ChatComposeAction::TextEdit(
                                TextFieldEditAction::InsertNewline,
                            ));
                        } else {
                            ctx.dispatch_typed_action(ChatComposeAction::Send);
                        }
                        DispatchEventResult::StopPropagation
                    }
                    "escape" => {
                        ctx.dispatch_typed_action(ChatComposeAction::ClearAndUnfocus);
                        DispatchEventResult::StopPropagation
                    }
                    "tab" => {
                        ctx.dispatch_typed_action(ChatComposeAction::ToggleFocus);
                        DispatchEventResult::StopPropagation
                    }
                    _ => DispatchEventResult::PropagateToParent,
                }
            }
        })
        .finish();

        let draft_label = if self.draft.trim().is_empty() {
            "消息输入框".to_string()
        } else {
            self.draft.clone()
        };
        let input_inner = EventHandler::new(input)
            .skip_automation()
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChatComposeAction::FocusInput);
                DispatchEventResult::StopPropagation
            })
            .finish();
        let input = AutomationTarget::new(
            ConstrainedBox::new(input_inner)
                .with_min_width(0.0)
                .with_height(input_height)
                .with_max_height(multiline_input::box_height(
                    &"x".repeat(multiline_input::DEFAULT_COLS * multiline_input::MAX_LINES),
                    multiline_input::DEFAULT_COLS,
                ))
                .finish(),
        )
        .with_label(draft_label)
        .with_id("chat:compose_input")
        .finish();

        let border_color = if self.input_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };

        Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(Expanded::new(1.0, input).finish())
                .finish(),
        )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_color(border_color))
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
        .finish()
    }

    fn reply_strip(&self, preview: &str) -> Box<dyn Element> {
        let clear = AutomationTarget::new(
            EventHandler::new(
                Container::new(
                    ui_text::chat_bubble_meta("×".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(4.0)
                .finish(),
            )
            .skip_automation()
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChatComposeAction::ClearReply);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.image.clear_reply"))
        .with_id("chat:reply_clear")
        .finish();

        AutomationTarget::new(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        Expanded::new(
                            1.0,
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
                                    ui_text::chat_bubble_meta(preview.to_string(), self.font)
                                        .with_color(theme::muted())
                                        .finish(),
                                )
                                .finish(),
                        )
                        .finish(),
                    )
                    .with_child(clear)
                    .finish(),
            )
            .with_padding_bottom(STAGED_STRIP_PAD)
            .with_padding_left(4.0)
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.image.reply"))
        .with_id("chat:reply_strip")
        .finish()
    }

    fn staged_strip(&self) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Max);
        for (index, item) in self.staged.iter().enumerate() {
            row.add_child(
                Container::new(self.staged_chip(index, item))
                    .with_margin_right(8.0)
                    .finish(),
            );
        }
        AutomationTarget::new(
            Container::new(row.finish())
                .with_padding_bottom(STAGED_STRIP_PAD)
                .finish(),
        )
        .with_label("待发送附件")
        .with_id("chat:staged_strip")
        .finish()
    }

    fn staged_chip(&self, index: usize, item: &StagedAttachment) -> Box<dyn Element> {
        let thumb = if item.kind == "image" {
            if let Some(asset_id) = &item.preview_asset_id {
                ConstrainedBox::new(
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
                .with_width(STAGED_THUMB)
                .with_height(STAGED_THUMB)
                .finish()
            } else {
                self.staged_placeholder(wormhole_i18n::t("chat.preview.image"))
            }
        } else {
            self.staged_file_chip(item)
        };
        let remove = AutomationTarget::new(
            EventHandler::new(
                Container::new(
                    ui_text::chat_bubble_meta("×".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(4.0)
                .finish(),
            )
            .skip_automation()
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatComposeAction::RemoveStaged(index));
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.compose.remove_attachment"))
        .with_id(format!("chat:staged_remove_{index}"))
        .finish();

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::End);
        col.add_child(remove);
        col.add_child(thumb);
        col.finish()
    }

    fn staged_placeholder(&self, label: String) -> Box<dyn Element> {
        ConstrainedBox::new(
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        ui_text::chat_bubble_meta(label, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .finish(),
            )
            .with_background(theme::accent_bg(24))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_width(STAGED_THUMB)
        .with_height(STAGED_THUMB)
        .finish()
    }

    fn staged_file_chip(&self, item: &StagedAttachment) -> Box<dyn Element> {
        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(
            ui_text::chat_bubble_meta(item.name.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(
            ui_text::chat_bubble_meta(format_file_size(item.size), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        ConstrainedBox::new(
            Container::new(col.finish())
                .with_uniform_padding(8.0)
                .with_background(theme::accent_bg(24))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish(),
        )
        .with_min_width(STAGED_THUMB)
        .with_max_width(160.0)
        .with_height(STAGED_THUMB)
        .finish()
    }
}

impl Entity for ChatComposeView {
    type Event = ();
}

impl View for ChatComposeView {
    fn ui_name() -> &'static str {
        "ChatComposeView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let draft_empty = self.draft.trim().is_empty();
        let has_staged = !self.staged.is_empty();
        let input_height = compose_input_height(&self.draft, &self.field_state.marked_text);

        let attach_btn = self.attach_button();

        let emoji_btn = Self::compose_plain_btn(
            "chat-compose-emoji.svg",
            theme::muted(),
            ChatComposeAction::ToggleStickerPicker,
            "贴纸选择器",
            "chat:sticker",
        );

        let send_btn = if draft_empty && !has_staged {
            Self::compose_plain_btn(
                "chat-compose-mic.svg",
                theme::accent_cool(),
                ChatComposeAction::FocusInput,
                "聚焦消息输入框",
                "chat:focus_input",
            )
        } else {
            Self::compose_send_btn(
                "chat-compose-send.svg",
                theme::canvas(),
                true,
                ChatComposeAction::Send,
            )
        };

        let bar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(attach_btn)
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(self.input_pill(input_height))
                        .with_horizontal_margin(TG_COMPOSE_GAP)
                        .finish(),
                )
                .finish(),
            )
            .with_child(emoji_btn)
            .with_child(
                Container::new(send_btn)
                    .with_margin_left(TG_COMPOSE_GAP)
                    .finish(),
            );

        let mut compose_col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if let Some(reply) = self
            .shell_state
            .lock()
            .ok()
            .and_then(|state| state.reply_draft.clone())
        {
            compose_col.add_child(self.reply_strip(&reply.preview));
        }
        if has_staged {
            compose_col.add_child(self.staged_strip());
        }
        compose_col.add_child(bar.finish());
        if !self.status.is_empty() && self.status_tone != StatusTone::Success {
            compose_col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }

        let compose_body = Container::new(compose_col.finish())
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(8.0)
            .with_padding_bottom(10.0)
            .with_background(theme::panel())
            .with_border(Border::top(1.0).with_border_fill(theme::border()))
            .finish();

        if self.sticker_open || self.attach_open {
            // Positioned overlays do not contribute to Stack size — input bar stays put.
            let mut stack = Stack::new();
            stack.add_child(compose_body);
            if self.attach_open {
                // Align to attach button (compose left).
                stack.add_positioned_overlay_child(
                    self.attach_panel(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(12.0, -8.0),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopLeft,
                        ChildAnchor::BottomLeft,
                    ),
                );
            }
            if self.sticker_open {
                // Align to emoji button (compose right), matching `.tg-compose-emoji-panel`.
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.sticker_picker).finish(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(-12.0, -8.0),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopRight,
                        ChildAnchor::BottomRight,
                    ),
                );
            }
            stack.finish()
        } else {
            compose_body
        }
    }

    fn accessibility_contents(&self, _app: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
            wormhole_i18n::t("chat.a11y.compose_label"),
            wormhole_i18n::t("chat.a11y.compose_help"),
            WarpA11yRole::TextfieldRole,
        ))
    }

    fn accessibility_data(&self, _ctx: &mut ViewContext<Self>) -> Option<AccessibilityData> {
        Some(AccessibilityData {
            content: if self.draft.is_empty() {
                wormhole_i18n::t("chat.a11y.compose_empty")
            } else {
                wormhole_i18n::t_args(
                    "chat.a11y.compose_chars",
                    &[("count", &self.draft.chars().count().to_string())],
                )
            },
        })
    }
}

impl TypedActionView for ChatComposeView {
    type Action = ChatComposeAction;

    fn handle_action(&mut self, action: &ChatComposeAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatComposeAction::Send => self.send(ctx),
            ChatComposeAction::FocusInput => {
                if !self.sending {
                    self.input_focused = true;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
            ChatComposeAction::ToggleFocus => {
                if !self.sending {
                    self.input_focused = !self.input_focused;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
            ChatComposeAction::ToggleStickerPicker => {
                self.sticker_open = !self.sticker_open;
                if self.sticker_open {
                    self.attach_open = false;
                }
                ctx.notify();
            }
            ChatComposeAction::ToggleAttachPanel => {
                self.attach_open = !self.attach_open;
                if self.attach_open {
                    self.sticker_open = false;
                }
                ctx.notify();
            }
            ChatComposeAction::PickAttachment(kind) => self.pick_attachment(*kind, ctx),
            ChatComposeAction::StagePicked { kind, paths } => {
                self.stage_picked_paths(*kind, paths.clone(), ctx)
            }
            ChatComposeAction::PickerFailed(err) => {
                self.picking = false;
                self.status = err.clone();
                self.status_tone = StatusTone::Danger;
                ctx.notify();
            }
            ChatComposeAction::RemoveStaged(index) => self.remove_staged(*index, ctx),
            ChatComposeAction::PasteClipboard => self.paste_clipboard(ctx),
            ChatComposeAction::ClearAndUnfocus => {
                if !self.sending {
                    if !self.staged.is_empty() {
                        self.clear_staged();
                    } else if self
                        .shell_state
                        .lock()
                        .map(|state| state.reply_draft.is_some())
                        .unwrap_or(false)
                    {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.clear_reply_draft();
                        }
                    } else {
                        self.draft.clear();
                        self.field_state.clear_marked();
                        self.input_focused = false;
                    }
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
            ChatComposeAction::ClearReply => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.clear_reply_draft();
                }
                ctx.notify();
            }
            ChatComposeAction::TextEdit(edit) => {
                if !self.sending {
                    self.field_state.apply(&mut self.draft, edit);
                    self.input_focused = true;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &ChatComposeAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let content = match action {
            ChatComposeAction::Send => {
                AccessibilityContent::new_without_help("发送消息", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::FocusInput | ChatComposeAction::ToggleFocus => {
                AccessibilityContent::new_without_help("聚焦消息输入框", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::ToggleStickerPicker => {
                AccessibilityContent::new_without_help("贴纸选择器", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::ToggleAttachPanel => {
                AccessibilityContent::new_without_help("附件菜单", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::PickAttachment(_)
            | ChatComposeAction::StagePicked { .. }
            | ChatComposeAction::PickerFailed(_) => {
                AccessibilityContent::new_without_help("选择附件类型", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::RemoveStaged(_) => {
                AccessibilityContent::new_without_help("移除附件", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::PasteClipboard => {
                AccessibilityContent::new_without_help("粘贴", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::ClearAndUnfocus
            | ChatComposeAction::ClearReply
            | ChatComposeAction::TextEdit(_) => {
                AccessibilityContent::new_without_help("编辑消息草稿", WarpA11yRole::TextfieldRole)
            }
        };
        ActionAccessibilityContent::Custom(content)
    }
}

impl CaretBlinkHost for ChatComposeView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.input_focused && !self.sending
    }
}
