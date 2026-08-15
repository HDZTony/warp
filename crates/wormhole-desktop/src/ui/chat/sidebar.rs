use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, ScrollbarWidth, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::bubble::format_sidebar_time;
use crate::ui::chat::labels::{
    chat_avatar_for_os, conversation_device_title, conversation_os_label, conversation_preview,
    find_cluster_node,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::fetch_cluster_for_ui;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    chat_item_active_bg, chat_search_pill, chat_sidebar_search_bg, positioned_context_menu,
    tg_avatar, StatusTone, AGENT_ROW_RADIUS, HUD_RADIUS,
};
use crate::ui::text_field_input::{
    render_search_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click_with_label,
    CaretBlink, CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_create_cluster_group, chat_list_conversations, chat_search_sidebar,
    chat_start_contact_conversation, chat_start_conversation, ChatConversationDto,
    ChatGroupMemberDto, ChatSidebarHitDto, CreateClusterGroupParams, SearchChatSidebarParams,
    StartChatContactParams, StartChatConversationParams,
};
use wormhole_desktop_core::chat_contacts::{
    aggregate_cluster_contacts_by_account, chat_contacts_list_manual, merge_contact_rows,
    ContactDto,
};
use wormhole_desktop_core::chat_ui_prefs::{
    load_chat_ui_prefs, mark_chat_read, set_chat_hidden, set_chat_muted, ChatUiPrefs,
};
use wormhole_desktop_core::cluster_commands::{cluster_status_hud, ClusterStatusDto};
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};

use std::collections::BTreeMap;

pub const TG_SIDEBAR_AVATAR: f32 = 46.0;
/// Inner content width inside sidebar item horizontal padding (12px × 2 in 300px column).
const CHAT_ITEM_INNER_WIDTH: f32 = 276.0;
const CTX_MENU_WIDTH: f32 = 160.0;
const SIDEBAR_COL_WIDTH: f32 = 300.0;
/// Header: 8px×2 padding + 34px menu + 6px gap → remaining search pill width.
const SEARCH_PILL_WIDTH: f32 = SIDEBAR_COL_WIDTH - 16.0 - 34.0 - 6.0;

/// Telegram-style search categories shown while the search field is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchCategory {
    Terminal,
    Group,
    Contact,
    Image,
    Video,
    File,
    Post,
}

impl SearchCategory {
    const ALL: [SearchCategory; 7] = [
        SearchCategory::Terminal,
        SearchCategory::Group,
        SearchCategory::Contact,
        SearchCategory::Image,
        SearchCategory::Video,
        SearchCategory::File,
        SearchCategory::Post,
    ];

    fn automation_id(self) -> &'static str {
        match self {
            Self::Terminal => "chat:search_cat:terminals",
            Self::Group => "chat:search_cat:groups",
            Self::Contact => "chat:search_cat:contacts",
            Self::Image => "chat:search_cat:images",
            Self::Video => "chat:search_cat:videos",
            Self::File => "chat:search_cat:files",
            Self::Post => "chat:search_cat:posts",
        }
    }

    fn i18n_key(self) -> &'static str {
        match self {
            Self::Terminal => "chat.sidebar.tab.terminals",
            Self::Group => "chat.sidebar.tab.groups",
            Self::Contact => "chat.sidebar.tab.contacts",
            Self::Image => "chat.sidebar.tab.images",
            Self::Video => "chat.sidebar.tab.videos",
            Self::File => "chat.sidebar.tab.files",
            Self::Post => "chat.sidebar.tab.posts",
        }
    }

    fn is_media(self) -> bool {
        matches!(
            self,
            Self::Image | Self::Video | Self::File | Self::Post
        )
    }

    fn sidebar_api_category(self) -> Option<&'static str> {
        match self {
            Self::Image => Some("image"),
            Self::Video => Some("video"),
            Self::File => Some("document"),
            Self::Post => Some("posts"),
            _ => None,
        }
    }
}

/// In-progress pointer drag on the search category bar (pan, not select-until-up).
#[derive(Debug, Clone)]
struct CategoryDrag {
    down_x: f32,
    scroll_at_down: f32,
    moved: bool,
    pending_select: Option<SearchCategory>,
}

const CATEGORY_DRAG_THRESHOLD_PX: f32 = 4.0;
const CATEGORY_WHEEL_STEP_PX: f32 = 48.0;
const CATEGORY_WHEEL_PRECISE_SCALE: f32 = 24.0;

fn presence_state(online: bool, raw: &str) -> String {
    if online || raw == "online" {
        "online".into()
    } else {
        match raw {
            "signed_in" | "recently_seen" => "recently_online".into(),
            "offline" => "offline".into(),
            _ => "unknown".into(),
        }
    }
}

fn sidebar_row_from_hit(hit: ChatSidebarHitDto) -> SidebarRow {
    let preview = hit
        .attachment_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| hit.body.clone());
    let os = match hit.attachment_kind.as_deref() {
        Some("image") => "IMG",
        Some("video") => "VID",
        Some("document") => "DOC",
        _ => "POST",
    };
    SidebarRow {
        id: hit.conv_id.clone(),
        title: hit.conv_title,
        preview,
        time: format_sidebar_time(hit.sent_at),
        online: false,
        presence: "unknown".into(),
        unread: 0,
        os: os.into(),
        muted: false,
        kind: SidebarRowKind::MediaHit,
        message_id: Some(hit.message_id),
        peer_user_id: None,
        peer_endpoints: Vec::new(),
    }
}

#[derive(Debug, Clone)]
enum ContextItemStyle {
    Normal,
    Accent,
    Danger,
}

#[derive(Debug, Clone)]
pub enum ChatSidebarAction {
    Select(String),
    SelectHit { conv_id: String, message_id: String },
    SearchEdit(TextFieldEditAction),
    FocusSearch,
    ActivateSearch,
    BlurSearch,
    SelectSearchCategory(SearchCategory),
    CategoryPointerDown {
        x: f32,
        select: Option<SearchCategory>,
    },
    CategoryPointerMove {
        x: f32,
    },
    CategoryPointerUp,
    OpenContextMenu { id: String, x: f32, y: f32 },
    CloseContextMenu,
    ContextOpen,
    ContextToggleMute,
    ContextDelete,
    ToggleSidebarMenu,
    CloseSidebarMenu,
    MenuHover(Option<&'static str>),
    MenuBtnHover(bool),
    MenuNewGroup,
    MenuNewChannel,
    MenuContacts,
    MenuCalls,
    MenuFavorites,
}

#[derive(Debug, Clone)]
pub enum ChatSidebarEvent {
    Selected(String),
    OpenContacts,
    OpenCalls,
    OpenChannel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarRowKind {
    Terminal,
    Group,
    Contact,
    Channel,
    MediaHit,
}

#[derive(Debug, Clone)]
struct SidebarRow {
    id: String,
    title: String,
    preview: String,
    time: String,
    online: bool,
    presence: String,
    unread: u32,
    /// OS label for avatar initials (`PC` / `iOS` / …).
    os: String,
    muted: bool,
    kind: SidebarRowKind,
    /// For media/post hits: jump target message id.
    message_id: Option<String>,
    /// Contact book rows carry peer user id for starting a contact DM.
    peer_user_id: Option<String>,
    peer_endpoints: Vec<(String, Vec<String>)>,
}

pub struct ChatSidebarView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    emoji_font: FamilyId,
    rows: Vec<SidebarRow>,
    hit_rows: Vec<SidebarRow>,
    contacts: Vec<ContactDto>,
    conversations: Vec<ChatConversationDto>,
    cluster: Option<ClusterStatusDto>,
    remarks: BTreeMap<String, String>,
    ui_prefs: ChatUiPrefs,
    selecting: Option<String>,
    search: String,
    search_field: TextFieldState,
    search_focused: bool,
    search_category: SearchCategory,
    category_scroll: ClippedScrollStateHandle,
    category_drag: Option<CategoryDrag>,
    hit_fetch_generation: u64,
    caret_blink: CaretBlink,
    status: String,
    scroll: ClippedScrollStateHandle,
    context_menu: Option<(String, f32, f32)>,
    menu_hover: Option<&'static str>,
    menu_btn_hover: bool,
    last_prefs_tick: u64,
    refresh_in_flight: bool,
    refresh_pending: bool,
}

impl ChatSidebarView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        let mut view = Self {
            core,
            selection,
            shell_state,
            font,
            emoji_font,
            rows: Vec::new(),
            hit_rows: Vec::new(),
            contacts: Vec::new(),
            conversations: Vec::new(),
            cluster: None,
            remarks: BTreeMap::new(),
            ui_prefs: ChatUiPrefs::default(),
            selecting: None,
            search: String::new(),
            search_field: TextFieldState::new(),
            search_focused: false,
            search_category: SearchCategory::Terminal,
            category_scroll: ClippedScrollStateHandle::new(),
            category_drag: None,
            hit_fetch_generation: 0,
            caret_blink: CaretBlink::new(),
            status: String::new(),
            scroll: ClippedScrollStateHandle::new(),
            context_menu: None,
            menu_hover: None,
            menu_btn_hover: false,
            last_prefs_tick: 0,
            refresh_in_flight: false,
            refresh_pending: false,
        };
        view.refresh(ctx);
        view.start_prefs_poll(ctx);
        view
    }

    fn start_prefs_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            },
            |view, _, ctx| {
                let tick = view
                    .shell_state
                    .lock()
                    .map(|state| state.prefs_tick)
                    .unwrap_or(0);
                if tick != view.last_prefs_tick {
                    view.last_prefs_tick = tick;
                    view.reload_ui_prefs(ctx);
                }
                view.start_prefs_poll(ctx);
            },
        );
    }

    fn reload_ui_prefs(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let state = runtime.state.clone();
                load_chat_ui_prefs(&state.data_dir)
                    .await
                    .unwrap_or_default()
            },
            |view, prefs, ctx| {
                view.ui_prefs = prefs;
                view.rows = view.build_rows(view.conversations.clone(), view.cluster.as_ref());
                ctx.notify();
            },
        );
    }

    pub(crate) fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        if self.refresh_in_flight {
            self.refresh_pending = true;
            return;
        }
        self.refresh_in_flight = true;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let cfg = chat_config(app, &state).await;
                let list = chat_list_conversations(app, &state).await;
                let cluster = cluster_status_hud(&state).await;
                let remarks = load_device_remarks(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let ui_prefs = load_chat_ui_prefs(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let manual = chat_contacts_list_manual(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let local_endpoint = state
                    .manager
                    .node_if_ready()
                    .map(|n| n.endpoint_id().to_string())
                    .unwrap_or_default();
                let contacts = match &cluster {
                    Ok(status) => {
                        let local_user_id = status
                            .nodes
                            .iter()
                            .find(|node| node.node_id == status.local_node_id)
                            .and_then(|node| node.user_id.clone());
                        let cluster_rows = aggregate_cluster_contacts_by_account(
                            &status.nodes,
                            &status.local_node_id,
                            local_user_id.as_deref(),
                            &local_endpoint,
                            &remarks,
                        );
                        merge_contact_rows(cluster_rows, manual)
                    }
                    Err(_) => merge_contact_rows(Vec::new(), manual),
                };
                (cfg, list, cluster, remarks, ui_prefs, contacts)
            },
            |view, output, ctx| {
                view.refresh_in_flight = false;
                let (cfg, list, cluster, remarks, ui_prefs, contacts) = output;
                if let Ok(c) = cfg {
                    view.status = c.display_name;
                }
                let conversations = list.unwrap_or_default();
                view.conversations = conversations.clone();
                match cluster {
                    Ok(status) => view.cluster = Some(status),
                    Err(_) => {}
                }
                view.remarks = remarks;
                view.ui_prefs = ui_prefs;
                view.contacts = contacts;
                view.rows = view.build_rows(conversations, view.cluster.as_ref());
                view.refresh_selected_summary();
                view.refresh_category_hits(ctx);
                ctx.notify();
                if view.refresh_pending {
                    view.refresh_pending = false;
                    view.refresh(ctx);
                }
            },
        );
    }

    fn create_default_cluster_group(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(cluster) = self.cluster.clone() else {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(wormhole_i18n::t("error.need_cluster"), StatusTone::Danger);
            }
            ctx.notify();
            return;
        };
        let Some(cluster_id) = cluster.cluster_id.clone() else {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(wormhole_i18n::t("error.need_cluster"), StatusTone::Danger);
            }
            ctx.notify();
            return;
        };
        let members: Vec<ChatGroupMemberDto> = cluster
            .nodes
            .iter()
            .filter_map(|node| {
                let endpoint = node.chat_endpoint_id.clone().unwrap_or_default();
                if endpoint.trim().is_empty() {
                    return None;
                }
                Some(ChatGroupMemberDto {
                    node_id: node.node_id.clone(),
                    endpoint,
                    display_name: Some(format!("{} · {}", node.os, node.hostname)),
                    bootstrap_addrs: node.chat_bootstrap_addrs.clone(),
                    is_local: node.node_id == cluster.local_node_id,
                })
            })
            .collect();
        if members.len() < 2 {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(wormhole_i18n::t("chat.toast.insufficient_members"), StatusTone::Danger);
            }
            ctx.notify();
            return;
        }
        let core = self.core.clone();
        let title = wormhole_i18n::t("chat.cluster_group");
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                chat_create_cluster_group(
                    app,
                    &runtime.state,
                    CreateClusterGroupParams {
                        title,
                        cluster_id,
                        members,
                        default_group: true,
                    },
                )
                .await
            },
            |view, result, ctx| match result {
                Ok(conv) => {
                    view.apply_selection(conv.id, ctx);
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.group_created"), StatusTone::Success);
                    }
                    view.refresh(ctx);
                }
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t_args(
                                "chat.toast.group_create_failed",
                                &[("err", &err.to_string())],
                            ),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                }
            },
        );
        ctx.notify();
    }

    fn apply_selection(&mut self, conv_id: String, ctx: &mut ViewContext<Self>) {
        if self.ui_prefs.is_hidden(&conv_id) {
            let core = self.core.clone();
            let hide_id = conv_id.clone();
            ctx.spawn(
                async move {
                    let runtime = core.runtime();
                    set_chat_hidden(&runtime.state.data_dir, &hide_id, false).await
                },
                move |view, output, ctx| {
                    if let Ok(prefs) = output {
                        view.apply_ui_prefs(prefs, ctx);
                    }
                    view.finish_apply_selection(conv_id, ctx);
                },
            );
            return;
        }
        self.finish_apply_selection(conv_id, ctx);
    }

    fn finish_apply_selection(&mut self, conv_id: String, ctx: &mut ViewContext<Self>) {
        if let Ok(mut guard) = self.selection.lock() {
            *guard = Some(conv_id.clone());
        }
        self.selecting = None;
        if let Some(row) = self.rows.iter_mut().find(|row| row.id == conv_id) {
            row.unread = 0;
        }
        let summary = self.rows.iter().find(|row| {
            row.id == conv_id
                || self.conversations.iter().any(|conv| {
                    conv.id == conv_id && (conv.peer_endpoint == row.id || conv.id == row.id)
                })
        });
        if let Ok(mut state) = self.shell_state.lock() {
            state.clear_pending_open();
            state.clear_open_error();
            state.clear_toast();
            if let Some(row) = summary {
                state.set_selected_summary(row.title.clone(), row.os.clone(), row.presence.clone());
            } else {
                state.clear_selected_summary();
            }
            state.bump_selection_tick();
        }
        let at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let core = self.core.clone();
        let read_id = conv_id.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let state = runtime.state.clone();
                mark_chat_read(&state.data_dir, &read_id, at_ms).await
            },
            move |view, output, ctx| {
                if let Ok(prefs) = output {
                    view.ui_prefs = prefs;
                }
                let _ = ctx;
            },
        );
        ctx.emit(ChatSidebarEvent::Selected(conv_id));
        ctx.notify();
    }

    fn refresh_selected_summary(&self) {
        let selected = self.selection.lock().ok().and_then(|guard| guard.clone());
        let Some(selected) = selected else {
            return;
        };
        let row = self
            .rows
            .iter()
            .find(|row| self.row_is_active(row, Some(&selected)));
        if let (Some(row), Ok(mut state)) = (row, self.shell_state.lock()) {
            let changed = state.selected_summary.as_ref().is_none_or(|current| {
                current.title != row.title
                    || current.os != row.os
                    || current.presence != row.presence
            });
            if changed {
                state.set_selected_summary(row.title.clone(), row.os.clone(), row.presence.clone());
                state.bump_selection_tick();
            }
        }
    }

    fn start_conversation_for_peer(
        &mut self,
        peer: String,
        display_name: Option<String>,
        bootstrap_addrs: Vec<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.selecting = Some(peer.clone());
        let title = display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| wormhole_i18n::t("chat.opening_conversation"));
        let (os, presence) = self
            .cluster
            .as_ref()
            .and_then(|cluster| {
                cluster.nodes.iter().find_map(|node| {
                    let endpoint = node
                        .chat_endpoint_id
                        .as_deref()
                        .unwrap_or(node.node_id.as_str());
                    if endpoint == peer.as_str() || node.node_id == peer {
                        Some((
                            node.os.clone(),
                            presence_state(node.online, &node.presence_status),
                        ))
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| (String::new(), "unknown".into()));
        if let Ok(mut state) = self.shell_state.lock() {
            state.set_pending_open(title, os, presence);
        }
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let params = StartChatConversationParams {
                    backend: None,
                    peer: Some(peer.clone()),
                    peer_endpoint: None,
                    peer_display_name: display_name,
                    peer_bootstrap_addrs: bootstrap_addrs,
                };
                chat_start_conversation(app, &state, params).await
            },
            |view, output, ctx| {
                view.selecting = None;
                match output {
                    Ok(conv) => {
                        view.apply_selection(conv.id, ctx);
                        view.refresh(ctx);
                    }
                    Err(err) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.clear_pending_open();
                            let message = wormhole_i18n::t_args(
                                "chat.toast.start_conversation_failed",
                                &[("err", &err.to_string())],
                            );
                            state.set_open_error(message.clone());
                            state.show_toast(message, StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn row_is_active(&self, row: &SidebarRow, selected: Option<&str>) -> bool {
        if let Some(selecting) = self.selecting.as_deref() {
            if row.id == selecting {
                return true;
            }
            if self.conversations.iter().any(|conv| {
                (conv.peer_endpoint == selecting || conv.id == selecting)
                    && (conv.peer_endpoint == row.id || conv.id == row.id)
            }) {
                return true;
            }
        }
        let Some(selected) = selected else {
            return false;
        };
        if row.id == selected {
            return true;
        }
        self.conversations
            .iter()
            .any(|conv| conv.id == selected && (conv.peer_endpoint == row.id || conv.id == row.id))
    }

    fn build_rows(
        &self,
        conversations: Vec<ChatConversationDto>,
        cluster: Option<&ClusterStatusDto>,
    ) -> Vec<SidebarRow> {
        let mut rows = Vec::new();
        for conv in &conversations {
            if self.ui_prefs.is_hidden(&conv.id) {
                continue;
            }
            let title = if conv.kind == "contact_direct" {
                conv.title
                    .clone()
                    .or_else(|| conv.peer_display_name.clone())
                    .unwrap_or_else(|| wormhole_i18n::t("chat.label.contact"))
            } else if conv.kind == "channel" {
                conv.title
                    .clone()
                    .or_else(|| conv.peer_display_name.clone())
                    .unwrap_or_else(|| wormhole_i18n::t("chat.label.channel"))
            } else if conv.kind == "cluster_group" {
                conv.title
                    .clone()
                    .or_else(|| conv.peer_display_name.clone())
                    .unwrap_or_else(|| wormhole_i18n::t("chat.label.group"))
            } else {
                let remark = find_cluster_node(conv, cluster)
                    .and_then(|node| self.remarks.get(&node.node_id).map(String::as_str));
                display_name_with_remark(remark, || conversation_device_title(conv, cluster))
            };
            let preview = conversation_preview(conv, cluster);
            let time = conv
                .last_message_at
                .map(format_sidebar_time)
                .unwrap_or_default();
            let online = find_cluster_node(conv, cluster)
                .map(|node| node.online)
                .unwrap_or(false);
            let presence = find_cluster_node(conv, cluster)
                .map(|node| presence_state(node.online, &node.presence_status))
                .unwrap_or_else(|| "unknown".into());
            let kind = match conv.kind.as_str() {
                "contact_direct" => SidebarRowKind::Contact,
                "cluster_group" => SidebarRowKind::Group,
                "channel" => SidebarRowKind::Channel,
                _ => SidebarRowKind::Terminal,
            };
            rows.push(SidebarRow {
                id: conv.id.clone(),
                title,
                preview,
                time,
                online,
                presence,
                unread: conv.unread_count,
                os: conversation_os_label(conv, cluster),
                muted: self.ui_prefs.is_muted(&conv.id),
                kind,
                message_id: None,
                peer_user_id: conv.peer_user_id.clone(),
                peer_endpoints: Vec::new(),
            });
        }
        if let Some(cluster) = cluster {
            for node in &cluster.nodes {
                if node.node_id == cluster.local_node_id {
                    continue;
                }
                if conversations.iter().any(|conv| {
                    !self.ui_prefs.is_hidden(&conv.id)
                        && conversation_covers_cluster_node(conv, node)
                }) {
                    continue;
                }
                let id = node
                    .chat_endpoint_id
                    .clone()
                    .unwrap_or_else(|| node.node_id.clone());
                if self.ui_prefs.is_hidden(&id) {
                    continue;
                }
                let title = display_name_with_remark(
                    self.remarks.get(&node.node_id).map(String::as_str),
                    || format!("{} · {}", node.os, node.hostname),
                );
                rows.push(SidebarRow {
                    id: id.clone(),
                    title,
                    preview: if node.online {
                        wormhole_i18n::t("chat.waiting")
                    } else {
                        wormhole_i18n::t("common.offline")
                    },
                    time: String::new(),
                    online: node.online,
                    presence: presence_state(node.online, &node.presence_status),
                    unread: 0,
                    os: node.os.clone(),
                    muted: self.ui_prefs.is_muted(&id),
                    kind: SidebarRowKind::Terminal,
                    message_id: None,
                    peer_user_id: None,
                    peer_endpoints: Vec::new(),
                });
            }
        }
        rows
    }

    /// Open or start a DM for a cluster node (used from Devices Tab 「发信息」).
    pub fn open_chat_for_cluster_node(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        if self.selecting.is_some() {
            return;
        }
        if let Some(cluster) = self.cluster.as_ref() {
            if node_id == cluster.local_node_id {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.cannot_message_self"), StatusTone::Danger);
                }
                ctx.notify();
                return;
            }
            if let Some(node) = cluster.nodes.iter().find(|node| {
                node.node_id == node_id
                    || node.chat_endpoint_id.as_deref() == Some(node_id.as_str())
            }) {
                let Some(peer) = node
                    .chat_endpoint_id
                    .clone()
                    .filter(|id| !id.trim().is_empty())
                else {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.no_chat_endpoint"), StatusTone::Danger);
                    }
                    ctx.notify();
                    return;
                };
                if let Some(conv) = self
                    .conversations
                    .iter()
                    .find(|conv| conv.peer_endpoint == peer)
                {
                    // Re-open existing DM via start path so invite/bootstrap repair runs.
                    let display_name = Some(format!("{} · {}", node.os, node.hostname));
                    let bootstrap_addrs = node.chat_bootstrap_addrs.clone();
                    let _ = conv;
                    self.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                    return;
                }
                let display_name = Some(format!("{} · {}", node.os, node.hostname));
                let bootstrap_addrs = node.chat_bootstrap_addrs.clone();
                self.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                return;
            }
            if let Some(conv_id) =
                find_conversation_for_node_ref(&self.conversations, Some(cluster), &node_id)
            {
                self.apply_selection(conv_id, ctx);
                return;
            }
        }
        // Cluster cache miss: refresh then open.
        let stale_peer = self.cluster.as_ref().and_then(|cluster| {
            cluster.nodes.iter().find_map(|node| {
                if node.node_id == node_id
                    || node.chat_endpoint_id.as_deref() == Some(node_id.as_str())
                {
                    node.chat_endpoint_id.clone()
                } else {
                    None
                }
            })
        });
        let core = self.core.clone();
        self.selecting = Some(node_id.clone());
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let list = chat_list_conversations(app, &state).await;
                let cluster = fetch_cluster_for_ui(&state).await?;
                Ok::<_, String>((list.unwrap_or_default(), cluster, node_id, stale_peer))
            },
            |view, output, ctx| {
                view.selecting = None;
                match output {
                    Ok((conversations, cluster, node_id, stale_peer)) => {
                        view.conversations = conversations.clone();
                        view.cluster = Some(cluster.clone());
                        view.rows = view.build_rows(conversations, Some(&cluster));
                        if node_id == cluster.local_node_id {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(wormhole_i18n::t("chat.toast.cannot_message_self"), StatusTone::Danger);
                            }
                            ctx.notify();
                            return;
                        }
                        let node = cluster.nodes.iter().find(|node| {
                            node.node_id == node_id
                                || node.chat_endpoint_id.as_deref() == Some(node_id.as_str())
                        });
                        let Some(node) = node else {
                            if let Some(conv_id) = find_conversation_for_node_ref(
                                &view.conversations,
                                Some(&cluster),
                                &node_id,
                            ) {
                                view.apply_selection(conv_id, ctx);
                                return;
                            }
                            if let Some(peer) = stale_peer.filter(|id| !id.trim().is_empty()) {
                                if let Some(conv) = view
                                    .conversations
                                    .iter()
                                    .find(|conv| conv.peer_endpoint == peer)
                                {
                                    view.apply_selection(conv.id.clone(), ctx);
                                    return;
                                }
                            }
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(wormhole_i18n::t("chat.toast.device_not_found"), StatusTone::Danger);
                            }
                            ctx.notify();
                            return;
                        };
                        let Some(peer) = node
                            .chat_endpoint_id
                            .clone()
                            .filter(|id| !id.trim().is_empty())
                        else {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(wormhole_i18n::t("chat.toast.no_chat_endpoint"), StatusTone::Danger);
                            }
                            ctx.notify();
                            return;
                        };
                        if let Some(conv) = view
                            .conversations
                            .iter()
                            .find(|conv| conv.peer_endpoint == peer)
                        {
                            view.apply_selection(conv.id.clone(), ctx);
                            return;
                        }
                        let display_name = Some(format!("{} · {}", node.os, node.hostname));
                        let bootstrap_addrs = node.chat_bootstrap_addrs.clone();
                        view.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                    }
                    Err(err) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.show_toast(
                                wormhole_i18n::t_args(
                                    "chat.toast.open_chat_failed",
                                    &[("err", &err.to_string())],
                                ),
                                StatusTone::Danger,
                            );
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn filtered_rows(&self) -> Vec<SidebarRow> {
        let q = self.search.trim().to_lowercase();
        let favorites_only = self
            .shell_state
            .lock()
            .map(|s| s.favorites_only)
            .unwrap_or(false);
        if favorites_only {
            return Vec::new();
        }

        if self.search_focused && self.search_category.is_media() {
            return self
                .hit_rows
                .iter()
                .filter(|row| {
                    q.is_empty()
                        || row.title.to_lowercase().contains(&q)
                        || row.preview.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
        }

        if self.search_focused && self.search_category == SearchCategory::Contact {
            return self.contact_category_rows(q.as_str());
        }

        let category_filter = if self.search_focused {
            Some(self.search_category)
        } else {
            None
        };

        self.rows
            .iter()
            .filter(|row| {
                if let Some(category) = category_filter {
                    let matches_kind = match category {
                        SearchCategory::Terminal => row.kind == SidebarRowKind::Terminal,
                        SearchCategory::Group => row.kind == SidebarRowKind::Group,
                        SearchCategory::Contact => row.kind == SidebarRowKind::Contact,
                        _ => true,
                    };
                    if !matches_kind {
                        return false;
                    }
                }
                q.is_empty()
                    || row.title.to_lowercase().contains(&q)
                    || row.preview.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    fn contact_category_rows(&self, q: &str) -> Vec<SidebarRow> {
        let mut out: Vec<SidebarRow> = self
            .rows
            .iter()
            .filter(|row| row.kind == SidebarRowKind::Contact)
            .filter(|row| {
                q.is_empty()
                    || row.title.to_lowercase().contains(q)
                    || row.preview.to_lowercase().contains(q)
            })
            .cloned()
            .collect();

        let existing_user_ids: std::collections::HashSet<String> = out
            .iter()
            .filter_map(|row| row.peer_user_id.clone())
            .collect();
        let existing_conv_peers: std::collections::HashSet<String> = self
            .conversations
            .iter()
            .filter(|conv| conv.kind == "contact_direct")
            .filter_map(|conv| conv.peer_user_id.clone())
            .collect();

        for contact in &self.contacts {
            if !contact.can_chat {
                continue;
            }
            let Some(user_id) = contact.user_id.clone() else {
                continue;
            };
            if existing_user_ids.contains(&user_id) || existing_conv_peers.contains(&user_id) {
                continue;
            }
            if !q.is_empty()
                && !contact.display_name.to_lowercase().contains(q)
                && !user_id.to_lowercase().contains(q)
            {
                continue;
            }
            let endpoints: Vec<(String, Vec<String>)> = contact
                .endpoints
                .iter()
                .cloned()
                .zip(contact.endpoint_bootstraps.iter().cloned())
                .collect();
            out.push(SidebarRow {
                id: contact.id.clone(),
                title: contact.display_name.clone(),
                preview: if contact.device_count > 1 {
                    format!("{} devices", contact.device_count)
                } else {
                    wormhole_i18n::t("chat.label.contact")
                },
                time: String::new(),
                online: false,
                presence: "unknown".into(),
                unread: 0,
                os: "CT".into(),
                muted: false,
                kind: SidebarRowKind::Contact,
                message_id: None,
                peer_user_id: Some(user_id),
                peer_endpoints: endpoints,
            });
        }
        out
    }

    fn refresh_category_hits(&mut self, ctx: &mut ViewContext<Self>) {
        if !(self.search_focused && self.search_category.is_media()) {
            self.hit_rows.clear();
            return;
        }
        let Some(api_category) = self.search_category.sidebar_api_category() else {
            return;
        };
        self.hit_fetch_generation = self.hit_fetch_generation.saturating_add(1);
        let generation = self.hit_fetch_generation;
        let query = self.search.clone();
        let category = api_category.to_string();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                chat_search_sidebar(
                    runtime.ctx.as_ref(),
                    &runtime.state,
                    SearchChatSidebarParams {
                        category,
                        query: if query.trim().is_empty() {
                            None
                        } else {
                            Some(query)
                        },
                        cursor: None,
                        limit: Some(50),
                    },
                )
                .await
            },
            move |view, result, ctx| {
                if view.hit_fetch_generation != generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        view.hit_rows = page
                            .hits
                            .into_iter()
                            .map(|hit| sidebar_row_from_hit(hit))
                            .collect();
                    }
                    Err(err) => {
                        view.hit_rows.clear();
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.show_toast(
                                wormhole_i18n::t_args(
                                    "chat.toast.open_chat_failed",
                                    &[("err", &err.to_string())],
                                ),
                                StatusTone::Danger,
                            );
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn set_search_category(&mut self, category: SearchCategory, ctx: &mut ViewContext<Self>) {
        if self.search_category == category {
            return;
        }
        self.search_category = category;
        self.refresh_category_hits(ctx);
        ctx.notify();
    }

    fn blur_search(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.search_focused {
            return;
        }
        self.search_focused = false;
        self.search_category = SearchCategory::Terminal;
        self.hit_rows.clear();
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn start_contact_conversation_for_row(
        &mut self,
        row: &SidebarRow,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(peer_user_id) = row.peer_user_id.clone() else {
            self.apply_selection(row.id.clone(), ctx);
            return;
        };
        self.selecting = Some(row.id.clone());
        let display_name = Some(row.title.clone());
        let peer_endpoints = row.peer_endpoints.clone();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                chat_start_contact_conversation(
                    runtime.ctx.as_ref(),
                    &runtime.state,
                    StartChatContactParams {
                        peer_user_id,
                        peer_display_name: display_name,
                        peer_endpoints,
                    },
                )
                .await
            },
            |view, output, ctx| {
                view.selecting = None;
                match output {
                    Ok(conv) => {
                        view.apply_selection(conv.id, ctx);
                        view.refresh(ctx);
                    }
                    Err(err) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            let message = wormhole_i18n::t_args(
                                "chat.toast.start_conversation_failed",
                                &[("err", &err.to_string())],
                            );
                            state.set_open_error(message.clone());
                            state.show_toast(message, StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn chat_item(&self, row: &SidebarRow, selected: bool) -> Box<dyn Element> {
        let menu_id = row.id.clone();
        let title_color = if row.muted {
            theme::muted()
        } else {
            theme::text()
        };
        let mut top = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max);
        top.add_child(
            Expanded::new(
                1.0,
                ui_text::chat_sidebar_name(row.title.clone(), self.font)
                    .with_color(title_color)
                    .finish(),
            )
            .finish(),
        );
        if !row.time.is_empty() {
            top.add_child(
                Container::new(
                    ui_text::chat_sidebar_time(row.time.clone(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_left(8.0)
                .finish(),
            );
        }

        let preview_color = if row.muted {
            theme::muted()
        } else if selected {
            theme::text()
        } else {
            theme::muted()
        };
        let mut preview_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max);
        let preview_text = if row.muted && !row.preview.is_empty() {
            format!("🔇 {}", row.preview)
        } else {
            row.preview.clone()
        };
        let preview_font =
            crate::ui::fonts::chat_message_font(self.font, self.emoji_font, &preview_text);
        preview_row.add_child(
            Expanded::new(
                1.0,
                ui_text::chat_preview(preview_text, preview_font)
                    .with_color(preview_color)
                    .finish(),
            )
            .finish(),
        );
        if row.unread > 0 {
            let badge_bg = if row.muted {
                theme::muted()
            } else {
                theme::accent_cool()
            };
            let badge_fg = if row.muted {
                theme::text()
            } else {
                theme::chat_bubble_text()
            };
            preview_row.add_child(
                Container::new(
                    Align::new(
                        ui_text::device_meta(row.unread.to_string(), self.font)
                            .with_color(badge_fg)
                            .finish(),
                    )
                    .finish(),
                )
                .with_margin_left(8.0)
                .with_horizontal_padding(6.0)
                .with_vertical_padding(2.0)
                .with_background(badge_bg)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
                .finish(),
            );
        }

        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(top.finish());
        col.add_child(
            Container::new(preview_row.finish())
                .with_margin_top(3.0)
                .finish(),
        );

        // ClippedScrollable gives infinite max width; Max+Expanded must sit inside a
        // fixed-width ConstrainedBox (avatar 46 + gap 10 leave 220 for text).
        let text_col_width = CHAT_ITEM_INNER_WIDTH - TG_SIDEBAR_AVATAR - 10.0;
        let text_col = ConstrainedBox::new(col.finish())
            .with_width(text_col_width)
            .finish();

        let row_body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(tg_avatar(
                chat_avatar_for_os(&row.os),
                self.font,
                TG_SIDEBAR_AVATAR,
            ))
            .with_child(Container::new(text_col).with_margin_left(10.0).finish())
            .finish();

        let row_title = row.title.clone();
        let message_id = row.message_id.clone();
        let is_media_hit = row.kind == SidebarRowKind::MediaHit;
        let select_id = row.id.clone();
        let interactive = EventHandler::new(
            ConstrainedBox::new(row_body)
                .with_width(CHAT_ITEM_INNER_WIDTH)
                .finish(),
        )
        .with_automation_label(row_title)
        .with_automation_id(format!("chat:conversation:{select_id}"))
        .on_left_mouse_down(move |ctx, _, _| {
            if is_media_hit {
                if let Some(message_id) = message_id.clone() {
                    ctx.dispatch_typed_action(ChatSidebarAction::SelectHit {
                        conv_id: select_id.clone(),
                        message_id,
                    });
                }
            } else {
                ctx.dispatch_typed_action(ChatSidebarAction::Select(select_id.clone()));
            }
            DispatchEventResult::StopPropagation
        })
        .on_right_mouse_down(move |ctx, _, position| {
            ctx.dispatch_typed_action(ChatSidebarAction::OpenContextMenu {
                id: menu_id.clone(),
                x: position.x(),
                y: position.y(),
            });
            DispatchEventResult::StopPropagation
        })
        .finish();

        Container::new(interactive)
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(9.0)
            .with_padding_bottom(9.0)
            .with_background(if selected {
                chat_item_active_bg()
            } else {
                ColorU::transparent_black()
            })
            .finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let search_focused = self.search_focused;
        let draft = self.search.clone();
        let marked = self.search_field.marked_text.clone();
        let search_placeholder = wormhole_i18n::t("chat.search.placeholder");
        let field = render_search_field_with_caret(
            &draft,
            &marked,
            &search_placeholder,
            self.font,
            search_focused,
            false,
            self.caret_blink.visible,
            self.search_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ChatSidebarAction::SearchEdit(action));
        })
        .focused(search_focused)
        .ime_preedit(!marked.is_empty())
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "tab" || keystroke.key == "escape" {
                ctx.dispatch_typed_action(ChatSidebarAction::BlurSearch);
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        let input = wrap_text_field_focus_on_click_with_label(
            input,
            wormhole_i18n::t("chat.search.placeholder"),
            |ctx| {
            ctx.dispatch_typed_action(ChatSidebarAction::ActivateSearch);
        },
        );
        // Stable id for sim-use (label alone may be nested under TextFieldInput).
        let input = EventHandler::new(input)
            .with_automation_label(wormhole_i18n::t("chat.search.placeholder"))
            .with_automation_id("chat:sidebar_search")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::ActivateSearch);
                DispatchEventResult::PropagateToParent
            })
            .finish();

        let border_color = if search_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };

        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::chat_sidebar_search_icon(theme::muted()))
                    .with_horizontal_margin(2.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, input).finish())
            .finish();

        ConstrainedBox::new(chat_search_pill(
            row,
            chat_sidebar_search_bg(),
            border_color,
            7.0,
            10.0,
            AGENT_ROW_RADIUS,
        ))
        .with_width(SEARCH_PILL_WIDTH)
        .finish()
    }

    fn context_menu_divider() -> Box<dyn Element> {
        Container::new(Flex::column().finish())
            .with_vertical_margin(4.0)
            .with_horizontal_margin(6.0)
            .with_border(Border::top(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn context_menu_item(
        &self,
        label: String,
        action: ChatSidebarAction,
        style: ContextItemStyle,
        enabled: bool,
    ) -> Box<dyn Element> {
        let color = if !enabled {
            theme::muted()
        } else {
            match style {
                ContextItemStyle::Normal => theme::text(),
                ContextItemStyle::Accent => theme::accent_cool(),
                ContextItemStyle::Danger => theme::danger(),
            }
        };
        let menu_label = label.clone();
        let menu_id = format!("chat:sidebar_menu:{menu_label}");
        let handler = EventHandler::new(
            Container::new(
                ui_text::body(label, self.font)
                    .with_color(color)
                    .finish(),
            )
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_padding_top(8.0)
            .with_padding_bottom(8.0)
            .finish(),
        );
        if enabled {
            handler
                .with_automation_label(menu_label)
                .with_automation_id(menu_id)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            handler.finish()
        }
    }

    fn conversation_context_menu(&self) -> Box<dyn Element> {
        let (row_id, x, y) = self.context_menu.clone().unwrap_or_default();
        let muted = self.ui_prefs.is_muted(&row_id);
        let mut menu = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        menu.add_child(self.context_menu_item(
            wormhole_i18n::t("chat.context.open"),
            ChatSidebarAction::ContextOpen,
            ContextItemStyle::Normal,
            true,
        ));
        menu.add_child(self.context_menu_item(
            if muted {
                wormhole_i18n::t("chat.context.unmute")
            } else {
                wormhole_i18n::t("chat.context.mute")
            },
            ChatSidebarAction::ContextToggleMute,
            ContextItemStyle::Accent,
            true,
        ));
        menu.add_child(Self::context_menu_divider());
        menu.add_child(self.context_menu_item(
            wormhole_i18n::t("chat.context.delete"),
            ChatSidebarAction::ContextDelete,
            ContextItemStyle::Danger,
            true,
        ));

        let panel = Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(CTX_MENU_WIDTH)
                .finish(),
        )
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();

        positioned_context_menu(x, y, panel)
    }

    fn sidebar_menu_item(
        &self,
        label: String,
        icon_path: &'static str,
        hover_key: &'static str,
        action: ChatSidebarAction,
    ) -> Box<dyn Element> {
        let hovered = self.menu_hover == Some(hover_key);
        let menu_label = label.clone();
        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(icons::chat_sidebar_menu_row_icon(icon_path, theme::muted()))
            .with_child(
                Container::new(
                    ui_text::chat_sidebar_name(menu_label.clone(), self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_margin_left(16.0)
                .finish(),
            )
            .finish();
        EventHandler::new(
            ConstrainedBox::new(
                Container::new(row)
                    .with_padding_left(18.0)
                    .with_padding_right(18.0)
                    .with_background(if hovered {
                        theme::accent_cool_bg(20)
                    } else {
                        ColorU::transparent_black()
                    })
                    .finish(),
            )
            .with_min_height(48.0)
            .finish(),
        )
        .with_automation_label(menu_label)
        .with_automation_id(format!("chat:sidebar_nav:{hover_key}"))
        .on_mouse_in(
            move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::MenuHover(Some(hover_key)));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(move |ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::MenuHover(None));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn sidebar_hamburger_menu(&self) -> Box<dyn Element> {
        let mut menu = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        menu.add_child(self.sidebar_menu_item(
            wormhole_i18n::t("chat.sidebar.menu.new_group"),
            "chat-menu-new-group.svg",
            "new-group",
            ChatSidebarAction::MenuNewGroup,
        ));
        menu.add_child(self.sidebar_menu_item(
            wormhole_i18n::t("chat.sidebar.menu.new_channel"),
            "chat-menu-new-channel.svg",
            "new-channel",
            ChatSidebarAction::MenuNewChannel,
        ));
        menu.add_child(self.sidebar_menu_item(
            wormhole_i18n::t("chat.sidebar.menu.contacts"),
            "chat-menu-contacts.svg",
            "contacts",
            ChatSidebarAction::MenuContacts,
        ));
        menu.add_child(self.sidebar_menu_item(
            wormhole_i18n::t("chat.sidebar.menu.calls"),
            "chat-menu-calls.svg",
            "calls",
            ChatSidebarAction::MenuCalls,
        ));
        menu.add_child(self.sidebar_menu_item(
            wormhole_i18n::t("chat.sidebar.menu.favorites"),
            "chat-menu-favorites.svg",
            "favorites",
            ChatSidebarAction::MenuFavorites,
        ));
        Container::new(
            Container::new(menu.finish())
                .with_padding_top(4.0)
                .with_padding_bottom(4.0)
                .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn search_categories(&self) -> Box<dyn Element> {
        use warpui_core::units::Pixels;

        let mut row = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        for category in SearchCategory::ALL {
            let active = self.search_category == category;
            let label = wormhole_i18n::t(category.i18n_key());
            let color = if active {
                theme::accent_cool()
            } else {
                theme::muted()
            };
            let mut tab = Container::new(
                ui_text::device_meta(label.clone(), self.font)
                    .with_color(color)
                    .finish(),
            )
            .with_padding_top(8.0)
            .with_padding_bottom(10.0);
            if active {
                tab = tab.with_border(Border::bottom(2.0).with_border_fill(theme::accent_cool()));
            }
            let tab = EventHandler::new(
                Container::new(tab.finish())
                    .with_margin_right(18.0)
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(category.automation_id())
            .on_left_mouse_down(move |ctx, _, position| {
                ctx.dispatch_typed_action(ChatSidebarAction::CategoryPointerDown {
                    x: position.x(),
                    select: Some(category),
                });
                DispatchEventResult::StopPropagation
            })
            .on_mouse_dragged(|ctx, _, position| {
                ctx.dispatch_typed_action(ChatSidebarAction::CategoryPointerMove {
                    x: position.x(),
                });
                DispatchEventResult::StopPropagation
            })
            .on_left_mouse_up(|ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::CategoryPointerUp);
                DispatchEventResult::StopPropagation
            })
            .finish();
            row.add_child(tab);
        }
        let scrolled = ClippedScrollable::horizontal(
            self.category_scroll.clone(),
            row.finish(),
            ScrollbarWidth::None,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();
        let category_scroll = self.category_scroll.clone();
        // Horizontal ClippedScrollable consumes ScrollWheel (with x=0 for vertical
        // notches) before this handler runs; always_handle keeps pan mapping alive.
        EventHandler::new(
            Container::new(scrolled)
                .with_margin_top(8.0)
                .finish(),
        )
        .with_always_handle()
        .on_left_mouse_down(|ctx, _, position| {
            ctx.dispatch_typed_action(ChatSidebarAction::CategoryPointerDown {
                x: position.x(),
                select: None,
            });
            DispatchEventResult::StopPropagation
        })
        .on_mouse_dragged(|ctx, _, position| {
            ctx.dispatch_typed_action(ChatSidebarAction::CategoryPointerMove {
                x: position.x(),
            });
            DispatchEventResult::StopPropagation
        })
        .on_left_mouse_up(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::CategoryPointerUp);
            DispatchEventResult::StopPropagation
        })
        .on_scroll_wheel(move |ctx, _, delta, _| {
            let direction = if delta.y().abs() >= delta.x().abs() {
                delta.y()
            } else {
                delta.x()
            };
            if direction == 0.0 {
                return DispatchEventResult::PropagateToParent;
            }
            // Discrete notches are large; trackpad/precision deltas stay small.
            let pixels = if direction.abs() <= 2.0 {
                direction * CATEGORY_WHEEL_PRECISE_SCALE
            } else {
                direction.signum() * CATEGORY_WHEEL_STEP_PX
            };
            category_scroll.scroll_by(Pixels::new(pixels));
            ctx.notify();
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn clear_selection_if_matches(&mut self, id: &str, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let matches = selected.as_deref() == Some(id)
            || self.conversations.iter().any(|conv| {
                selected.as_deref() == Some(conv.id.as_str())
                    && (conv.peer_endpoint == id || conv.id == id)
            });
        if matches {
            if let Ok(mut guard) = self.selection.lock() {
                *guard = None;
            }
            if let Ok(mut state) = self.shell_state.lock() {
                state.clear_pending_open();
                state.clear_selected_summary();
                state.bump_selection_tick();
            }
        }
        let _ = ctx;
    }

    fn apply_ui_prefs(&mut self, prefs: ChatUiPrefs, ctx: &mut ViewContext<Self>) {
        self.ui_prefs = prefs;
        self.rows = self.build_rows(self.conversations.clone(), self.cluster.as_ref());
        if let Ok(mut state) = self.shell_state.lock() {
            state.bump_prefs_tick();
            self.last_prefs_tick = state.prefs_tick;
        }
        ctx.notify();
    }
}

impl Entity for ChatSidebarView {
    type Event = ChatSidebarEvent;
}

impl View for ChatSidebarView {
    fn ui_name() -> &'static str {
        "ChatSidebarView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let rows = self.filtered_rows();
        let menu_open = self.context_menu.is_some();

        let mut list = Flex::column().with_main_axis_size(MainAxisSize::Min);
        if rows.is_empty() {
            list.add_child(
                Container::new(
                    ui_text::body(
                        if self.search.is_empty() && !self.search_focused {
                            let favorites_only = self
                                .shell_state
                                .lock()
                                .map(|s| s.favorites_only)
                                .unwrap_or(false);
                            if favorites_only {
                                wormhole_i18n::t("common.empty.no_favorites")
                            } else if self.status.is_empty() {
                                wormhole_i18n::t("common.empty.no_conversations")
                            } else {
                                wormhole_i18n::t_args(
                                    "chat.empty.no_conversations_with_status",
                                    &[("status", &self.status)],
                                )
                            }
                        } else {
                            wormhole_i18n::t("chat.empty.no_matches")
                        },
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_uniform_padding(12.0)
                .finish(),
            );
        } else {
            for row in &rows {
                let active = self.row_is_active(row, selected.as_deref());
                list.add_child(self.chat_item(row, active));
            }
        }

        let sidebar_menu_open = self
            .shell_state
            .lock()
            .map(|s| s.sidebar_menu_open)
            .unwrap_or(false);
        let menu_btn_active = sidebar_menu_open || self.menu_btn_hover;
        let menu_btn = EventHandler::new(
            Container::new(
                ConstrainedBox::new(
                    Align::new(icons::chat_sidebar_menu_icon(if menu_btn_active {
                        theme::text()
                    } else {
                        theme::muted()
                    }))
                    .finish(),
                )
                .with_width(34.0)
                .with_height(34.0)
                .finish(),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(17.0)))
            .with_background(if menu_btn_active {
                theme::accent_cool_bg(28)
            } else {
                ColorU::new(0, 0, 0, 0)
            })
            .finish(),
        )
        .with_automation_label("侧边栏菜单")
        .with_automation_id("chat:sidebar_menu_btn")
        .on_mouse_in(
            |ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::MenuBtnHover(true));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::MenuBtnHover(false));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::BlurSearch);
            ctx.dispatch_typed_action(ChatSidebarAction::ToggleSidebarMenu);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut search_header = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(menu_btn)
                    .with_child(
                        Expanded::new(
                            1.0,
                            Container::new(self.search_box())
                                .with_padding_left(6.0)
                                .finish(),
                        )
                        .finish(),
                    )
                    .finish(),
            );
        if self.search_focused {
            search_header.add_child(self.search_categories());
        }

        let list_body = EventHandler::new(
            Container::new(
                ClippedScrollable::vertical(
                    self.scroll.clone(),
                    Container::new(list.finish())
                        .with_vertical_padding(4.0)
                        .finish(),
                    ScrollbarWidth::Auto,
                    Fill::None,
                    Fill::None,
                    Fill::None,
                )
                .finish(),
            )
            .with_background(theme::panel())
            .with_border(Border::right(1.0).with_border_fill(theme::border()))
            .finish(),
        )
        .with_automation_label("会话列表")
        .with_automation_id("chat:conversation_list")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::BlurSearch);
            DispatchEventResult::PropagateToParent
        })
        .finish();

        let list_layer = if sidebar_menu_open {
            let mut list_stack = Stack::new();
            list_stack.add_child(list_body);
            let scrim = EventHandler::new(
                Container::new(Flex::column().finish())
                    .with_background(ColorU::new(0, 0, 0, 0))
                    .finish(),
            )
            .with_automation_label("关闭侧边栏菜单")
            .with_automation_id("chat:sidebar_menu_scrim")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::BlurSearch);
                ctx.dispatch_typed_action(ChatSidebarAction::CloseSidebarMenu);
                DispatchEventResult::StopPropagation
            })
            .finish();
            list_stack.add_child(scrim);
            list_stack.add_child(
                Align::new(
                    ConstrainedBox::new(self.sidebar_hamburger_menu())
                        .with_width(SIDEBAR_COL_WIDTH)
                        .finish(),
                )
                .top_left()
                .finish(),
            );
            list_stack.finish()
        } else {
            list_body
        };

        let body = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Container::new(search_header.finish())
                    .with_horizontal_padding(8.0)
                    .with_vertical_padding(10.0)
                    .with_background(theme::panel())
                    .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                    .finish(),
            )
            .with_child(Expanded::new(1.0, list_layer).finish())
            .finish();

        if !menu_open {
            return body;
        }

        let mut stack = Stack::new();
        stack.add_child(body);
        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(8, 7, 11, 40))
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::CloseContextMenu);
            DispatchEventResult::StopPropagation
        })
        .finish();
        stack.add_child(scrim);
        stack.add_child(self.conversation_context_menu());
        EventHandler::new(stack.finish())
            .on_keydown(|ctx, _, keystroke| {
                if keystroke.key.as_str() == "escape" {
                    ctx.dispatch_typed_action(ChatSidebarAction::CloseContextMenu);
                    ctx.dispatch_typed_action(ChatSidebarAction::CloseSidebarMenu);
                    return DispatchEventResult::StopPropagation;
                }
                DispatchEventResult::PropagateToParent
            })
            .finish()
    }
}

impl TypedActionView for ChatSidebarView {
    type Action = ChatSidebarAction;

    fn handle_action(&mut self, action: &ChatSidebarAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatSidebarAction::Select(id) => {
                if self.selecting.is_some() {
                    return;
                }
                let contact_row = self
                    .contact_category_rows("")
                    .into_iter()
                    .find(|row| {
                        row.id == *id
                            && row.peer_user_id.is_some()
                            && !self.conversations.iter().any(|conv| conv.id == row.id)
                    });
                if self.search_focused {
                    self.blur_search(ctx);
                }
                if let Some(row) = contact_row {
                    self.start_contact_conversation_for_row(&row, ctx);
                    return;
                }
                match resolve_select_target(id, &self.conversations, self.cluster.as_ref()) {
                    SelectTarget::Conversation(conv_id) => {
                        self.apply_selection(conv_id, ctx);
                    }
                    SelectTarget::StartWithPeer {
                        peer,
                        display_name,
                        bootstrap_addrs,
                    } => {
                        self.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                    }
                    SelectTarget::MissingChatEndpoint => {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.set_open_error(wormhole_i18n::t("chat.toast.no_chat_endpoint"));
                            state.show_toast(wormhole_i18n::t("chat.toast.no_chat_endpoint"), StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            }
            ChatSidebarAction::SelectHit {
                conv_id,
                message_id,
            } => {
                if self.search_focused {
                    self.blur_search(ctx);
                }
                if let Ok(mut state) = self.shell_state.lock() {
                    state.set_pending_jump_message(message_id.clone());
                }
                self.apply_selection(conv_id.clone(), ctx);
            }
            ChatSidebarAction::FocusSearch => {
                self.search_focused = !self.search_focused;
                if !self.search_focused {
                    self.search_category = SearchCategory::Terminal;
                    self.hit_rows.clear();
                } else {
                    self.refresh_category_hits(ctx);
                }
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatSidebarAction::ActivateSearch => {
                if !self.search_focused {
                    self.search_focused = true;
                    self.refresh_category_hits(ctx);
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
            ChatSidebarAction::BlurSearch => {
                self.blur_search(ctx);
            }
            ChatSidebarAction::SelectSearchCategory(category) => {
                self.search_focused = true;
                self.set_search_category(*category, ctx);
                sync_caret_blink(self, ctx);
            }
            ChatSidebarAction::CategoryPointerDown { x, select } => {
                self.category_drag = Some(CategoryDrag {
                    down_x: *x,
                    scroll_at_down: self.category_scroll.scroll_start().as_f32(),
                    moved: false,
                    pending_select: *select,
                });
            }
            ChatSidebarAction::CategoryPointerMove { x } => {
                use warpui_core::units::Pixels;
                let Some(drag) = self.category_drag.as_mut() else {
                    return;
                };
                let dx = *x - drag.down_x;
                if dx.abs() > CATEGORY_DRAG_THRESHOLD_PX {
                    drag.moved = true;
                }
                if drag.moved {
                    self.category_scroll
                        .scroll_to(Pixels::new(drag.scroll_at_down - dx));
                    ctx.notify();
                }
            }
            ChatSidebarAction::CategoryPointerUp => {
                let Some(drag) = self.category_drag.take() else {
                    return;
                };
                if !drag.moved {
                    if let Some(category) = drag.pending_select {
                        self.search_focused = true;
                        self.set_search_category(category, ctx);
                        sync_caret_blink(self, ctx);
                    }
                }
                ctx.notify();
            }
            ChatSidebarAction::SearchEdit(edit) => {
                self.search_field.apply(&mut self.search, edit);
                self.search_focused = true;
                self.refresh_category_hits(ctx);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatSidebarAction::OpenContextMenu { id, x, y } => {
                if self.search_focused {
                    self.blur_search(ctx);
                }
                self.context_menu = Some((id.clone(), *x, *y));
                ctx.notify();
            }
            ChatSidebarAction::CloseContextMenu => {
                self.context_menu = None;
                ctx.notify();
            }
            ChatSidebarAction::ContextOpen => {
                let Some((id, _, _)) = self.context_menu.clone() else {
                    return;
                };
                self.context_menu = None;
                if self.selecting.is_some() {
                    ctx.notify();
                    return;
                }
                match resolve_select_target(&id, &self.conversations, self.cluster.as_ref()) {
                    SelectTarget::Conversation(conv_id) => {
                        self.apply_selection(conv_id, ctx);
                    }
                    SelectTarget::StartWithPeer {
                        peer,
                        display_name,
                        bootstrap_addrs,
                    } => {
                        self.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                    }
                    SelectTarget::MissingChatEndpoint => {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.set_open_error(wormhole_i18n::t("chat.toast.no_chat_endpoint"));
                            state.show_toast(wormhole_i18n::t("chat.toast.no_chat_endpoint"), StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            }
            ChatSidebarAction::ContextToggleMute => {
                let Some((id, _, _)) = self.context_menu.clone() else {
                    return;
                };
                self.context_menu = None;
                let next = !self.ui_prefs.is_muted(&id);
                let core = self.core.clone();
                let mute_id = id.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        let state = runtime.state.clone();
                        set_chat_muted(&state.data_dir, &mute_id, next).await
                    },
                    move |view, output, ctx| match output {
                        Ok(prefs) => {
                            view.apply_ui_prefs(prefs, ctx);
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(
                                    if next {
                                        wormhole_i18n::t("chat.toast.mute_on")
                                    } else {
                                        wormhole_i18n::t("chat.toast.mute_off")
                                    },
                                    StatusTone::Success,
                                );
                            }
                            ctx.notify();
                        }
                        Err(err) => {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(
                                    wormhole_i18n::t_args(
                                        "chat.toast.mute_update_failed",
                                        &[("err", &err.to_string())],
                                    ),
                                    StatusTone::Danger,
                                );
                            }
                            ctx.notify();
                        }
                    },
                );
                ctx.notify();
            }
            ChatSidebarAction::ContextDelete => {
                let Some((id, _, _)) = self.context_menu.clone() else {
                    return;
                };
                self.context_menu = None;
                let core = self.core.clone();
                let hide_id = id.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        let state = runtime.state.clone();
                        set_chat_hidden(&state.data_dir, &hide_id, true).await
                    },
                    move |view, output, ctx| match output {
                        Ok(prefs) => {
                            view.clear_selection_if_matches(&id, ctx);
                            view.apply_ui_prefs(prefs, ctx);
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(wormhole_i18n::t("chat.toast.conversation_deleted"), StatusTone::Muted);
                            }
                            ctx.notify();
                        }
                        Err(err) => {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state
                                    .show_toast(
                                        wormhole_i18n::t_args(
                                            "chat.toast.delete_failed",
                                            &[("err", &err.to_string())],
                                        ),
                                        StatusTone::Danger,
                                    );
                            }
                            ctx.notify();
                        }
                    },
                );
                ctx.notify();
            }
            ChatSidebarAction::ToggleSidebarMenu => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = !state.sidebar_menu_open;
                    state.header_menu_open = false;
                    state.mute_flyout_open = false;
                }
                self.context_menu = None;
                ctx.notify();
            }
            ChatSidebarAction::CloseSidebarMenu => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = false;
                }
                self.menu_hover = None;
                ctx.notify();
            }
            ChatSidebarAction::MenuHover(key) => {
                self.menu_hover = *key;
                ctx.notify();
            }
            ChatSidebarAction::MenuBtnHover(on) => {
                self.menu_btn_hover = *on;
                ctx.notify();
            }
            ChatSidebarAction::MenuNewGroup => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = false;
                }
                self.menu_hover = None;
                self.create_default_cluster_group(ctx);
            }
            ChatSidebarAction::MenuNewChannel => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = false;
                    state.open_channel_create();
                }
                self.menu_hover = None;
                ctx.emit(ChatSidebarEvent::OpenChannel);
                ctx.notify();
            }
            ChatSidebarAction::MenuContacts => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = false;
                    state.open_contacts();
                }
                self.menu_hover = None;
                ctx.emit(ChatSidebarEvent::OpenContacts);
                ctx.notify();
            }
            ChatSidebarAction::MenuCalls => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = false;
                    state.open_calls();
                }
                self.menu_hover = None;
                ctx.emit(ChatSidebarEvent::OpenCalls);
                ctx.notify();
            }
            ChatSidebarAction::MenuFavorites => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.sidebar_menu_open = false;
                    state.favorites_only = !state.favorites_only;
                    let on = state.favorites_only;
                    state.show_toast(
                        if on {
                            wormhole_i18n::t("chat.toast.favorites_only")
                        } else {
                            wormhole_i18n::t("chat.toast.show_all")
                        },
                        StatusTone::Muted,
                    );
                }
                self.menu_hover = None;
                ctx.notify();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectTarget {
    Conversation(String),
    StartWithPeer {
        peer: String,
        display_name: Option<String>,
        bootstrap_addrs: Vec<String>,
    },
    /// Cluster node is known but has no usable chat endpoint yet.
    MissingChatEndpoint,
}

fn resolve_select_target(
    id: &str,
    conversations: &[ChatConversationDto],
    cluster: Option<&ClusterStatusDto>,
) -> SelectTarget {
    if conversations.iter().any(|conv| conv.id == id) {
        return SelectTarget::Conversation(id.to_string());
    }
    if let Some(conv) = conversations.iter().find(|conv| conv.peer_endpoint == id) {
        return SelectTarget::Conversation(conv.id.clone());
    }
    if let Some(cluster) = cluster {
        if let Some(node) = cluster
            .nodes
            .iter()
            .find(|node| node.chat_endpoint_id.as_deref() == Some(id) || node.node_id == id)
        {
            let Some(peer) = node
                .chat_endpoint_id
                .clone()
                .filter(|endpoint| !endpoint.trim().is_empty())
            else {
                return SelectTarget::MissingChatEndpoint;
            };
            let display_name = format!("{} · {}", node.os, node.hostname);
            return SelectTarget::StartWithPeer {
                peer,
                display_name: Some(display_name),
                bootstrap_addrs: node.chat_bootstrap_addrs.clone(),
            };
        }
    }
    SelectTarget::StartWithPeer {
        peer: id.to_string(),
        display_name: None,
        bootstrap_addrs: Vec::new(),
    }
}

/// Resolve an existing DM when cluster topology no longer lists `node_id`
/// (offline prune) but the conversation is already local.
fn find_conversation_for_node_ref(
    conversations: &[ChatConversationDto],
    cluster: Option<&ClusterStatusDto>,
    node_id: &str,
) -> Option<String> {
    if let Some(conv) = conversations
        .iter()
        .find(|conv| conv.peer_endpoint == node_id || conv.id == node_id)
    {
        return Some(conv.id.clone());
    }
    if let Some(cluster) = cluster {
        if let Some(node) = cluster.nodes.iter().find(|node| {
            node.node_id == node_id || node.chat_endpoint_id.as_deref() == Some(node_id)
        }) {
            if let Some(endpoint) = node
                .chat_endpoint_id
                .as_deref()
                .filter(|endpoint| !endpoint.is_empty())
            {
                if let Some(conv) = conversations
                    .iter()
                    .find(|conv| conv.peer_endpoint == endpoint)
                {
                    return Some(conv.id.clone());
                }
            }
            let label = format!("{} · {}", node.os, node.hostname);
            if let Some(conv) = conversations.iter().find(|conv| {
                conv.peer_display_name.as_deref().is_some_and(|name| {
                    let name = name.trim();
                    name.eq_ignore_ascii_case(&label)
                        || name.eq_ignore_ascii_case(&node.hostname)
                        || name.to_lowercase().contains(&node.hostname.to_lowercase())
                })
            }) {
                return Some(conv.id.clone());
            }
        }
    }
    let needle = node_id.to_lowercase();
    conversations.iter().find_map(|conv| {
        conv.peer_display_name.as_deref().and_then(|name| {
            if name.to_lowercase().contains(&needle) {
                Some(conv.id.clone())
            } else {
                None
            }
        })
    })
}

fn conversation_covers_cluster_node(
    conv: &ChatConversationDto,
    node: &wormhole_desktop_core::cluster_commands::ClusterNodeDto,
) -> bool {
    if let Some(endpoint) = node.chat_endpoint_id.as_deref() {
        if !endpoint.is_empty() && conv.peer_endpoint == endpoint {
            return true;
        }
    }
    if conv.peer_endpoint == node.node_id {
        return true;
    }
    let display = conv
        .peer_display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    if let Some(display) = display {
        if node.hostname.eq_ignore_ascii_case(display)
            || format!("{} · {}", node.os, node.hostname).eq_ignore_ascii_case(display)
        {
            return true;
        }
    }
    false
}

impl CaretBlinkHost for ChatSidebarView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_mapping_does_not_treat_unknown_as_offline() {
        assert_eq!(presence_state(true, ""), "online");
        assert_eq!(presence_state(false, "signed_in"), "recently_online");
        assert_eq!(presence_state(false, "offline"), "offline");
        assert_eq!(presence_state(false, "handshake_failed"), "unknown");
    }

    use wormhole_desktop_core::cluster_commands::{ClusterNodeDto, ClusterStatusDto};

    fn sample_conv(id: &str, peer: &str) -> ChatConversationDto {
        ChatConversationDto {
            id: id.into(),
            backend: "wormhole".into(),
            kind: "direct".into(),
            title: Some("Peer".into()),
            cluster_id: None,
            members: Vec::new(),
            created_by: None,
            membership_version: 0,
            peer_endpoint: peer.into(),
            peer_display_name: None,
            peer_bootstrap_addrs: Vec::new(),
            peer_user_id: None,
            contact_conv_id: None,
            description: None,
            avatar_path: None,
            doc_ticket: String::new(),
            created_at: 0,
            last_message_at: None,
            last_message_preview: None,
            unread_count: 0,
        }
    }

    #[test]
    fn resolve_select_target_uses_existing_conv_id() {
        let convs = vec![sample_conv("conv-hash", "peer-endpoint")];
        let target = resolve_select_target("conv-hash", &convs, None);
        assert_eq!(target, SelectTarget::Conversation("conv-hash".to_string()));
    }

    #[test]
    fn resolve_select_target_maps_peer_endpoint_to_conv_id() {
        let convs = vec![sample_conv("conv-hash", "peer-endpoint")];
        let target = resolve_select_target("peer-endpoint", &convs, None);
        assert_eq!(target, SelectTarget::Conversation("conv-hash".to_string()));
    }

    #[test]
    fn resolve_select_target_cluster_node_starts_conversation() {
        let cluster = ClusterStatusDto {
            configured: true,
            cluster_id: None,
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: String::new(),
            transport: String::new(),
            nodes: vec![ClusterNodeDto {
                node_id: "node-1".into(),
                device_id: None,
                chat_endpoint_id: Some("chat-endpoint".into()),
                chat_bootstrap_addrs: vec!["bootstrap-1".into()],
                hostname: "host".into(),
                os: "Windows".into(),
                roles: Vec::new(),
                online: true,
                presence_status: String::new(),
                cpu_cores: 0,
                memory_total: 0,
                storage_total: 0,
                storage_free: 0,
                billing_node_score: None,
                billing_expired: false,
                role: String::new(),
                removable: false,
                revoked: false,
                server_member_confirmed: false,
                same_account: false,
                user_id: None,
                account_display_name: None,
                pending_handshake: false,
                handshake_error: None,
                share_volumes: Vec::new(),
                has_local_share_replicas: false,
            }],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
            role_stale: false,
        };
        let target = resolve_select_target("chat-endpoint", &[], Some(&cluster));
        assert_eq!(
            target,
            SelectTarget::StartWithPeer {
                peer: "chat-endpoint".into(),
                display_name: Some("Windows · host".into()),
                bootstrap_addrs: vec!["bootstrap-1".into()],
            }
        );
    }

    #[test]
    fn resolve_select_target_cluster_node_without_endpoint_is_missing() {
        let cluster = ClusterStatusDto {
            configured: true,
            cluster_id: None,
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: String::new(),
            transport: String::new(),
            nodes: vec![ClusterNodeDto {
                node_id: "node-1".into(),
                device_id: None,
                chat_endpoint_id: None,
                chat_bootstrap_addrs: Vec::new(),
                hostname: "host".into(),
                os: "Windows".into(),
                roles: Vec::new(),
                online: false,
                presence_status: String::new(),
                cpu_cores: 0,
                memory_total: 0,
                storage_total: 0,
                storage_free: 0,
                billing_node_score: None,
                billing_expired: false,
                role: String::new(),
                removable: false,
                revoked: false,
                server_member_confirmed: false,
                same_account: false,
                user_id: None,
                account_display_name: None,
                pending_handshake: false,
                handshake_error: None,
                share_volumes: Vec::new(),
                has_local_share_replicas: false,
            }],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
            role_stale: false,
        };
        let target = resolve_select_target("node-1", &[], Some(&cluster));
        assert_eq!(target, SelectTarget::MissingChatEndpoint);
    }

    #[test]
    fn find_conversation_for_node_ref_matches_endpoint_and_display_name() {
        let mut conv = sample_conv("conv-hash", "chat-endpoint");
        conv.peer_display_name = Some("Windows · DESKTOP-KDSVGM5".into());
        let conversations = vec![conv];
        assert_eq!(
            find_conversation_for_node_ref(&conversations, None, "chat-endpoint").as_deref(),
            Some("conv-hash")
        );
        assert_eq!(
            find_conversation_for_node_ref(&conversations, None, "DESKTOP-KDSVGM5").as_deref(),
            Some("conv-hash")
        );
        let cluster = ClusterStatusDto {
            configured: true,
            cluster_id: None,
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: String::new(),
            transport: String::new(),
            nodes: vec![sample_node("node-1", Some("chat-endpoint"), false)],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
            role_stale: false,
        };
        assert_eq!(
            find_conversation_for_node_ref(&conversations, Some(&cluster), "node-1").as_deref(),
            Some("conv-hash")
        );
    }

    fn sample_node(node_id: &str, endpoint: Option<&str>, local: bool) -> ClusterNodeDto {
        ClusterNodeDto {
            node_id: node_id.into(),
            device_id: None,
            chat_endpoint_id: endpoint.map(str::to_string),
            chat_bootstrap_addrs: Vec::new(),
            hostname: "host".into(),
            os: "Windows".into(),
            roles: Vec::new(),
            online: true,
            presence_status: String::new(),
            cpu_cores: 0,
            memory_total: 0,
            storage_total: 0,
            storage_free: 0,
            billing_node_score: None,
            billing_expired: false,
            role: String::new(),
            removable: false,
            revoked: false,
            server_member_confirmed: false,
            same_account: local,
            user_id: None,
            account_display_name: None,
            pending_handshake: false,
            handshake_error: None,
            share_volumes: Vec::new(),
            has_local_share_replicas: false,
        }
    }

    #[test]
    fn conversation_covers_cluster_node_by_endpoint_or_node_id() {
        let node = sample_node("node-1", Some("ep-1"), false);
        assert!(conversation_covers_cluster_node(
            &sample_conv("c1", "ep-1"),
            &node
        ));
        assert!(conversation_covers_cluster_node(
            &sample_conv("c2", "node-1"),
            &sample_node("node-1", None, false)
        ));
        assert!(!conversation_covers_cluster_node(
            &sample_conv("c3", "other"),
            &node
        ));
    }

    #[test]
    fn merge_placeholders_skip_local_and_existing_peers() {
        let conversations = vec![sample_conv("conv-a", "ep-remote-a")];
        let cluster = ClusterStatusDto {
            configured: true,
            cluster_id: None,
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: "node-local".into(),
            transport: String::new(),
            nodes: vec![
                sample_node("node-local", Some("ep-local"), true),
                sample_node("node-a", Some("ep-remote-a"), false),
                sample_node("node-b", Some("ep-remote-b"), false),
            ],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
            role_stale: false,
        };
        let mut placeholder_ids = Vec::new();
        for node in &cluster.nodes {
            if node.node_id == cluster.local_node_id {
                continue;
            }
            if conversations
                .iter()
                .any(|conv| conversation_covers_cluster_node(conv, node))
            {
                continue;
            }
            placeholder_ids.push(
                node.chat_endpoint_id
                    .clone()
                    .unwrap_or_else(|| node.node_id.clone()),
            );
        }
        assert_eq!(placeholder_ids, vec!["ep-remote-b".to_string()]);
    }

    #[test]
    fn hidden_pref_skips_conversation_row_but_not_cover_check_helper() {
        let prefs = ChatUiPrefs {
            muted: Default::default(),
            hidden: ["conv-win".into()].into_iter().collect(),
            wallpapers: Default::default(),
            pinned: Default::default(),
            media_upload_group: None,
            media_upload_as_file: None,
            last_read_at: Default::default(),
        };
        assert!(prefs.is_hidden("conv-win"));
        let node = sample_node("n-win", Some("ep-win"), false);
        let covers = conversation_covers_cluster_node(&sample_conv("conv-win", "ep-win"), &node);
        assert!(covers);
        // build_rows uses: !is_hidden && covers — so hidden conv must not suppress placeholders.
        assert!(!prefs.is_hidden("ep-win"));
        assert!(
            !(!prefs.is_hidden("conv-win") && covers),
            "hidden conversation must not participate in placeholder suppression"
        );
    }
}
