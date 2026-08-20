use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::Fill;
use warpui::elements::{
    Align, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, DispatchEventResult, EventHandler, Expanded, Flex, MainAxisSize, ParentElement,
    Radius, SavePosition, ScrollTarget, ScrollToPositionMode, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext};

use crate::ui::chat::bubble::{format_message_time_pub, outgoing_message_read, ChatBubbleView};
use crate::ui::chat::image_asset::{
    chat_wallpaper_asset_id, decode_image_asset_from_path, insert_attachment_image_payload,
    insert_wallpaper_asset, load_wallpaper_bytes_from_path,
};
use crate::ui::chat::layout::{
    bubble_max_width, format_date_divider_label, message_is_grouped, message_local_day_key,
    message_row_margin_bottom, should_insert_date_divider, TG_THREAD_PAD_BOTTOM, TG_THREAD_PAD_TOP,
    TG_THREAD_PAD_X,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::{PendingOutgoingMessage, SharedChatShellState};
use crate::ui::chat::thread_backdrop::ChatThreadBackdrop;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{chat_date_bg, status_line, StatusTone, TG_BUBBLE_MAX_WIDTH};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_messages, chat_message_window, chat_search_messages, ChatAttachmentDto,
    ChatMessageDto, ChatMessageWindowParams, ChatSearchCursorDto, ListChatMessagesParams,
    SearchChatMessagesParams,
};
use wormhole_desktop_core::chat_rtc_call::video_signal_display_text;
use wormhole_desktop_core::video_chat_commands::{
    parse_video_room_signal, video_chat_join, VideoChatJoinParams,
};
use crate::ui::chat::calls_panel::open_video_viewer_url;
use crate::ui::chat::voice_call_ui;
use wormhole_desktop_core::chat_ui_prefs::load_chat_ui_prefs;
use wormhole_desktop_core::chat_wallpaper_storage::wallpaper_abs_path;

struct BubbleEntry {
    outgoing: bool,
    body: String,
    visible: bool,
    handle: warpui::ViewHandle<ChatBubbleView>,
    video_room_id: Option<String>,
}

pub struct ChatThreadView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    loaded_for: Option<String>,
    last_message_tick: u64,
    last_selection_tick: u64,
    last_search_request_tick: u64,
    last_search_nav_tick: u64,
    search_hits: Vec<ChatMessageDto>,
    search_next_cursor: Option<ChatSearchCursorDto>,
    search_fetch_generation: u64,
    local_endpoint: Option<String>,
    messages: Vec<ChatMessageDto>,
    message_cache: HashMap<String, Vec<ChatMessageDto>>,
    bubbles: Vec<BubbleEntry>,
    hint_bubble: warpui::ViewHandle<ChatBubbleView>,
    loading_conv: Option<String>,
    loading_older_conv: Option<String>,
    fetch_generations: HashMap<String, u64>,
    next_fetch_seq: u64,
    history_before: HashMap<String, u64>,
    history_exhausted: HashMap<String, bool>,
    fetch_error: Option<String>,
    scroll: ClippedScrollStateHandle,
    thread_content_width: f32,
    wallpaper_asset_id: Option<String>,
    wallpaper_loaded: bool,
    last_wallpaper_tick: u64,
    last_history_cleared_tick: u64,
    attachment_previews: HashMap<String, String>,
    attachment_preview_sizes: HashMap<String, (u32, u32)>,
    attachment_decode_inflight: HashSet<String>,
    attachment_decode_errors: HashMap<String, String>,
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

#[derive(Debug, Clone)]
pub enum ChatThreadAction {
    LoadEarlier,
    JoinVideoRoom { room_id: String },
}

fn advance_history_before(current: Option<u64>, fetched: &[ChatMessageDto]) -> Option<u64> {
    fetched
        .iter()
        .map(|message| message.sent_at)
        .min()
        .map(|earliest| current.map_or(earliest, |cursor| cursor.min(earliest)))
}

fn page_exhausts_history(fetched_len: usize) -> bool {
    fetched_len < RECENT_MESSAGE_PAGE
}

fn message_position_id(message_id: &str) -> String {
    format!("chat-message-{message_id}")
}

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
    // When a confirmed message matches a pending one but still lacks local_path
    // (race before media-cache enrich), copy paths from pending by attachment name.
    for item in pending {
        if let Some(msg) = merged
            .iter_mut()
            .find(|msg| pending_is_represented_by(msg, item))
        {
            fill_missing_attachment_paths(msg, item);
        }
    }
    for item in pending {
        if merged
            .iter()
            .any(|msg| pending_is_represented_by(msg, item))
        {
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
                    local_path: attachment.local_path.clone(),
                })
                .collect(),
            source_kind: None,
            reply_to: None,
            forwarded_from: None,
        });
    }
    merged.sort_by_key(|msg| msg.sent_at);
    merged
}

fn fill_missing_attachment_paths(msg: &mut ChatMessageDto, item: &PendingOutgoingMessage) {
    for attachment in &mut msg.attachments {
        if attachment.local_path.is_some() {
            continue;
        }
        if let Some(pending) = item
            .attachments
            .iter()
            .find(|pending| pending.name == attachment.name)
        {
            attachment.local_path = pending.local_path.clone();
        }
    }
}

/// Remap optimistic `pending-attach:{name}` preview assets onto confirmed attachment ids.
pub(crate) fn remap_pending_attachment_previews(
    previews: &mut HashMap<String, String>,
    sizes: &mut HashMap<String, (u32, u32)>,
    messages: &[ChatMessageDto],
    pending: &[PendingOutgoingMessage],
) {
    for item in pending {
        let Some(msg) = messages.iter().find(|msg| pending_is_represented_by(msg, item)) else {
            continue;
        };
        for attachment in &msg.attachments {
            let pending_id = format!("pending-attach:{}", attachment.name);
            if let Some(asset_id) = previews.remove(&pending_id) {
                previews.insert(attachment.id.clone(), asset_id);
            }
            if let Some(size) = sizes.remove(&pending_id) {
                sizes.insert(attachment.id.clone(), size);
            }
        }
    }
}

fn pending_is_represented_by(msg: &ChatMessageDto, item: &PendingOutgoingMessage) -> bool {
    if msg.id == item.client_id {
        return true;
    }
    if msg.body != item.body {
        return false;
    }
    if msg.sent_at.abs_diff(item.sent_at) > PENDING_DEDUP_WINDOW_MS {
        return false;
    }
    attachment_names(msg) == pending_attachment_names(item)
}

fn attachment_names(msg: &ChatMessageDto) -> Vec<&str> {
    msg.attachments
        .iter()
        .map(|attachment| attachment.name.as_str())
        .collect()
}

fn pending_attachment_names(item: &PendingOutgoingMessage) -> Vec<&str> {
    item.attachments
        .iter()
        .map(|attachment| attachment.name.as_str())
        .collect()
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
            ctx.add_typed_action_view(|ctx| ChatBubbleView::system_hint(ctx, "选择左侧终端开始聊天".into()));
        let view = Self {
            core,
            selection,
            shell_state,
            font,
            loaded_for: None,
            last_message_tick: 0,
            last_selection_tick: 0,
            last_search_request_tick: 0,
            last_search_nav_tick: 0,
            search_hits: Vec::new(),
            search_next_cursor: None,
            search_fetch_generation: 0,
            local_endpoint: None,
            messages: Vec::new(),
            message_cache: HashMap::new(),
            bubbles: Vec::new(),
            hint_bubble,
            loading_conv: None,
            loading_older_conv: None,
            fetch_generations: HashMap::new(),
            next_fetch_seq: 0,
            history_before: HashMap::new(),
            history_exhausted: HashMap::new(),
            fetch_error: None,
            scroll: ClippedScrollStateHandle::new(),
            thread_content_width: 0.0,
            wallpaper_asset_id: None,
            wallpaper_loaded: false,
            last_wallpaper_tick: 0,
            last_history_cleared_tick: 0,
            attachment_previews: HashMap::new(),
            attachment_preview_sizes: HashMap::new(),
            attachment_decode_inflight: HashSet::new(),
            attachment_decode_errors: HashMap::new(),
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
        let (
            message_tick,
            selection_tick,
            wallpaper_tick,
            search_request_tick,
            search_nav_tick,
            history_cleared_tick,
            history_cleared_conv,
        ) = self
            .shell_state
            .lock()
            .map(|state| {
                (
                    state.message_tick,
                    state.selection_tick,
                    state.wallpaper_tick,
                    state.thread_search_request_tick,
                    state.thread_search_nav_tick,
                    state.history_cleared_tick,
                    state.history_cleared_conv.clone(),
                )
            })
            .unwrap_or((0, 0, 0, 0, 0, 0, None));
        if history_cleared_tick != self.last_history_cleared_tick {
            self.last_history_cleared_tick = history_cleared_tick;
            if let Some(conv_id) = history_cleared_conv.as_ref() {
                self.message_cache.insert(conv_id.clone(), Vec::new());
                if self.loaded_for.as_deref() == Some(conv_id.as_str()) {
                    self.messages.clear();
                    self.rebuild_bubbles(ctx);
                }
            }
        }
        let wallpaper_changed = wallpaper_tick != self.last_wallpaper_tick;
        if wallpaper_changed {
            self.last_wallpaper_tick = wallpaper_tick;
            if let Some(conv_id) = current.as_ref() {
                self.refresh_wallpaper(conv_id, ctx);
            }
        }
        let selection_changed = current != self.loaded_for;
        let tick_changed = message_tick != self.last_message_tick;
        let pending_changed = selection_tick != self.last_selection_tick;
        if search_request_tick != self.last_search_request_tick {
            self.last_search_request_tick = search_request_tick;
            self.start_search(ctx);
        }
        if search_nav_tick != self.last_search_nav_tick {
            self.last_search_nav_tick = search_nav_tick;
            self.navigate_search(ctx);
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
            self.search_hits.clear();
            self.search_next_cursor = None;
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

    pub(crate) fn selection_changed(&mut self, ctx: &mut ViewContext<Self>) {
        self.poll(ctx);
    }

    fn start_search(&mut self, ctx: &mut ViewContext<Self>) {
        self.fetch_search_page(true, None, 0, ctx);
    }

    fn fetch_search_page(
        &mut self,
        reset: bool,
        cursor: Option<ChatSearchCursorDto>,
        desired_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(conv_id) = self.loaded_for.clone() else {
            return;
        };
        let (query, from_ms, to_ms) = self
            .shell_state
            .lock()
            .map(|state| {
                (
                    state.thread_search_query.clone(),
                    state.thread_search_from_ms,
                    state.thread_search_to_ms,
                )
            })
            .unwrap_or_default();
        if query.trim().is_empty() && from_ms.is_none() && to_ms.is_none() {
            self.search_hits.clear();
            self.search_next_cursor = None;
            if let Ok(mut state) = self.shell_state.lock() {
                state.thread_search_current = 0;
                state.thread_search_total = 0;
                state.thread_search_loading = false;
                state.thread_search_error = None;
            }
            self.rebuild_bubbles(ctx);
            ctx.notify();
            return;
        }
        self.search_fetch_generation = self.search_fetch_generation.saturating_add(1);
        let generation = self.search_fetch_generation;
        if let Ok(mut state) = self.shell_state.lock() {
            state.thread_search_loading = true;
            state.thread_search_error = None;
        }
        let core = self.core.clone();
        let ascending = query.trim().is_empty();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = SearchChatMessagesParams {
                    conv_id: conv_id.clone(),
                    query: (!query.trim().is_empty()).then_some(query),
                    from_ms,
                    to_ms,
                    ascending: Some(ascending),
                    cursor,
                    limit: Some(50),
                };
                let page = chat_search_messages(runtime.ctx.as_ref(), &runtime.state, params).await;
                (conv_id, generation, reset, desired_index, page)
            },
            |view, output, ctx| {
                let (conv_id, generation, reset, desired_index, page) = output;
                if generation != view.search_fetch_generation
                    || view.loaded_for.as_deref() != Some(conv_id.as_str())
                {
                    return;
                }
                match page {
                    Ok(page) => {
                        if reset {
                            view.search_hits = page.messages;
                        } else {
                            for message in page.messages {
                                if !view.search_hits.iter().any(|hit| hit.id == message.id) {
                                    view.search_hits.push(message);
                                }
                            }
                        }
                        view.search_next_cursor = page.next_cursor;
                        let current = desired_index.min(page.total.saturating_sub(1) as usize);
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.thread_search_current = current;
                            state.thread_search_total = page.total as usize;
                            state.thread_search_loading = false;
                            state.thread_search_error = None;
                        }
                        view.rebuild_bubbles(ctx);
                        view.ensure_search_target(current, ctx);
                    }
                    Err(error) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.thread_search_loading = false;
                            state.thread_search_error = Some(error);
                            state.thread_search_current = 0;
                            state.thread_search_total = 0;
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn navigate_search(&mut self, ctx: &mut ViewContext<Self>) {
        let (current, total, delta) = self
            .shell_state
            .lock()
            .map(|state| {
                (
                    state.thread_search_current,
                    state.thread_search_total,
                    state.thread_search_nav_delta,
                )
            })
            .unwrap_or((0, 0, 0));
        if total == 0 || delta == 0 {
            return;
        }
        let desired = if delta < 0 {
            current.saturating_sub(1)
        } else {
            current.saturating_add(1).min(total.saturating_sub(1))
        };
        if desired >= self.search_hits.len() {
            if let Some(cursor) = self.search_next_cursor.clone() {
                self.fetch_search_page(false, Some(cursor), desired, ctx);
            }
            return;
        }
        if let Ok(mut state) = self.shell_state.lock() {
            state.thread_search_current = desired;
        }
        self.rebuild_bubbles(ctx);
        self.ensure_search_target(desired, ctx);
        ctx.notify();
    }

    fn ensure_search_target(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let Some(hit) = self.search_hits.get(index).cloned() else {
            return;
        };
        if self.messages.iter().any(|message| message.id == hit.id) {
            self.scroll.scroll_to_position(ScrollTarget {
                position_id: message_position_id(&hit.id),
                mode: ScrollToPositionMode::FullyIntoView,
            });
            return;
        }
        let Some(conv_id) = self.loaded_for.clone() else {
            return;
        };
        let target_id = hit.id.clone();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = ChatMessageWindowParams {
                    conv_id: conv_id.clone(),
                    anchor_message_id: target_id.clone(),
                    before: Some(25),
                    after: Some(25),
                };
                let result =
                    chat_message_window(runtime.ctx.as_ref(), &runtime.state, params).await;
                (conv_id, target_id, result)
            },
            |view, output, ctx| {
                let (conv_id, target_id, result) = output;
                if view.loaded_for.as_deref() != Some(conv_id.as_str()) {
                    return;
                }
                let active_id = view.shell_state.lock().ok().and_then(|state| {
                    view.search_hits
                        .get(state.thread_search_current)
                        .map(|hit| hit.id.clone())
                });
                if active_id.as_deref() != Some(target_id.as_str()) {
                    return;
                }
                if let Ok(window) = result {
                    let base = view
                        .message_cache
                        .get(&conv_id)
                        .cloned()
                        .unwrap_or_default();
                    let merged = merge_messages_by_id(&base, &window);
                    view.message_cache.insert(conv_id.clone(), merged.clone());
                    view.messages = view.merge_messages_for_conv(&merged, &conv_id);
                    view.rebuild_bubbles(ctx);
                    view.scroll.scroll_to_position(ScrollTarget {
                        position_id: message_position_id(&target_id),
                        mode: ScrollToPositionMode::FullyIntoView,
                    });
                    ctx.notify();
                }
            },
        );
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
        let pending = self.pending_for_conv(conv_id);
        let merged = merge_pending_messages(&base, &pending, self.local_endpoint.as_deref());
        remap_pending_attachment_previews(
            &mut self.attachment_previews,
            &mut self.attachment_preview_sizes,
            &merged,
            &pending,
        );
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
        let pending = self.pending_for_conv(conv_id);
        let merged = merge_pending_messages(
            &merged_base,
            &pending,
            self.local_endpoint.as_deref(),
        );
        remap_pending_attachment_previews(
            &mut self.attachment_previews,
            &mut self.attachment_preview_sizes,
            &merged,
            &pending,
        );
        self.message_cache
            .insert(conv_id.to_string(), merged_base.clone());

        if let Some(cursor) =
            advance_history_before(self.history_before.get(conv_id).copied(), &fetched)
        {
            self.history_before.insert(conv_id.to_string(), cursor);
        }

        if page_exhausts_history(fetched.len()) {
            self.history_exhausted.insert(conv_id.to_string(), true);
        } else if before.is_none() {
            self.history_exhausted.remove(conv_id);
        }

        if self.loaded_for.as_deref() == Some(conv_id) {
            if !messages_snapshot_equal(&self.messages, &merged) {
                self.messages = merged;
                self.rebuild_bubbles(ctx);
            }
            self.update_hint_bubble(ctx);
            self.try_pending_jump(ctx);
            ctx.notify();
        }
    }

    fn try_pending_jump(&mut self, ctx: &mut ViewContext<Self>) {
        let target = self
            .shell_state
            .lock()
            .ok()
            .and_then(|mut state| state.take_pending_jump_message());
        let Some(target_id) = target else {
            return;
        };
        if self.messages.iter().any(|message| message.id == target_id) {
            self.scroll.scroll_to_position(ScrollTarget {
                position_id: message_position_id(&target_id),
                mode: ScrollToPositionMode::FullyIntoView,
            });
            return;
        }
        let Some(conv_id) = self.loaded_for.clone() else {
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = ChatMessageWindowParams {
                    conv_id: conv_id.clone(),
                    anchor_message_id: target_id.clone(),
                    before: Some(25),
                    after: Some(25),
                };
                let result =
                    chat_message_window(runtime.ctx.as_ref(), &runtime.state, params).await;
                (conv_id, target_id, result)
            },
            |view, output, ctx| {
                let (conv_id, target_id, result) = output;
                if view.loaded_for.as_deref() != Some(conv_id.as_str()) {
                    return;
                }
                if let Ok(window) = result {
                    let base = view
                        .message_cache
                        .get(&conv_id)
                        .cloned()
                        .unwrap_or_default();
                    let merged = merge_messages_by_id(&base, &window);
                    view.message_cache.insert(conv_id.clone(), merged.clone());
                    view.messages = view.merge_messages_for_conv(&merged, &conv_id);
                    view.rebuild_bubbles(ctx);
                    view.scroll.scroll_to_position(ScrollTarget {
                        position_id: message_position_id(&target_id),
                        mode: ScrollToPositionMode::FullyIntoView,
                    });
                    ctx.notify();
                }
            },
        );
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
        let pending = self.pending_for_conv(conv_id);
        let merged = merge_pending_messages(
            &merged_base,
            &pending,
            self.local_endpoint.as_deref(),
        );
        remap_pending_attachment_previews(
            &mut self.attachment_previews,
            &mut self.attachment_preview_sizes,
            &merged,
            &pending,
        );
        self.message_cache
            .insert(conv_id.to_string(), merged_base);
        if !messages_snapshot_equal(&self.messages, &merged) {
            self.messages = merged;
            self.rebuild_bubbles(ctx);
        }
        self.update_hint_bubble(ctx);
        ctx.notify();
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
        let search_empty = self
            .shell_state
            .lock()
            .map(|state| {
                state.thread_search_open
                    && !state.thread_search_loading
                    && state.thread_search_total == 0
                    && (!state.thread_search_query.trim().is_empty()
                        || state.thread_search_from_ms.is_some())
            })
            .unwrap_or(false);
        if search_empty {
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
                match load_wallpaper_bytes_from_path(&abs)
                    .and_then(|bytes| insert_wallpaper_asset(ctx, &conv_id, bytes).map(|_| ()))
                {
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

    fn bubble_width_cap(&self) -> f32 {
        if self.thread_content_width.is_finite() && self.thread_content_width > 0.0 {
            bubble_max_width(self.thread_content_width)
        } else {
            TG_BUBBLE_MAX_WIDTH
        }
    }

    fn rebuild_bubbles(&mut self, ctx: &mut ViewContext<Self>) {
        self.queue_attachment_previews(ctx);
        self.bubbles.clear();
        let max_bubble_width = self.bubble_width_cap();
        let local = self.local_endpoint.as_deref();
        let active_search_id = self.shell_state.lock().ok().and_then(|state| {
            self.search_hits
                .get(state.thread_search_current)
                .map(|message| message.id.clone())
        });
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
            let search_hit = active_search_id.as_deref() == Some(msg.id.as_str());
            let visible = true;
            let mut image_assets = HashMap::new();
            let mut image_sizes = HashMap::new();
            for attachment in &attachments {
                if let Some(asset_id) = self.attachment_previews.get(&attachment.id) {
                    image_assets.insert(attachment.id.clone(), asset_id.clone());
                }
                if let Some(size) = self.attachment_preview_sizes.get(&attachment.id) {
                    image_sizes.insert(attachment.id.clone(), *size);
                }
            }
            let reply_preview = msg.reply_to.as_ref().and_then(|reply_id| {
                self.messages
                    .iter()
                    .find(|candidate| candidate.id == *reply_id)
                    .map(|candidate| {
                        if candidate.attachments.iter().any(|a| a.kind == "image")
                            && candidate.body.trim().is_empty()
                        {
                            wormhole_i18n::t("chat.preview.image")
                        } else if candidate.body.trim().is_empty() {
                            wormhole_i18n::t("chat.preview.file")
                        } else {
                            truncate_preview(&candidate.body, 80)
                        }
                    })
            });
            let message_id = msg.id.clone();
            let forwarded_from = msg.forwarded_from.clone();
            let shell_state = self.shell_state.clone();
            let handle = ctx.add_typed_action_view(move |ctx| {
                ChatBubbleView::new(
                    ctx,
                    message_id,
                    body,
                    attachments,
                    outgoing,
                    timestamp,
                    grouped,
                    read,
                    search_hit,
                    max_bubble_width,
                    image_assets,
                    image_sizes,
                    reply_preview,
                    forwarded_from,
                    shell_state,
                )
            });
            let video_room_id = parse_video_room_signal(&msg.body).map(|p| p.room_id);
            self.bubbles.push(BubbleEntry {
                outgoing,
                body: msg.body.clone(),
                visible,
                video_room_id,
                handle,
            });
        }
    }

    fn queue_attachment_previews(&mut self, ctx: &mut ViewContext<Self>) {
        let mut pending = Vec::new();
        for msg in &self.messages {
            for attachment in &msg.attachments {
                if attachment.kind != "image" {
                    continue;
                }
                let Some(local_path) = attachment.local_path.clone() else {
                    continue;
                };
                if self.attachment_previews.contains_key(&attachment.id) {
                    continue;
                }
                if self.attachment_decode_errors.contains_key(&attachment.id) {
                    continue;
                }
                if !self.attachment_decode_inflight.insert(attachment.id.clone()) {
                    continue;
                }
                pending.push((attachment.id.clone(), local_path));
            }
        }
        for (id, local_path) in pending {
            let path = PathBuf::from(local_path);
            ctx.spawn(
                async move {
                    match tokio::task::spawn_blocking(move || decode_image_asset_from_path(&path))
                        .await
                    {
                        Ok(Ok(decoded)) => Ok(decoded),
                        Ok(Err(err)) => Err(err),
                        Err(err) => Err(err.to_string()),
                    }
                },
                move |view, result, ctx| {
                    view.attachment_decode_inflight.remove(&id);
                    match result {
                        Ok(decoded) => {
                            view.attachment_decode_errors.remove(&id);
                            view.attachment_preview_sizes
                                .insert(id.clone(), (decoded.width, decoded.height));
                            let asset_id =
                                insert_attachment_image_payload(ctx, &id, decoded.payload);
                            view.attachment_previews.insert(id, asset_id);
                            view.rebuild_bubbles(ctx);
                        }
                        Err(err) => {
                            view.attachment_decode_errors.insert(id, err);
                            view.rebuild_bubbles(ctx);
                        }
                    }
                    ctx.notify();
                },
            );
        }
    }
}

fn truncate_preview(body: &str, max_chars: usize) -> String {
    let trimmed = body.trim().replace('\n', " ");
    let count = trimmed.chars().count();
    if count <= max_chars {
        return trimmed;
    }
    let truncated: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
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
            // Telegram-style centered service capsule (not a top-aligned system bubble).
            col.add_child(
                Container::new(
                    Align::new(ChildView::new(&self.hint_bubble).finish())
                        .finish(),
                )
                .with_vertical_margin(120.0)
                .finish(),
            );
        } else {
            let history_exhausted = self
                .loaded_for
                .as_ref()
                .and_then(|conv_id| self.history_exhausted.get(conv_id))
                .copied()
                .unwrap_or(false);
            let has_history_cursor = self
                .loaded_for
                .as_ref()
                .is_some_and(|conv_id| self.history_before.contains_key(conv_id));
            if has_history_cursor && !history_exhausted {
                let loading = self.loading_older_conv.is_some();
                let label = if loading {
                    "正在加载更早消息…"
                } else {
                    "加载更早消息"
                };
                let color = if loading {
                    theme::muted()
                } else {
                    theme::accent_cool()
                };
                let load_earlier = EventHandler::new(
                    ConstrainedBox::new(
                        Align::new(ui_text::body(label, self.font).with_color(color).finish())
                            .finish(),
                    )
                    .with_min_height(32.0)
                    .finish(),
                )
                .with_automation_label(label)
                .with_automation_id("chat:load_earlier")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(ChatThreadAction::LoadEarlier);
                    DispatchEventResult::StopPropagation
                })
                .finish();
                col.add_child(
                    Container::new(Align::new(load_earlier).top_center().finish())
                        .with_margin_bottom(8.0)
                        .finish(),
                );
            }
            let mut prev_day: Option<String> = None;
            for (index, bubble) in self.bubbles.iter().enumerate() {
                if !bubble.visible {
                    continue;
                }
                let day_key = self
                    .messages
                    .get(index)
                    .and_then(|msg| message_local_day_key(msg.sent_at));
                if should_insert_date_divider(prev_day.as_deref(), day_key.as_deref()) {
                    if let Some(ref key) = day_key {
                        let label = format_date_divider_label(key);
                        col.add_child(
                            Container::new(
                                Align::new(
                                    Container::new(
                                        ui_text::chat_bubble_meta(label, self.font)
                                            .with_color(theme::text())
                                            .finish(),
                                    )
                                    .with_horizontal_padding(12.0)
                                    .with_padding_top(3.0)
                                    .with_padding_bottom(4.0)
                                    .with_background(chat_date_bg())
                                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                                        999.0,
                                    )))
                                    .finish(),
                                )
                                .top_center()
                                .finish(),
                            )
                            .with_margin_top(10.0)
                            .with_margin_bottom(12.0)
                            .finish(),
                        );
                    }
                }
                if day_key.is_some() {
                    prev_day = day_key;
                }
                let grouped_with_next = self
                    .bubbles
                    .get(index + 1)
                    .map(|next| {
                        next.visible && message_is_grouped(Some(bubble.outgoing), next.outgoing)
                    })
                    .unwrap_or(false);
                let margin_bottom = message_row_margin_bottom(grouped_with_next);
                let mut row = Flex::column().with_main_axis_size(MainAxisSize::Min);
                row.add_child(ChildView::new(&bubble.handle).finish());
                if let Some(room_id) = bubble.video_room_id.clone() {
                    let join = EventHandler::new(
                        Container::new(
                            ui_text::body("加入群视频".to_string(), self.font)
                                .with_color(theme::accent_cool())
                                .finish(),
                        )
                        .with_uniform_padding(8.0)
                        .finish(),
                    )
                    .with_automation_label("加入群视频")
                    .with_automation_id(format!("chat:join_video:{room_id}"))
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(ChatThreadAction::JoinVideoRoom {
                            room_id: room_id.clone(),
                        });
                        DispatchEventResult::StopPropagation
                    })
                    .finish();
                    row.add_child(join);
                }
                col.add_child(
                    SavePosition::new(
                        Container::new(row.finish())
                            .with_margin_bottom(margin_bottom)
                            .finish(),
                        &message_position_id(&self.messages[index].id),
                    )
                    .for_single_frame()
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

impl TypedActionView for ChatThreadView {
    type Action = ChatThreadAction;

    fn handle_action(&mut self, action: &ChatThreadAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatThreadAction::LoadEarlier => {
                if self.loading_older_conv.is_some() {
                    return;
                }
                let Some(conv_id) = self.loaded_for.clone() else {
                    return;
                };
                let Some(before) = self.history_before.get(&conv_id).copied() else {
                    return;
                };
                self.fetch_older_messages(conv_id, Some(before), ctx);
                ctx.notify();
            }
            ChatThreadAction::JoinVideoRoom { room_id } => {
                let core = self.core.clone();
                let shell_state = self.shell_state.clone();
                let room_id = room_id.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        video_chat_join(
                            &runtime.state,
                            VideoChatJoinParams {
                                room_id,
                                display_name: None,
                            },
                        )
                        .await
                    },
                    move |_view, result, ctx| match result {
                        Ok(join) => {
                            if let Err(err) = open_video_viewer_url(&join.open_url) {
                                if let Ok(mut state) = shell_state.lock() {
                                    state.show_toast(
                                        format!("无法打开群视频: {err}"),
                                        StatusTone::Danger,
                                    );
                                }
                            } else if let Ok(mut state) = shell_state.lock() {
                                state.show_toast("已打开群视频", StatusTone::Success);
                            }
                            ctx.notify();
                        }
                        Err(err) => {
                            let (text, tone) = voice_call_ui::video_error_toast(&err);
                            if let Ok(mut state) = shell_state.lock() {
                                state.show_toast(text, tone);
                            }
                            ctx.notify();
                        }
                    },
                );
                ctx.notify();
            }
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &ChatThreadAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let label = match action {
            ChatThreadAction::LoadEarlier => "加载更早消息",
            ChatThreadAction::JoinVideoRoom { .. } => "加入群视频",
        };
        ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
            label,
            WarpA11yRole::ButtonRole,
        ))
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
            source_kind: None,
            reply_to: None,
            forwarded_from: None,
        }
    }

    #[test]
    fn history_cursor_only_moves_toward_older_messages() {
        let messages = vec![
            sample_message("newest", "three", 30),
            sample_message("oldest", "one", 10),
            sample_message("middle", "two", 20),
        ];

        assert_eq!(advance_history_before(None, &messages), Some(10));
        assert_eq!(advance_history_before(Some(5), &messages), Some(5));
        assert_eq!(advance_history_before(Some(15), &messages), Some(10));
        assert_eq!(advance_history_before(Some(5), &[]), None);
    }

    #[test]
    fn short_history_page_marks_history_exhausted() {
        assert!(page_exhausts_history(0));
        assert!(page_exhausts_history(RECENT_MESSAGE_PAGE - 1));
        assert!(!page_exhausts_history(RECENT_MESSAGE_PAGE));
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
    fn merge_pending_messages_keeps_empty_caption_image() {
        let base = vec![sample_message("m1", "", 5_000)];
        let pending = vec![PendingOutgoingMessage {
            client_id: "pending:img".into(),
            conv_id: "conv".into(),
            body: String::new(),
            sent_at: 5_100,
            attachments: vec![crate::ui::chat::shell_state::PendingOutgoingAttachment {
                kind: "image".into(),
                name: "shot.png".into(),
                size: 12,
                local_path: Some("/tmp/shot.png".into()),
            }],
        }];
        let merged = merge_pending_messages(&base, &pending, Some("local"));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].id, "pending:img");
        assert_eq!(merged[1].attachments[0].local_path.as_deref(), Some("/tmp/shot.png"));
    }

    #[test]
    fn merge_pending_messages_dedupes_confirmed_image_by_name() {
        let mut confirmed = sample_message("m2", "", 5_000);
        confirmed.attachments = vec![ChatAttachmentDto {
            id: "att-1".into(),
            kind: "image".into(),
            name: "shot.png".into(),
            size: 12,
            mime: None,
            source_hash: None,
            local_path: Some("/tmp/shot.png".into()),
        }];
        let pending = vec![PendingOutgoingMessage {
            client_id: "pending:img".into(),
            conv_id: "conv".into(),
            body: String::new(),
            sent_at: 5_100,
            attachments: vec![crate::ui::chat::shell_state::PendingOutgoingAttachment {
                kind: "image".into(),
                name: "shot.png".into(),
                size: 12,
                local_path: Some("/tmp/shot.png".into()),
            }],
        }];
        let merged = merge_pending_messages(&[confirmed], &pending, Some("local"));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "m2");
    }

    #[test]
    fn merge_pending_messages_fills_missing_local_path() {
        let mut confirmed = sample_message("m2", "", 5_000);
        confirmed.attachments = vec![ChatAttachmentDto {
            id: "att-1".into(),
            kind: "image".into(),
            name: "shot.png".into(),
            size: 12,
            mime: None,
            source_hash: None,
            local_path: None,
        }];
        let pending = vec![PendingOutgoingMessage {
            client_id: "pending:img".into(),
            conv_id: "conv".into(),
            body: String::new(),
            sent_at: 5_100,
            attachments: vec![crate::ui::chat::shell_state::PendingOutgoingAttachment {
                kind: "image".into(),
                name: "shot.png".into(),
                size: 12,
                local_path: Some("/tmp/shot.png".into()),
            }],
        }];
        let merged = merge_pending_messages(&[confirmed], &pending, Some("local"));
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].attachments[0].local_path.as_deref(),
            Some("/tmp/shot.png")
        );
    }

    #[test]
    fn remap_pending_attachment_previews_moves_asset_id() {
        let mut confirmed = sample_message("m2", "", 5_000);
        confirmed.attachments = vec![ChatAttachmentDto {
            id: "att-1".into(),
            kind: "image".into(),
            name: "shot.png".into(),
            size: 12,
            mime: None,
            source_hash: None,
            local_path: Some("/tmp/shot.png".into()),
        }];
        let pending = vec![PendingOutgoingMessage {
            client_id: "pending:img".into(),
            conv_id: "conv".into(),
            body: String::new(),
            sent_at: 5_100,
            attachments: vec![crate::ui::chat::shell_state::PendingOutgoingAttachment {
                kind: "image".into(),
                name: "shot.png".into(),
                size: 12,
                local_path: Some("/tmp/shot.png".into()),
            }],
        }];
        let mut previews = HashMap::new();
        previews.insert("pending-attach:shot.png".into(), "asset-1".into());
        let mut sizes = HashMap::new();
        sizes.insert("pending-attach:shot.png".into(), (10u32, 20u32));
        remap_pending_attachment_previews(&mut previews, &mut sizes, &[confirmed], &pending);
        assert_eq!(previews.get("att-1").map(String::as_str), Some("asset-1"));
        assert_eq!(sizes.get("att-1").copied(), Some((10, 20)));
        assert!(!previews.contains_key("pending-attach:shot.png"));
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
