use std::sync::{Arc, Mutex};

use warpui::elements::Fill;
use warpui::elements::{
    Border, ChildView, ClippedScrollStateHandle, ClippedScrollable, Container, Flex, MainAxisSize,
    ParentElement, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::chat::bubble::ChatBubbleView;
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{ui_title, SECTION_PADDING};
use crate::ui::theme;
use wormhole_desktop_core::chat_commands::{
    chat_list_messages, ChatMessageDto, ListChatMessagesParams,
};

pub struct ChatThreadView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    loaded_for: Option<String>,
    messages: Vec<ChatMessageDto>,
    bubbles: Vec<warpui::ViewHandle<ChatBubbleView>>,
    hint_bubble: warpui::ViewHandle<ChatBubbleView>,
    scroll: ClippedScrollStateHandle,
}

impl ChatThreadView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let hint_bubble = ctx.add_view(|ctx| {
            ChatBubbleView::system_hint(
                ctx,
                "Vault ready · endpoint 已连接 · 等待消息…".into(),
            )
        });
        let view = Self {
            core,
            selection,
            font,
            loaded_for: None,
            messages: Vec::new(),
            bubbles: Vec::new(),
            hint_bubble,
            scroll: ClippedScrollStateHandle::new(),
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            },
            |view, _, ctx| {
                view.poll_selection(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn poll_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let current = self.selection.lock().ok().and_then(|g| g.clone());
        if current == self.loaded_for {
            return;
        }
        self.loaded_for = current.clone();
        if let Some(conv_id) = current {
            let core = self.core.clone();
            ctx.spawn(
                async move {
                    let runtime = core.runtime();
                    let params = ListChatMessagesParams {
                        conv_id,
                        limit: Some(50),
                        before: None,
                    };
                    chat_list_messages(runtime.ctx.as_ref(), &runtime.state, params).await
                },
                |view, output, ctx| {
                    match output {
                        Ok(messages) => {
                            view.messages = messages;
                            view.rebuild_bubbles(ctx);
                        }
                        Err(_) => {
                            view.messages.clear();
                            view.bubbles.clear();
                        }
                    }
                    ctx.notify();
                },
            );
        } else {
            self.messages.clear();
            self.bubbles.clear();
            ctx.notify();
        }
    }

    fn rebuild_bubbles(&mut self, ctx: &mut ViewContext<Self>) {
        self.bubbles.clear();
        for msg in &self.messages {
            let body = msg.body.clone();
            let author = msg.author_endpoint.clone();
            self.bubbles
                .push(ctx.add_view(move |ctx| ChatBubbleView::new(ctx, author, body)));
        }
    }

    fn thread_title(&self) -> String {
        self.loaded_for
            .clone()
            .unwrap_or_else(|| "选择左侧终端".into())
    }
}

impl Entity for ChatThreadView {
    type Event = ();
}

impl View for ChatThreadView {
    fn ui_name() -> &'static str {
        "ChatThreadView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(ui_title(self.thread_title(), self.font));
        if self.messages.is_empty() {
            col.add_child(
                Container::new(ChildView::new(&self.hint_bubble).finish())
                    .with_margin_top(10.0)
                    .finish(),
            );
        } else {
            for bubble in &self.bubbles {
                col.add_child(
                    Container::new(ChildView::new(bubble).finish())
                        .with_vertical_margin(5.0)
                        .finish(),
                );
            }
        }

        Container::new(
            ClippedScrollable::vertical(
                self.scroll.clone(),
                col.finish(),
                ScrollbarWidth::Auto,
                Fill::None,
                Fill::None,
                Fill::None,
            )
            .finish(),
        )
        .with_uniform_padding(SECTION_PADDING)
        .with_background(theme::canvas())
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish()
    }
}
