use std::collections::HashMap;

use warpui::elements::Fill;
use warpui::elements::{
    Align, ChildView, ClippedScrollStateHandle, ClippedScrollable, Container, Expanded, Flex,
    MainAxisSize, ParentElement, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, UpdateView, View, ViewContext};

use crate::ui::chat::bubble::{format_message_time_pub, outgoing_message_read, ChatBubbleView};
use crate::ui::chat::layout::{
    bubble_max_width, message_is_grouped, message_row_margin_bottom, thread_search_matches,
    TG_THREAD_PAD_BOTTOM, TG_THREAD_PAD_TOP, TG_THREAD_PAD_X,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::{PendingOutgoingMessage, SharedChatShellState};
use crate::ui::chat::image_asset::{
    chat_wallpaper_asset_id, insert_wallpaper_asset, load_wallpaper_bytes_from_path,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{status_line, StatusTone, TG_BUBBLE_MAX_WIDTH};
use crate::ui::chat::thread_backdrop::ChatThreadBackdrop;
use wormhole_desktop_core::chat_rtc_call::video_signal_display_text;
use wormhole_desktop_core::chat_ui_prefs::load_chat_ui_prefs;
use wormhole_desktop_core::chat_wallpaper_storage::wallpaper_abs_path;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_messages, ChatAttachmentDto, ChatMessageDto, ListChatMessagesParams,
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
    last_selection_tick: u64,
    last_search_query: String,
    local_endpoint: Option<String>,
    messages: Vec<ChatMessageDto>,
    message_cache: HashMap<String, Vec<ChatMessageDto>>,
    bubbles: Vec<BubbleEntry>,
    hint_bubble: warpui::ViewHandle<ChatBubbleView>,
    loading_conv: Option<String>,
    loading_older_conv: Option<String>,
    fetch_generations: HashMap<String, u64>,
    next_fetch_seq: u64,
    history_exhausted: HashMap<String, bool>,
    fetch_error: Option<String>,
    scroll: ClippedScrollStateHandle,
    thread_content_width: f32,
    wallpaper_asset_id: Option<String>,
    wallpaper_loaded: bool,
    last_wallpaper_tick: u64,
}

fn messages_snapshot_equal(a: &[ChatMessageDto], b: &[ChatMessageDto]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).all(|(left, right)| {
        left.id == right.id && left.sent_at == right.sent_at && left.body == right.body
    })
}

const PENDING_DEDUP_WINDOW_MS: u64 = 30_000;
const RECENT_MESSAGE_PAGE: usize = 50;

/// Merge `incoming` into `existing` by message id; newer `sent_at` wins on conflict.
pub(crate) fn merge_messages_by_id(
    existing: &[ChatMessageDto],
    incoming: &[ChatMessageDto],
) -> Vec<ChatMessageDto> {
    let mut by_id: HashMap<String, ChatMessageDto> = HashMap::new();
    for message in existing {
        by_id.insert(message.id.clone(), message.clone());
    }
    for message in incoming {
        by_id
            .entry(message.id.clone())
            .and_modify(|current| {
                if message.sent_at >= current.sent_at {
                    *current = message.clone();
                }
            })
            .or_insert_with(|| message.clone());
    }
    let mut merged: Vec<_> = by_id.into_values().collect();
    merged.sort_by_key(|msg| msg.sent_at);
    merged
}

pub(crate) fn merge_pending_messages(
    messages: &[ChatMessageDto],
    pending: &[PendingOutgoingMessage],
    local_endpoint: Option<&str>,
) -> Vec<ChatMessageDto> {
    let author = local_endpoint.unwrap_or("");
    let mut merged = messages.to_vec();
    for item in pending {
        if merged.iter().any(|msg| msg.id == item.client_id) {
            continue;
        }
        if merged.iter().any(|msg| {
            msg.body == item.body && msg.sent_at.abs_diff(item.sent_at) <= PENDING_DEDUP_WINDOW_MS
        }) {
            continue;
        }
        merged.push(ChatMessageDto {
            id: item.client_id.clone(),
            author_endpoint: author.to_string(),
            body: item.body.clone(),
            sent_at: item.sent_at,
            sticker: None,
            attachments: item
                .attachments
                .iter()
                .map(|attachment| ChatAttachmentDto {
                    id: format!("pending-attach:{}", attachment.name),
                    kind: attachment.kind.clone(),
                    name: attachment.name.clone(),
                    size: attachment.size,
                    mime: None,
                    source_hash: None,
                    local_path: None,
                })
                .collect(),
        });
    }
    merged.sort_by_key(|msg| msg.sent_at);
    merged
}

fn message_is_outgoing(msg: &ChatMessageDto, local: Option<&str>) -> bool {
    msg.id.starts_with("pending:")
        || local.is_some_and(|endpoint| endpoint == msg.author_endpoint.as_str())
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
            last_selection_tick: 0,
            last_search_query: String::new(),
            local_endpoint: None,
            messages: Vec::new(),
            message_cache: HashMap::new(),
            bubbles: Vec::new(),
            hint_bubble,
            loading_conv: None,
            loading_older_conv: None,
            fetch_generations: HashMap::new(),
            next_fetch_seq: 0,
            history_exhausted: HashMap::new(),
            fetch_error: None,
            scroll: ClippedScrollStateHandle::new(),
            thread_content_width: 0.0,
            wallpaper_asset_id: None,
            wallpaper_loaded: false,
            last_wallpaper_tick: 0,
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
        let (message_tick, selection_tick, wallpaper_tick) = self
            .shell_state
            .lock()
            .map(|state| (state.message_tick, state.selection_tick, state.wallpaper_tick))
            .unwrap_or((0, 0, 0));
        let wallpaper_changed = wallpaper_tick != self.last_wallpaper_tick;
        if wallpaper_changed {
            self.last_wallpaper_tick = wallpaper_tick;
            if let Some(conv_id) = current.as_ref() {
                self.refresh_wallpaper(conv_id, ctx);
            }
        }
        let query = self.search_query();
        let search_changed = query != self.last_search_query;
        let selection_changed = current != self.loaded_for;
        let tick_changed = message_tick != self.last_message_tick;
        let pending_changed = selection_tick != self.last_selection_tick;
        if search_changed && !self.messages.is_empty() {
            self.last_search_query = query;
            self.rebuild_bubbles(ctx);
            ctx.notify();
            return;
        }
        if pending_changed {
            self.last_selection_tick = selection_tick;
            self.update_hint_bubble(ctx);
            ctx.notify();
        }
        if !selection_changed && !tick_changed {
            return;
        }
        if selection_changed {
            self.loaded_for = current.clone();
            if let Ok(mut state) = self.shell_state.lock() {
                state.close_thread_search();
            }
            self.last_search_query = String::new();
            self.fetch_error = None;
            if let Some(conv_id) = current.as_ref() {
                let cached = self.message_cache.get(conv_id).cloned();
                let has_cache = cached.as_ref().is_some_and(|messages| !messages.is_empty());
                self.loading_conv = if has_cache {
                    None
                } else {
                    Some(conv_id.clone())
                };
                self.loading_older_conv = None;
                self.history_exhausted.remove(conv_id);
                let base = cached.unwrap_or_default();
                let merged = self.merge_messages_for_conv(&base, conv_id);
                if !messages_snapshot_equal(&self.messages, &merged) {
                    self.messages = merged;
                    self.rebuild_bubbles(ctx);
                }
                self.update_hint_bubble(ctx);
                self.refresh_wallpaper(conv_id, ctx);
            } else {
                self.messages.clear();
                self.bubbles.clear();
                self.wallpaper_asset_id = None;
                self.wallpaper_loaded = false;
                self.loading_conv = None;
                self.loading_older_conv = None;
                self.update_hint_bubble(ctx);
            }
            ctx.notify();
        }
        self.last_message_tick = message_tick;
        if let Some(conv_id) = current {
            if tick_changed {
                self.sync_messages_for_conv(&conv_id, ctx);
                ctx.notify();
            }
            self.fetch_messages(conv_id, ctx);
        } else if tick_changed {
            self.rebuild_bubbles(ctx);
            ctx.notify();
        }
    }

    fn pending_for_conv(&self, conv_id: &str) -> Vec<PendingOutgoingMessage> {
        self.shell_state
            .lock()
            .map(|state| state.pending_for_conv(conv_id))
            .unwrap_or_default()
    }

    fn merge_messages_for_conv(
        &self,
        base: &[ChatMessageDto],
        conv_id: &str,
    ) -> Vec<ChatMessageDto> {
        merge_pending_messages(
            base,
            &self.pending_for_conv(conv_id),
            self.local_endpoint.as_deref(),
        )
    }

    fn sync_messages_for_conv(&mut self, conv_id: &str, ctx: &mut ViewContext<Self>) {
        let base = self
            .message_cache
            .get(conv_id)
            .cloned()
            .unwrap_or_else(|| self.messages.clone());
        let merged = self.merge_messages_for_conv(&base, conv_id);
        if messages_snapshot_equal(&self.messages, &merged) {
            return;
        }
        self.messages = merged.clone();
        self.message_cache.insert(conv_id.to_string(), base);
        self.rebuild_bubbles(ctx);
    }

    fn next_fetch_generation(&mut self, conv_id: &str) -> u64 {
        self.next_fetch_seq = self.next_fetch_seq.saturating_add(1);
        self.fetch_generations
            .insert(conv_id.to_string(), self.next_fetch_seq);
        self.next_fetch_seq
    }

    fn fetch_generation_matches(&self, conv_id: &str, generation: u64) -> bool {
        self.fetch_generations
            .get(conv_id)
            .copied()
            .is_some_and(|current| current == generation)
    }

    fn apply_fetched_messages(
        &mut self,
        conv_id: &str,
        generation: u64,
        fetched: Vec<ChatMessageDto>,
        before: Option<u64>,
        ctx: &mut ViewContext<Self>,
    ) {
        if !self.fetch_generation_matches(conv_id, generation) {
            return;
        }
        if before.is_none() {
            self.loading_conv = None;
        } else {
            self.loading_older_conv = None;
        }
        self.fetch_error = None;

        let existing = self.message_cache.get(conv_id).cloned().unwrap_or_default();
        if fetched.is_empty() {
            if before.is_some() {
                self.history_exhausted.insert(conv_id.to_string(), true);
            } else if existing.is_empty() && self.messages.is_empty() {
                self.messages.clear();
                self.message_cache.insert(conv_id.to_string(), Vec::new());
                self.rebuild_bubbles(ctx);
            }
            self.update_hint_bubble(ctx);
            ctx.notify();
            return;
        }

        let merged_base = merge_messages_by_id(&existing, &fetched);
        let merged = merge_pending_messages(
            &merged_base,
            &self.pending_for_conv(conv_id),
            self.local_endpoint.as_deref(),
        );
        self.message_cache
            .insert(conv_id.to_string(), merged_base.clone());

        if before.is_some() && fetched.len() < RECENT_MESSAGE_PAGE {
            self.history_exhausted.insert(conv_id.to_string(), true);
        }

        if self.loaded_for.as_deref() == Some(conv_id) {
            if !messages_snapshot_equal(&self.messages, &merged) {
                self.messages = merged;
                self.rebuild_bubbles(ctx);
            }
            self.update_hint_bubble(ctx);
            ctx.notify();
        }

        if before.is_none()
            && fetched.len() >= RECENT_MESSAGE_PAGE
            && !self
                .history_exhausted
                .get(conv_id)
                .copied()
                .unwrap_or(false)
            && self.loading_older_conv.is_none()
        {
            let earliest = merged_base.first().map(|msg| msg.sent_at);
            if let Some(sent_at) = earliest {
                self.fetch_older_messages(conv_id.to_string(), Some(sent_at), ctx);
            }
        }
    }

    pub fn apply_incoming_message(
        &mut self,
        message: ChatMessageDto,
        conv_id: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.loaded_for.as_deref() != Some(conv_id) {
            return;
        }
        let base = self
            .message_cache
            .get(conv_id)
            .cloned()
            .unwrap_or_else(|| self.messages.clone());
        if base.iter().any(|existing| existing.id == message.id) {
            return;
        }
        let merged_base = merge_messages_by_id(&base, std::slice::from_ref(&message));
        let merged = merge_pending_messages(
            &merged_base,
            &self.pending_for_conv(conv_id),
            self.local_endpoint.as_deref(),
        );
        if messages_snapshot_equal(&self.messages, &merged) {
            return;
        }
        self.messages = merged.clone();
        self.message_cache.insert(conv_id.to_string(), merged_base);
        self.rebuild_bubbles(ctx);
        self.update_hint_bubble(ctx);
    }

    pub fn force_sync_messages(&mut self, conv_id: &str, ctx: &mut ViewContext<Self>) {
        if self.loaded_for.as_deref() != Some(conv_id) {
            return;
        }
        self.fetch_messages(conv_id.to_string(), ctx);
    }

    fn hint_text(&self) -> String {
        let pending_open = self
            .shell_state
            .lock()
            .map(|state| state.pending_open.is_some())
            .unwrap_or(false);
        if pending_open {
            return "正在打开会话…".into();
        }
        if self.loaded_for.is_none() {
            return "选择左侧终端开始聊天".into();
        }
        if self.fetch_error.is_some() {
            return self.fetch_error.clone().unwrap_or_default();
        }
        if self.loading_conv.is_some() {
            return "加载中…".into();
        }
        if self.loading_older_conv.is_some() {
            return "加载更早消息…".into();
        }
        let query = self.search_query();
        if !query.trim().is_empty()
            && !self.messages.is_empty()
            && self.bubbles.iter().all(|bubble| !bubble.visible)
        {
            return "无匹配消息".into();
        }
        "暂无消息，发送第一条吧".into()
    }

    fn update_hint_bubble(&self, ctx: &mut ViewContext<Self>) {
        let text = self.hint_text();
        ctx.update_view(&self.hint_bubble, |bubble, ctx| {
            bubble.set_body(text);
            ctx.notify();
        });
    }

    fn fetch_messages(&mut self, conv_id: String, ctx: &mut ViewContext<Self>) {
        self.fetch_error = None;
        if self
            .message_cache
            .get(&conv_id)
            .is_none_or(|messages| messages.is_empty())
        {
            self.loading_conv = Some(conv_id.clone());
            self.update_hint_bubble(ctx);
        }
        let generation = self.next_fetch_generation(&conv_id);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let cfg = chat_config(app, &state).await;
                let params = ListChatMessagesParams {
                    conv_id: conv_id.clone(),
                    limit: Some(RECENT_MESSAGE_PAGE as u32),
                    before: None,
                };
                let messages =
                    chat_list_messages(runtime.ctx.as_ref(), &runtime.state, params).await;
                (conv_id, generation, cfg, messages, None)
            },
            |view, output, ctx| {
                let (conv_id, generation, cfg, messages, before) = output;
                if let Ok(c) = cfg {
                    view.local_endpoint = Some(c.endpoint_id);
                }
                match messages {
                    Ok(messages) => {
                        view.apply_fetched_messages(&conv_id, generation, messages, before, ctx);
                    }
                    Err(err) if view.fetch_generation_matches(&conv_id, generation) => {
                        view.loading_conv = None;
                        view.loading_older_conv = None;
                        if view.loaded_for.as_deref() == Some(conv_id.as_str()) {
                            if view.messages.is_empty() {
                                view.fetch_error = Some(format!("无法加载消息: {err}"));
                                view.bubbles.clear();
                            }
                            view.update_hint_bubble(ctx);
                            ctx.notify();
                        }
                    }
                    Err(_) => {
                        view.loading_conv = None;
                        view.loading_older_conv = None;
                    }
                }
            },
        );
    }

    fn fetch_older_messages(
        &mut self,
        conv_id: String,
        before: Option<u64>,
        ctx: &mut ViewContext<Self>,
    ) {
        if self
            .history_exhausted
            .get(&conv_id)
            .copied()
            .unwrap_or(false)
        {
            return;
        }
        if self.loading_older_conv.as_deref() == Some(conv_id.as_str()) {
            return;
        }
        self.loading_older_conv = Some(conv_id.clone());
        self.update_hint_bubble(ctx);
        let generation = self.next_fetch_generation(&conv_id);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let params = ListChatMessagesParams {
                    conv_id: conv_id.clone(),
                    limit: Some(RECENT_MESSAGE_PAGE as u32),
                    before,
                };
                let messages =
                    chat_list_messages(runtime.ctx.as_ref(), &runtime.state, params).await;
                (conv_id, generation, messages, before)
            },
            |view, output, ctx| {
                let (conv_id, generation, messages, before) = output;
                match messages {
                    Ok(messages) => {
                        view.apply_fetched_messages(&conv_id, generation, messages, before, ctx);
                    }
                    Err(err) if view.fetch_generation_matches(&conv_id, generation) => {
                        view.loading_older_conv = None;
                        if view.loaded_for.as_deref() == Some(conv_id.as_str()) {
                            view.fetch_error = Some(format!("无法加载更早消息: {err}"));
                            view.update_hint_bubble(ctx);
                            ctx.notify();
                        }
                    }
                    Err(_) => {
                        view.loading_older_conv = None;
                    }
                }
            },
        );
    }

    fn refresh_wallpaper(&mut self, conv_id: &str, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let conv_id = conv_id.to_string();
        ctx.spawn(
            async move {
                let data_dir = core.data_dir();
                let prefs = load_chat_ui_prefs(&data_dir).await.unwrap_or_default();
                let rel = prefs.wallpaper_path(&conv_id).map(str::to_string);
                Ok::<_, String>((conv_id, rel, data_dir))
            },
            |view, output, ctx| {
                let Ok((conv_id, rel, data_dir)) = output else {
                    return;
                };
                if view.loaded_for.as_deref() != Some(conv_id.as_str()) {
                    return;
                }
                let Some(rel) = rel else {
                    view.wallpaper_asset_id = None;
                    view.wallpaper_loaded = false;
                    ctx.notify();
                    return;
                };
                let asset_id = chat_wallpaper_asset_id(&conv_id);
                view.wallpaper_asset_id = Some(asset_id);
                let abs = wallpaper_abs_path(&data_dir, &rel);
                match load_wallpaper_bytes_from_path(&abs).and_then(|bytes| {
                    insert_wallpaper_asset(ctx, &conv_id, bytes).map(|_| ())
                }) {
                    Ok(()) => {
                        view.wallpaper_loaded = true;
                        ctx.notify();
                    }
                    Err(_) => {
                        view.wallpaper_loaded = false;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn search_query(&self) -> String {
        self.shell_state
            .lock()
            .map(|state| state.thread_search_query.clone())
            .unwrap_or_default()
    }

    fn bubble_width_cap(&self) -> f32 {
        if self.thread_content_width.is_finite() && self.thread_content_width > 0.0 {
            bubble_max_width(self.thread_content_width)
        } else {
            TG_BUBBLE_MAX_WIDTH
        }
    }

    fn rebuild_bubbles(&mut self, ctx: &mut ViewContext<Self>) {
        self.bubbles.clear();
        let max_bubble_width = self.bubble_width_cap();
        let local = self.local_endpoint.as_deref();
        let query = self.search_query();
        let mut prev_outgoing: Option<bool> = None;
        let mut last_outgoing_index: Option<usize> = None;
        let mut outgoing_flags = Vec::with_capacity(self.messages.len());
        for msg in &self.messages {
            let outgoing = message_is_outgoing(msg, local);
            if outgoing {
                last_outgoing_index = Some(outgoing_flags.len());
            }
            outgoing_flags.push(outgoing);
        }

        for (index, msg) in self.messages.iter().enumerate() {
            let outgoing = outgoing_flags[index];
            let grouped = message_is_grouped(prev_outgoing, outgoing);
            prev_outgoing = Some(outgoing);
            let body = video_signal_display_text(&msg.body);
            let attachments = msg.attachments.clone();
            let timestamp = format_message_time_pub(msg.sent_at);
            let read = outgoing
                && !msg.id.starts_with("pending:")
                && outgoing_message_read(msg.sent_at, last_outgoing_index == Some(index));
            let search_hit = !query.trim().is_empty()
                && (thread_search_matches(&query, &body)
                    || msg
                        .attachments
                        .iter()
                        .any(|attachment| thread_search_matches(&query, &attachment.name)));
            let visible = search_hit
                || query.trim().is_empty()
                || thread_search_matches(&query, &body)
                || msg
                    .attachments
                    .iter()
                    .any(|attachment| thread_search_matches(&query, &attachment.name));
            let handle = ctx.add_view(move |ctx| {
                ChatBubbleView::new(
                    ctx,
                    body,
                    attachments,
                    outgoing,
                    timestamp,
                    grouped,
                    read,
                    search_hit,
                    max_bubble_width,
                )
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
                        next.visible && message_is_grouped(Some(bubble.outgoing), next.outgoing)
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

        let mut body = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(scroll)
                        .with_padding_left(TG_THREAD_PAD_X)
                        .with_padding_right(TG_THREAD_PAD_X)
                        .with_padding_top(TG_THREAD_PAD_TOP)
                        .with_padding_bottom(TG_THREAD_PAD_BOTTOM)
                        .finish(),
                )
                .finish(),
            );
        if let Some(error) = self
            .fetch_error
            .as_ref()
            .filter(|_| self.messages.is_empty())
        {
            body.add_child(status_line(error.clone(), self.font, StatusTone::Danger));
        }

        let finished = body.finish();
        if let Some(asset_id) = self.wallpaper_asset_id.as_ref() {
            ChatThreadBackdrop::with_wallpaper(finished, asset_id.clone(), self.wallpaper_loaded)
        } else {
            ChatThreadBackdrop::new(finished)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message(id: &str, body: &str, sent_at: u64) -> ChatMessageDto {
        ChatMessageDto {
            id: id.into(),
            author_endpoint: "local".into(),
            body: body.into(),
            sent_at,
            sticker: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn messages_snapshot_equal_compares_id_sent_at_and_body() {
        let a = sample_message("m1", "hello", 1_731_637_500_000);
        let mut b = a.clone();
        assert!(messages_snapshot_equal(
            std::slice::from_ref(&a),
            std::slice::from_ref(&b)
        ));
        b.body = "other".into();
        assert!(!messages_snapshot_equal(
            std::slice::from_ref(&a),
            std::slice::from_ref(&b)
        ));
    }

    #[test]
    fn merge_pending_messages_appends_pending_bubble() {
        let base = vec![sample_message("m1", "hello", 1_000)];
        let pending = vec![PendingOutgoingMessage {
            client_id: "pending:1".into(),
            conv_id: "conv".into(),
            body: "optimistic".into(),
            sent_at: 2_000,
            attachments: Vec::new(),
        }];
        let merged = merge_pending_messages(&base, &pending, Some("local"));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].id, "pending:1");
        assert_eq!(merged[1].body, "optimistic");
    }

    #[test]
    fn merge_pending_messages_dedupes_confirmed_server_message() {
        let base = vec![sample_message("m2", "same text", 5_000)];
        let pending = vec![PendingOutgoingMessage {
            client_id: "pending:2".into(),
            conv_id: "conv".into(),
            body: "same text".into(),
            sent_at: 5_100,
            attachments: Vec::new(),
        }];
        let merged = merge_pending_messages(&base, &pending, Some("local"));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "m2");
    }

    #[test]
    fn merge_messages_by_id_dedupes_and_sorts() {
        let existing = vec![
            sample_message("a", "one", 10),
            sample_message("b", "two", 20),
        ];
        let incoming = vec![
            sample_message("b", "two-updated", 25),
            sample_message("c", "three", 30),
        ];
        let merged = merge_messages_by_id(&existing, &incoming);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].id, "a");
        assert_eq!(merged[1].id, "b");
        assert_eq!(merged[1].body, "two-updated");
        assert_eq!(merged[2].id, "c");
    }

    #[test]
    fn merge_messages_by_id_keeps_existing_when_incoming_empty() {
        let existing = vec![sample_message("a", "one", 10)];
        let merged = merge_messages_by_id(&existing, &[]);
        assert_eq!(merged.len(), existing.len());
        assert_eq!(merged[0].id, existing[0].id);
    }
}
