use std::path::PathBuf;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    AutomationTarget, Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment,
    MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius,
    Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::attach_panel::{
    picker_config_for_kind, prepare_dropped_file, prepare_staged_file, prepare_staged_file_with_kind,
    sim_use_attach_paths_from_env, AttachKind, PreparedStagedFile,
};
use crate::ui::chat::media_upload_modal::open_media_upload_with_files;
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::chat::sticker_picker::{StickerPickerEvent, StickerPickerView};
use crate::ui::clipboard::{
    clipboard_image_kind_and_ext, paths_from_clipboard_content, read_clipboard_existing_paths,
    read_clipboard_image_png, read_clipboard_text,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons::{self, CHAT_COMPOSE_BTN, CHAT_COMPOSE_BTN_H};
use crate::ui::multiline_input;
use crate::ui::panel_primitives::{
    popover_icon_label_option, popover_shell_with_radius, status_line, StatusTone,
};
use crate::ui::text_field_input::{
    render_compose_field_with_caret, sync_caret_blink, CaretBlink, CaretBlinkHost,
    TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{chat_send_message, SendChatMessageParams};

const TG_COMPOSE_GAP: f32 = 4.0;
const ATTACH_POPOVER_WIDTH: f32 = 196.0;
const ATTACH_POPOVER_RADIUS: f32 = 12.0;
const ATTACH_ICON_SIZE: f32 = 36.0;
const ATTACH_ICON_GLYPH: f32 = 18.0;

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
    PasteClipboard,
    ClearAndUnfocus,
    ClearReply,
    TextEdit(TextFieldEditAction),
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
            picking: false,
        }
    }

    pub fn focus_input(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.sending {
            self.input_focused = true;
            sync_caret_blink(self, ctx);
            ctx.notify();
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
        ctx.notify();
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
        self.open_upload_modal(Some(conv_id), files, Vec::new(), false, ctx);
    }

    fn open_upload_modal(
        &mut self,
        conv_id: Option<String>,
        files: Vec<PreparedStagedFile>,
        delete_flags: Vec<bool>,
        force_as_file: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if files.is_empty() {
            ctx.notify();
            return;
        }
        self.status.clear();
        self.status_tone = StatusTone::Neutral;
        self.attach_open = false;
        open_media_upload_with_files(
            &self.shell_state,
            &self.core.data_dir(),
            conv_id,
            files,
            delete_flags,
            force_as_file,
        );
        ctx.notify();
    }

    fn send(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(conv_id) = self.require_selected_conversation(ctx) else {
            return;
        };
        let body = self.draft.trim().to_string();
        if body.is_empty() {
            return;
        }

        let client_id = format!("pending:{}", uuid::Uuid::new_v4());
        let sent_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);

        self.draft.clear();
        self.field_state.clear_marked();
        self.sending = true;
        self.status = wormhole_i18n::t("chat.compose.sending");
        self.status_tone = StatusTone::Neutral;

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
                Vec::new(),
            );
            state.bump_message_tick();
        }
        ctx.notify();

        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let client_id_for_spawn = client_id.clone();
        let body_for_restore = body.clone();
        let conv_id_for_restore = conv_id.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = SendChatMessageParams {
                    conv_id,
                    body,
                    backend: None,
                    peer_bootstrap_addrs: Vec::new(),
                    sticker: None,
                    attachments: Vec::new(),
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
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.bump_message_tick();
                        }
                    }
                    Err(e) => {
                        let current = view.selection.lock().ok().and_then(|g| g.clone());
                        if current.as_ref() == Some(&conv_id_for_restore) {
                            view.draft = body_for_restore;
                        }
                        view.status =
                            wormhole_i18n::t_args("chat.send_failed", &[("err", &e.to_string())]);
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
        if let Some(_conv_id) = conv_id {
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
        // sim-use / CI: skip OS file dialog when absolute path(s) are provided.
        let sim_paths = sim_use_attach_paths_from_env();
        if !sim_paths.is_empty() {
            self.stage_picked_paths(kind, sim_paths, ctx);
            return;
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
        let conv_id = self.selection.lock().ok().and_then(|g| g.clone());
        if conv_id.is_some() {
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
        self.open_upload_modal(
            conv_id,
            files,
            Vec::new(),
            kind == AttachKind::Document,
            ctx,
        );
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
            let mut files = Vec::new();
            let mut delete_flags = Vec::new();
            for image in images {
                let (kind, ext) = clipboard_image_kind_and_ext(&image.mime_type);
                let name = image
                    .filename
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| wormhole_i18n::t("chat.attachment.clipboard_image"));
                match self.write_clipboard_staging_file(&image.data, ext) {
                    Ok(path) => match prepare_staged_file_with_kind(path.clone(), kind.into()) {
                        Ok(mut file) => {
                            file.name = name;
                            files.push(file);
                            delete_flags.push(true);
                        }
                        Err(err) => {
                            let _ = std::fs::remove_file(&path);
                            self.status = err.message();
                            self.status_tone = StatusTone::Danger;
                        }
                    },
                    Err(err) => {
                        self.status = err;
                        self.status_tone = StatusTone::Danger;
                    }
                }
            }
            self.open_upload_modal(Some(conv_id), files, delete_flags, false, ctx);
            return;
        }
        if let Some((png, name)) = read_clipboard_image_png() {
            let Some(conv_id) = self.require_selected_conversation(ctx) else {
                return;
            };
            match self.write_clipboard_staging_file(&png, "png") {
                Ok(path) => match prepare_staged_file_with_kind(path.clone(), "image".into()) {
                    Ok(mut file) => {
                        file.name = name;
                        self.open_upload_modal(Some(conv_id), vec![file], vec![true], false, ctx);
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

    fn write_clipboard_staging_file(&self, bytes: &[u8], ext: &str) -> Result<PathBuf, String> {
        let dir = self.core.data_dir().join("chat").join("compose-staging");
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join(format!("{}.{ext}", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).map_err(|err| err.to_string())?;
        Ok(path)
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
                .with_height(CHAT_COMPOSE_BTN_H)
                .finish(),
        )
        .with_label(automation_label)
        .with_id(automation_id)
        .finish()
    }

    fn compose_send_btn(
        icon_path: &'static str,
        color: ColorU,
        _filled: bool,
        action: ChatComposeAction,
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
                .with_height(CHAT_COMPOSE_BTN_H)
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
            let automation_id = match kind {
                AttachKind::Media => "chat:attach_media",
                AttachKind::Document => "chat:attach_document",
                AttachKind::Location => "chat:attach_location",
            };
            col.add_child(
                AutomationTarget::new(popover_icon_label_option(
                    self.font,
                    self.attach_kind_icon(kind),
                    kind.label(),
                    move |ctx, _, _| {
                        ctx.dispatch_typed_action(ChatComposeAction::PickAttachment(kind));
                        DispatchEventResult::StopPropagation
                    },
                ))
                .with_label(kind.label())
                .with_id(automation_id)
                .finish(),
            );
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

    fn input_pill(&self) -> Box<dyn Element> {
        let draft = self.draft.clone();
        let marked = self.field_state.marked_text.clone();
        let placeholder = if self.sending {
            wormhole_i18n::t("chat.compose.sending")
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
        // Height follows soft-wrapped Text (not a fixed cols estimate). Cap like HTML textarea.
        let input = AutomationTarget::new(
            ConstrainedBox::new(input_inner)
                .with_min_width(0.0)
                .with_min_height(multiline_input::COMPOSE_BASE_HEIGHT)
                .with_max_height(multiline_input::COMPOSE_MAX_HEIGHT)
                .finish(),
        )
        .with_label(draft_label)
        .with_id("chat:compose_input")
        .finish();

        let border = if self.input_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };
        // Bordered field (6px, not 22/999 pill). Outer bar must wrap with Expanded so
        // the stroke stretches with the flex row and does not shrink around the caret.
        Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(Expanded::new(1.0, input).finish())
                .finish(),
        )
        .with_padding_left(6.0)
        .with_padding_right(6.0)
        .with_padding_top(8.0)
        .with_padding_bottom(8.0)
        .with_background(ColorU::transparent_black())
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
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
            .with_padding_bottom(8.0)
            .with_padding_left(4.0)
            .finish(),
        )
        .with_label(wormhole_i18n::t("chat.image.reply"))
        .with_id("chat:reply_strip")
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

        let attach_btn = self.attach_button();

        let emoji_btn = Self::compose_plain_btn(
            "chat-compose-emoji.svg",
            theme::muted(),
            ChatComposeAction::ToggleStickerPicker,
            "贴纸选择器",
            "chat:sticker",
        );

        let send_btn = if draft_empty {
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
                theme::accent_cool(),
                false,
                ChatComposeAction::Send,
            )
        };

        let bar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(attach_btn)
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(self.input_pill())
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
        compose_col.add_child(bar.finish());
        if !self.status.is_empty() && self.status_tone != StatusTone::Success {
            compose_col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }

        let compose_body = Container::new(compose_col.finish())
            .with_padding_left(8.0)
            .with_padding_right(8.0)
            .with_padding_top(9.0)
            .with_padding_bottom(9.0)
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
            ChatComposeAction::FocusInput => self.focus_input(ctx),
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
            ChatComposeAction::PasteClipboard => self.paste_clipboard(ctx),
            ChatComposeAction::ClearAndUnfocus => {
                if !self.sending {
                    if self
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
