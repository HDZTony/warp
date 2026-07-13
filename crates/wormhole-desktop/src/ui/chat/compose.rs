use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize,
    OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::attach_panel::{
    attachment_kind_for_path, pick_file_for_kind, AttachKind,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::{PendingOutgoingAttachment, SharedChatShellState};
use crate::ui::chat::sticker_picker::{StickerPickerEvent, StickerPickerView};
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
use wormhole_desktop_core::chat_commands::{
    chat_send_message, SendChatAttachmentDto, SendChatMessageParams,
};

const TG_COMPOSE_GAP: f32 = 8.0;
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
    ClearAndUnfocus,
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
        }
    }

    /// Resolve the active conversation id for send paths. Distinguishes pending open
    /// and open failures from a true "no session selected" state.
    fn require_selected_conversation(&mut self, ctx: &mut ViewContext<Self>) -> Option<String> {
        if let Some(id) = self.selection.lock().ok().and_then(|g| g.clone()) {
            return Some(id);
        }
        let (pending, open_error) = self
            .shell_state
            .lock()
            .map(|state| (state.pending_open.is_some(), state.open_error.clone()))
            .unwrap_or((false, None));
        if pending {
            self.status = "正在打开会话…".into();
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
        self.status = "请先选择会话".into();
        self.status_tone = StatusTone::Warn;
        ctx.notify();
        None
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
        self.status.clear();
        self.status_tone = StatusTone::Neutral;

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
                };
                chat_send_message(runtime.ctx.as_ref(), &runtime.state, params).await
            },
            move |view, output, ctx| {
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
                        view.draft = body_for_restore;
                        view.status = format!("发送失败: {e}");
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

    fn send_attachment(
        &mut self,
        path: std::path::PathBuf,
        kind: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(conv_id) = self.require_selected_conversation(ctx) else {
            return;
        };
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "附件".into());
        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        let path_string = path.to_string_lossy().into_owned();

        let client_id = format!("pending:{}", uuid::Uuid::new_v4());
        let sent_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);

        self.status.clear();
        self.status_tone = StatusTone::Neutral;
        self.attach_open = false;

        if let Ok(mut state) = self.shell_state.lock() {
            state.push_pending_outgoing(
                client_id.clone(),
                conv_id.clone(),
                String::new(),
                sent_at,
                vec![PendingOutgoingAttachment {
                    kind: kind.clone(),
                    name: name.clone(),
                    size,
                }],
            );
            state.bump_message_tick();
        }
        ctx.notify();

        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let client_id_for_spawn = client_id.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = SendChatMessageParams {
                    conv_id,
                    body: String::new(),
                    backend: None,
                    peer_bootstrap_addrs: Vec::new(),
                    sticker: None,
                    attachments: vec![SendChatAttachmentDto {
                        kind,
                        path: path_string,
                    }],
                };
                chat_send_message(runtime.ctx.as_ref(), &runtime.state, params).await
            },
            move |view, output, ctx| {
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
                        view.status = format!("附件发送失败: {e}");
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

    fn compose_plain_btn(
        icon_path: &'static str,
        color: ColorU,
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
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                CHAT_COMPOSE_BTN / 2.0,
            )))
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish();

        ConstrainedBox::new(inner)
            .with_width(CHAT_COMPOSE_BTN)
            .with_height(CHAT_COMPOSE_BTN)
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
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish();

        ConstrainedBox::new(inner)
            .with_width(CHAT_COMPOSE_BTN)
            .with_height(CHAT_COMPOSE_BTN)
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
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(ATTACH_ICON_SIZE / 2.0)))
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
        )
    }

    fn input_pill(&self, input_height: f32) -> Box<dyn Element> {
        let draft = self.draft.clone();
        let marked = self.field_state.marked_text.clone();
        let placeholder = if self.sending {
            "发送中…"
        } else {
            "输入消息…"
        };
        let draft_font = crate::ui::fonts::chat_message_font(self.font, self.emoji_font, &draft);
        let field = render_compose_field_with_caret(
            &draft,
            &marked,
            placeholder,
            draft_font,
            self.input_focused,
            self.sending,
            self.caret_blink.visible,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ChatComposeAction::TextEdit(action));
        })
        .focused(self.input_focused)
        .disabled(self.sending)
        .ime_preedit(!marked.is_empty())
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

        let input = ConstrainedBox::new(
            EventHandler::new(input)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(ChatComposeAction::FocusInput);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_min_width(0.0)
        .with_height(input_height)
        .with_max_height(multiline_input::box_height(
            &"x".repeat(multiline_input::DEFAULT_COLS * multiline_input::MAX_LINES),
            multiline_input::DEFAULT_COLS,
        ))
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
        let input_height = compose_input_height(&self.draft, &self.field_state.marked_text);

        let attach_btn = self.attach_button();

        let emoji_btn = Self::compose_plain_btn(
            "chat-compose-emoji.svg",
            theme::muted(),
            ChatComposeAction::ToggleStickerPicker,
        );

        let send_btn = if draft_empty {
            Self::compose_plain_btn(
                "chat-compose-mic.svg",
                theme::accent_cool(),
                ChatComposeAction::FocusInput,
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
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(bar.finish());
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
            "聊天消息输入",
            "Tab 聚焦输入框。Enter 发送，Shift+Enter 换行。Esc 清空并取消焦点。",
            WarpA11yRole::TextfieldRole,
        ))
    }

    fn accessibility_data(&self, _ctx: &mut ViewContext<Self>) -> Option<AccessibilityData> {
        Some(AccessibilityData {
            content: if self.draft.is_empty() {
                "聊天输入框，空".into()
            } else {
                format!("聊天输入框，{} 个字符", self.draft.chars().count())
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
            ChatComposeAction::PickAttachment(kind) => {
                self.attach_open = false;
                if *kind == AttachKind::Location {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast("位置附件尚未支持", StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                }
                if let Some(path) = pick_file_for_kind(*kind) {
                    let attachment_kind = attachment_kind_for_path(&path, *kind);
                    self.send_attachment(path, attachment_kind, ctx);
                } else {
                    ctx.notify();
                }
            }
            ChatComposeAction::ClearAndUnfocus => {
                if !self.sending {
                    self.draft.clear();
                    self.field_state.clear_marked();
                    self.input_focused = false;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
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
            ChatComposeAction::PickAttachment(_) => {
                AccessibilityContent::new_without_help("选择附件类型", WarpA11yRole::ButtonRole)
            }
            ChatComposeAction::ClearAndUnfocus | ChatComposeAction::TextEdit(_) => {
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
