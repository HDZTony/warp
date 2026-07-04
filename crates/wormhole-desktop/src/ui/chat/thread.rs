use std::sync::Arc;

use warpui::elements::Fill;
use warpui::elements::{
    Align, ChildView, ClippedScrollStateHandle, ClippedScrollable, Container, Flex, MainAxisSize,
    ParentElement, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::chat::bubble::{format_message_time_pub, ChatBubbleView};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_messages, ChatMessageDto, ListChatMessagesParams,
};

pub struct ChatThreadView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    loaded_for: Option<String>,
    local_endpoint: Option<String>,
    messages: Vec<ChatMessageDto>,
    bubbles: Vec<warpui::ViewHandle<ChatBubbleView>>,
    system_bubbles: Vec<warpui::ViewHandle<ChatBubbleView>>,
    search_filter: String,
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
        let hint_bubble =
            ctx.add_view(|ctx| ChatBubbleView::system_hint(ctx, "选择左侧终端开始聊天".into()));
        let view = Self {
            core,
            selection,
            font,
            loaded_for: None,
            local_endpoint: None,
            messages: Vec::new(),
            bubbles: Vec::new(),
            system_bubbles: Vec::new(),
            search_filter: String::new(),
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
                    let app = runtime.ctx.as_ref();
                    let state = runtime.state.clone();
                    let cfg = chat_config(app, &state).await;
                    let params = ListChatMessagesParams {
                        conv_id: conv_id.clone(),
                        limit: Some(50),
                        before: None,
                    };
                    let messages =
                        chat_list_messages(runtime.ctx.as_ref(), &runtime.state, params).await;
                    (cfg, messages)
                },
                |view, output, ctx| {
                    let (cfg, messages) = output;
                    if let Ok(c) = cfg {
                        view.local_endpoint = Some(c.endpoint_id);
                    }
                    match messages {
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

    pub fn set_search_filter(&mut self, query: String, ctx: &mut ViewContext<Self>) {
        if self.search_filter == query {
            return;
        }
        self.search_filter = query;
        ctx.notify();
    }

    pub fn clear_local_messages(&mut self, ctx: &mut ViewContext<Self>) {
        self.messages.clear();
        self.bubbles.clear();
        self.system_bubbles.clear();
        ctx.notify();
    }

    pub fn push_system_message(&mut self, body: String, ctx: &mut ViewContext<Self>) {
        self.system_bubbles
            .push(ctx.add_view(move |ctx| ChatBubbleView::system_hint(ctx, body)));
        ctx.notify();
    }

    fn message_visible(&self, index: usize) -> bool {
        let filter = self.search_filter.trim();
        if filter.is_empty() {
            return true;
        }
        self.messages
            .get(index)
            .map(|msg| msg.body.to_ascii_lowercase().contains(&filter.to_ascii_lowercase()))
            .unwrap_or(false)
    }

    fn rebuild_bubbles(&mut self, ctx: &mut ViewContext<Self>) {
        self.bubbles.clear();
        let local = self.local_endpoint.as_deref();
        for msg in &self.messages {
            let body = msg.body.clone();
            let outgoing = local
                .map(|ep| ep == msg.author_endpoint.as_str())
                .unwrap_or(false);
            let timestamp = format_message_time_pub(msg.sent_at);
            self.bubbles
                .push(ctx.add_view(move |ctx| ChatBubbleView::new(ctx, body, outgoing, timestamp)));
        }
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
        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        for bubble in &self.system_bubbles {
            col.add_child(
                Container::new(
                    Align::new(ChildView::new(bubble).finish())
                        .top_center()
                        .finish(),
                )
                .with_vertical_margin(4.0)
                .finish(),
            );
        }
        if self.messages.is_empty() && self.system_bubbles.is_empty() {
            col.add_child(
                Container::new(
                    Align::new(ChildView::new(&self.hint_bubble).finish())
                        .top_center()
                        .finish(),
                )
                .with_vertical_margin(48.0)
                .finish(),
            );
        } else {
            for (index, bubble) in self.bubbles.iter().enumerate() {
                if !self.message_visible(index) {
                    continue;
                }
                col.add_child(
                    Container::new(ChildView::new(bubble).finish())
                        .with_horizontal_padding(16.0)
                        .with_vertical_margin(4.0)
                        .finish(),
                );
            }
            if !self.search_filter.trim().is_empty()
                && self
                    .bubbles
                    .iter()
                    .enumerate()
                    .all(|(i, _)| !self.message_visible(i))
            {
                col.add_child(
                    Container::new(
                        Align::new(
                            ui_text::body("无匹配消息".to_string(), self.font)
                                .with_color(crate::ui::theme::muted())
                                .finish(),
                        )
                        .top_center()
                        .finish(),
                    )
                    .with_vertical_margin(24.0)
                    .finish(),
                );
            }
        }

        ClippedScrollable::vertical(
            self.scroll.clone(),
            col.finish(),
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish()
    }
}
