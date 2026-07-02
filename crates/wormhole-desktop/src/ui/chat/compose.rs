use pathfinder_color::ColorU;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Align, Border, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::sticker_picker::StickerPickerView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons::{self, CHAT_COMPOSE_BTN};
use crate::ui::multiline_input;
use crate::ui::panel_primitives::{status_line, StatusTone};
use crate::ui::text_field_input::{
    compose_input_height, render_field_with_caret, sync_caret_blink, CaretBlink, CaretBlinkHost,
    TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use wormhole_desktop_core::chat_commands::{chat_send_message, SendChatMessageParams};

const TG_COMPOSE_PILL_RADIUS: f32 = 22.0;
const TG_COMPOSE_GAP: f32 = 6.0;

#[derive(Debug, Clone)]
pub enum ChatComposeAction {
    Send,
    FocusInput,
    ToggleFocus,
    ToggleStickerPicker,
    ClearAndUnfocus,
    TextEdit(TextFieldEditAction),
}

pub struct ChatComposeView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    draft: String,
    field_state: TextFieldState,
    status: String,
    status_tone: StatusTone,
    input_focused: bool,
    caret_blink: CaretBlink,
    sending: bool,
    sticker_open: bool,
    sticker_picker: warpui::ViewHandle<StickerPickerView>,
}

impl ChatComposeView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let sticker_picker = ctx.add_view(|ctx| StickerPickerView::new(ctx, core.clone()));
        Self {
            core,
            selection,
            font,
            draft: String::new(),
            field_state: TextFieldState::new(),
            status: String::new(),
            status_tone: StatusTone::Neutral,
            input_focused: false,
            caret_blink: CaretBlink::new(),
            sending: false,
            sticker_open: false,
            sticker_picker,
        }
    }

    fn send(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selection.lock().ok().and_then(|g| g.clone()) {
            Some(id) => id,
            None => {
                self.status = "请先选择会话".into();
                self.status_tone = StatusTone::Warn;
                ctx.notify();
                return;
            }
        };
        let body = self.draft.trim().to_string();
        if body.is_empty() {
            return;
        }
        self.sending = true;
        self.status = "发送中…".into();
        self.status_tone = StatusTone::Muted;
        let core = self.core.clone();
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
            |view, output, ctx| {
                view.sending = false;
                match output {
                    Ok(_) => {
                        view.draft.clear();
                        view.field_state.clear_marked();
                        view.status = "已发送".into();
                        view.status_tone = StatusTone::Success;
                    }
                    Err(e) => {
                        view.status = format!("发送失败: {e}");
                        view.status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn plain_compose_btn(
        icon_path: &'static str,
        color: ColorU,
        action: ChatComposeAction,
    ) -> Box<dyn Element> {
        Container::new(
            EventHandler::new(Align::new(icons::chat_compose_icon(icon_path, color)).finish())
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
            CHAT_COMPOSE_BTN / 2.0,
        )))
        .finish()
    }

    fn send_compose_btn(has_text: bool) -> Box<dyn Element> {
        let (icon_path, bg, fg, action) = if has_text {
            (
                "chat-compose-send.svg",
                theme::accent_cool(),
                theme::canvas(),
                ChatComposeAction::Send,
            )
        } else {
            (
                "chat-compose-mic.svg",
                ColorU::transparent_black(),
                theme::accent_cool(),
                ChatComposeAction::FocusInput,
            )
        };
        let mut btn = Container::new(
            EventHandler::new(Align::new(icons::chat_compose_icon(icon_path, fg)).finish())
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
            CHAT_COMPOSE_BTN / 2.0,
        )));
        if has_text {
            btn = btn.with_background(bg);
        }
        ConstrainedBox::new(btn.finish())
            .with_width(CHAT_COMPOSE_BTN)
            .with_height(CHAT_COMPOSE_BTN)
            .finish()
    }

    fn input_pill(&self, input_height: f32) -> Box<dyn Element> {
        let draft = self.draft.clone();
        let marked = self.field_state.marked_text.clone();
        let placeholder = if self.sending {
            "发送中…"
        } else {
            "输入消息…"
        };
        let field = render_field_with_caret(
            &draft,
            &marked,
            placeholder,
            self.font,
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

        Container::new(
            ConstrainedBox::new(
                EventHandler::new(input)
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(ChatComposeAction::FocusInput);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_height(input_height)
            .with_max_height(multiline_input::box_height(
                &"x".repeat(multiline_input::DEFAULT_COLS * multiline_input::MAX_LINES),
                multiline_input::DEFAULT_COLS,
            ))
            .finish(),
        )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(8.0)
        .with_padding_bottom(8.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(if self.input_focused {
            theme::accent_cool()
        } else {
            theme::border()
        }))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
            TG_COMPOSE_PILL_RADIUS,
        )))
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

        let attach_btn = ConstrainedBox::new(Self::plain_compose_btn(
            "chat-compose-attach.svg",
            theme::muted(),
            ChatComposeAction::FocusInput,
        ))
        .with_width(CHAT_COMPOSE_BTN)
        .with_height(CHAT_COMPOSE_BTN)
        .finish();

        let emoji_btn = ConstrainedBox::new(Self::plain_compose_btn(
            "chat-compose-emoji.svg",
            theme::muted(),
            ChatComposeAction::ToggleStickerPicker,
        ))
        .with_width(CHAT_COMPOSE_BTN)
        .with_height(CHAT_COMPOSE_BTN)
        .finish();

        let bar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::End)
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
            .with_child(Self::send_compose_btn(!draft_empty));

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

        if self.sticker_open {
            let mut stack = Stack::new();
            stack.add_child(compose_body);
            stack.add_child(
                Align::new(
                    Container::new(ChildView::new(&self.sticker_picker).finish())
                        .with_uniform_padding(8.0)
                        .with_background(theme::panel_elevated())
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
                        .with_margin_left(12.0)
                        .with_margin_bottom(56.0)
                        .finish(),
                )
                .bottom_left()
                .finish(),
            );
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
                ctx.notify();
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
