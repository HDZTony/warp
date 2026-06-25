use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, ChildView, Container, DispatchEventResult, EventHandler, Flex, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{
    AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext,
};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::sticker_picker::StickerPickerView;
use crate::ui::chat::video::ChatVideoView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::status_line;
use crate::ui::panel_primitives::StatusTone;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{chat_send_message, SendChatMessageParams};

#[derive(Debug, Clone)]
pub enum ChatComposeAction {
    Send,
    FocusInput,
    ToggleFocus,
    Backspace,
    InsertChar(char),
    ClearAndUnfocus,
}

pub struct ChatComposeView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    mono: FamilyId,
    draft: String,
    status: String,
    status_tone: StatusTone,
    input_focused: bool,
    sending: bool,
    sticker_picker: warpui::ViewHandle<StickerPickerView>,
    video: warpui::ViewHandle<ChatVideoView>,
}

impl ChatComposeView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let sticker_picker = ctx.add_view(|ctx| StickerPickerView::new(ctx, core.clone()));
        let video = ctx.add_view(|ctx| ChatVideoView::new(ctx, core.clone(), selection.clone()));
        Self {
            core,
            selection,
            font,
            mono,
            draft: String::new(),
            status: String::new(),
            status_tone: StatusTone::Neutral,
            input_focused: false,
            sending: false,
            sticker_picker,
            video,
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
}

impl Entity for ChatComposeView {
    type Event = ();
}

impl View for ChatComposeView {
    fn ui_name() -> &'static str {
        "ChatComposeView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let input_border = if self.input_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };

        let placeholder = if self.sending {
            "发送中…"
        } else if self.input_focused {
            "输入消息，Enter 发送，Esc 取消焦点"
        } else {
            "点击输入框或按 Tab 聚焦，Enter 发送"
        };

        let draft_empty = self.draft.is_empty();
        let input_text = if draft_empty {
            placeholder.to_string()
        } else {
            self.draft.clone()
        };

        let input_color = if self.sending || draft_empty {
            theme::placeholder()
        } else {
            theme::text()
        };

        let input_focused = self.input_focused;
        let sending = self.sending;

        let column = Flex::column()
            .with_child(ChildView::new(&self.video).finish())
            .with_child(ChildView::new(&self.sticker_picker).finish())
            .with_child(
                Container::new(
                    EventHandler::new(ui_text::mono(input_text, self.mono).with_color(input_color).finish())
                        .on_left_mouse_down(|ctx, _, _| {
                            ctx.dispatch_typed_action(ChatComposeAction::FocusInput);
                            DispatchEventResult::StopPropagation
                        })
                        .finish(),
                )
                .with_uniform_padding(12.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_color(input_border))
                .finish(),
            )
            .with_child(status_line(self.status.clone(), self.font, self.status_tone));

        EventHandler::new(column.finish())
            .with_always_handle()
            .on_keydown(move |ctx, _, keystroke| {
                if keystroke.key == "tab" {
                    ctx.dispatch_typed_action(ChatComposeAction::ToggleFocus);
                    return DispatchEventResult::StopPropagation;
                }
                if !input_focused || sending {
                    return DispatchEventResult::PropagateToParent;
                }
                if keystroke.ctrl || keystroke.meta || keystroke.alt {
                    return DispatchEventResult::PropagateToParent;
                }
                match keystroke.key.as_str() {
                    "enter" | "return" => {
                        ctx.dispatch_typed_action(ChatComposeAction::Send);
                        DispatchEventResult::StopPropagation
                    }
                    "backspace" => {
                        ctx.dispatch_typed_action(ChatComposeAction::Backspace);
                        DispatchEventResult::StopPropagation
                    }
                    "escape" => {
                        ctx.dispatch_typed_action(ChatComposeAction::ClearAndUnfocus);
                        DispatchEventResult::StopPropagation
                    }
                    key if key.len() == 1 => {
                        if let Some(ch) = key.chars().next() {
                            ctx.dispatch_typed_action(ChatComposeAction::InsertChar(ch));
                        }
                        DispatchEventResult::StopPropagation
                    }
                    _ => DispatchEventResult::PropagateToParent,
                }
            })
            .finish()
    }

    fn accessibility_contents(&self, _app: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
            "聊天消息输入",
            "Tab 聚焦输入框。Enter 发送。Esc 清空并取消焦点。",
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
                    ctx.notify();
                }
            }
            ChatComposeAction::ToggleFocus => {
                if !self.sending {
                    self.input_focused = !self.input_focused;
                    ctx.notify();
                }
            }
            ChatComposeAction::Backspace => {
                if self.input_focused && !self.sending {
                    self.draft.pop();
                    ctx.notify();
                }
            }
            ChatComposeAction::InsertChar(ch) => {
                if self.input_focused && !self.sending {
                    self.draft.push(*ch);
                    ctx.notify();
                }
            }
            ChatComposeAction::ClearAndUnfocus => {
                if self.input_focused && !self.sending {
                    self.draft.clear();
                    self.input_focused = false;
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
            ChatComposeAction::Send => AccessibilityContent::new_without_help(
                "发送消息",
                WarpA11yRole::ButtonRole,
            ),
            ChatComposeAction::FocusInput | ChatComposeAction::ToggleFocus => {
                AccessibilityContent::new_without_help(
                    "聚焦消息输入框",
                    WarpA11yRole::ButtonRole,
                )
            }
            ChatComposeAction::Backspace
            | ChatComposeAction::InsertChar(_)
            | ChatComposeAction::ClearAndUnfocus => AccessibilityContent::new_without_help(
                "编辑消息草稿",
                WarpA11yRole::TextfieldRole,
            ),
        };
        ActionAccessibilityContent::Custom(content)
    }
}
