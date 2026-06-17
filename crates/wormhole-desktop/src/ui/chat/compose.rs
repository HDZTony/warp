use std::sync::{Arc, Mutex};

use warpui::elements::{
    ChildView, Container, DispatchEventResult, EventHandler, Flex, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::sticker_picker::StickerPickerView;
use crate::ui::chat::video::ChatVideoView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{chat_send_message, SendChatMessageParams};

#[derive(Debug, Clone)]
pub enum ChatComposeAction {
    Send,
}

pub struct ChatComposeView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    draft: String,
    status: String,
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
        let sticker_picker = ctx.add_view(|ctx| StickerPickerView::new(ctx, core.clone()));
        let video = ctx.add_view(|ctx| ChatVideoView::new(ctx, core.clone(), selection.clone()));
        Self {
            core,
            selection,
            font,
            draft: String::new(),
            status: String::new(),
            sticker_picker,
            video,
        }
    }

    fn send(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selection.lock().ok().and_then(|g| g.clone()) {
            Some(id) => id,
            None => {
                self.status = "请先选择会话".into();
                ctx.notify();
                return;
            }
        };
        let body = self.draft.trim().to_string();
        if body.is_empty() {
            return;
        }
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
                match output {
                    Ok(_) => {
                        view.draft.clear();
                        view.status = "已发送".into();
                    }
                    Err(e) => view.status = format!("发送失败: {e}"),
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
        let draft = if self.draft.is_empty() {
            "（输入消息，Enter 发送）".to_string()
        } else {
            self.draft.clone()
        };
        Flex::column()
            .with_child(ChildView::new(&self.video).finish())
            .with_child(ChildView::new(&self.sticker_picker).finish())
            .with_child(
                EventHandler::new(ui_text::body(draft, self.font).finish())
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(ChatComposeAction::Send);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_child(ui_text::body(self.status.clone(), self.font).finish())
            .with_child(
                Container::new(
                    ui_text::body("点击输入区发送当前草稿（stub）", self.font).finish(),
                )
                    .with_background(theme::panel())
                    .with_uniform_padding(8.0)
                    .finish(),
            )
            .finish()
    }
}

impl TypedActionView for ChatComposeView {
    type Action = ChatComposeAction;

    fn handle_action(&mut self, action: &ChatComposeAction, ctx: &mut ViewContext<Self>) {
        if matches!(action, ChatComposeAction::Send) {
            if self.draft.is_empty() {
                self.draft = "Hello from Warp desktop".into();
            }
            self.send(ctx);
        }
    }
}
