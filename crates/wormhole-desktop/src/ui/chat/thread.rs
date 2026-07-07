use warpui::elements::Fill;
use warpui::elements::{
    Align, ChildView, ClippedScrollStateHandle, ClippedScrollable, Container, Flex, MainAxisSize,
    ParentElement, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::chat::bubble::{format_message_time_pub, outgoing_message_read, ChatBubbleView};
use crate::ui::chat::layout::{
    message_is_grouped, message_row_margin_bottom, thread_search_matches, TG_THREAD_PAD_BOTTOM,
    TG_THREAD_PAD_TOP, TG_THREAD_PAD_X,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::chat::thread_backdrop::ChatThreadBackdrop;
use crate::ui::core_handle::CoreHandle;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_messages, ChatMessageDto, ListChatMessagesParams,
};

struct BubbleEntry {
    outgoing: bool,
    body: String,
    visible: bool,
    handle: warpui::ViewHandle<ChatBubbleView>,
}

pub struct ChatThreadView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    loaded_for: Option<String>,
    last_message_tick: u64,
    last_search_query: String,
    local_endpoint: Option<String>,
    messages: Vec<ChatMessageDto>,
    bubbles: Vec<BubbleEntry>,
    hint_bubble: warpui::ViewHandle<ChatBubbleView>,
    scroll: ClippedScrollStateHandle,
}

impl ChatThreadView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let hint_bubble =
            ctx.add_view(|ctx| ChatBubbleView::system_hint(ctx, "选择左侧终端开始聊天".into()));
        let view = Self {
            core,
            selection,
            shell_state,
            font,
            loaded_for: None,
            last_message_tick: 0,
            last_search_query: String::new(),
            local_endpoint: None,
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
                view.poll(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn poll(&mut self, ctx: &mut ViewContext<Self>) {
        let current = self.selection.lock().ok().and_then(|g| g.clone());
        let message_tick = self
            .shell_state
            .lock()
            .map(|state| state.message_tick)
            .unwrap_or(0);
        let query = self.search_query();
        let search_changed = query != self.last_search_query;
        let selection_changed = current != self.loaded_for;
        let tick_changed = message_tick != self.last_message_tick;
        if search_changed && !self.messages.is_empty() {
            self.last_search_query = query;
            self.rebuild_bubbles(ctx);
            ctx.notify();
            return;
        }
        if !selection_changed && !tick_changed {
            return;
        }
        if selection_changed {
            self.loaded_for = current.clone();
            self.last_search_query = self.search_query();
        }
        self.last_message_tick = message_tick;
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
            self.rebuild_bubbles(ctx);
            ctx.notify();
        }
    }

    fn search_query(&self) -> String {
        self.shell_state
            .lock()
            .map(|state| state.thread_search_query.clone())
            .unwrap_or_default()
    }

    fn rebuild_bubbles(&mut self, ctx: &mut ViewContext<Self>) {
        self.bubbles.clear();
        let local = self.local_endpoint.as_deref();
        let query = self.search_query();
        let mut prev_outgoing: Option<bool> = None;
        let mut last_outgoing_index: Option<usize> = None;
        let mut outgoing_flags = Vec::with_capacity(self.messages.len());
        for msg in &self.messages {
            let outgoing = local
                .map(|ep| ep == msg.author_endpoint.as_str())
                .unwrap_or(false);
            if outgoing {
                last_outgoing_index = Some(outgoing_flags.len());
            }
            outgoing_flags.push(outgoing);
        }

        for (index, msg) in self.messages.iter().enumerate() {
            let outgoing = outgoing_flags[index];
            let grouped = message_is_grouped(prev_outgoing, outgoing);
            prev_outgoing = Some(outgoing);
            let body = msg.body.clone();
            let timestamp = format_message_time_pub(msg.sent_at);
            let read = outgoing
                && outgoing_message_read(
                    msg.sent_at,
                    last_outgoing_index == Some(index),
                );
            let search_hit = !query.trim().is_empty() && thread_search_matches(&query, &body);
            let visible = thread_search_matches(&query, &body);
            let handle = ctx.add_view(move |ctx| {
                ChatBubbleView::new(ctx, body, outgoing, timestamp, grouped, read, search_hit)
            });
            self.bubbles.push(BubbleEntry {
                outgoing,
                body: msg.body.clone(),
                visible,
                handle,
            });
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
        if self.messages.is_empty() {
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
                if !bubble.visible {
                    continue;
                }
                let grouped_with_next = self
                    .bubbles
                    .get(index + 1)
                    .map(|next| {
                        next.visible
                            && message_is_grouped(Some(bubble.outgoing), next.outgoing)
                    })
                    .unwrap_or(false);
                let margin_bottom = message_row_margin_bottom(grouped_with_next);
                col.add_child(
                    Container::new(ChildView::new(&bubble.handle).finish())
                        .with_margin_bottom(margin_bottom)
                        .finish(),
                );
            }
        }

        let scroll = ClippedScrollable::vertical(
            self.scroll.clone(),
            col.finish(),
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();

        ChatThreadBackdrop::new(
            Container::new(scroll)
                .with_padding_left(TG_THREAD_PAD_X)
                .with_padding_right(TG_THREAD_PAD_X)
                .with_padding_top(TG_THREAD_PAD_TOP)
                .with_padding_bottom(TG_THREAD_PAD_BOTTOM)
                .finish(),
        )
    }
}
