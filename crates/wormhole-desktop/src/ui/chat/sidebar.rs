use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use pathfinder_color::ColorU;

use crate::ui::chat::bubble::format_message_time_pub;
use crate::ui::chat::labels::{
    chat_avatar_for_os, conversation_device_title, conversation_os_label, conversation_preview,
    find_cluster_node,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    chat_item_active_bg, chat_search_pill, chat_sidebar_search_bg, positioned_context_menu,
    tg_avatar, StatusTone, HUD_RADIUS,
};
use crate::ui::text_field_input::{
    render_search_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_conversations, chat_start_conversation, ChatConversationDto,
    StartChatConversationParams,
};
use wormhole_desktop_core::chat_ui_prefs::{
    load_chat_ui_prefs, set_chat_hidden, set_chat_muted, ChatUiPrefs,
};
use wormhole_desktop_core::cluster_commands::{cluster_status, ClusterStatusDto};
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};

use std::collections::BTreeMap;

pub const TG_SIDEBAR_AVATAR: f32 = 46.0;
/// Inner content width inside sidebar item horizontal padding (12px × 2 in 300px column).
const CHAT_ITEM_INNER_WIDTH: f32 = 276.0;
const CTX_MENU_WIDTH: f32 = 160.0;

#[derive(Debug, Clone)]
enum ContextItemStyle {
    Normal,
    Accent,
    Danger,
}

#[derive(Debug, Clone)]
pub enum ChatSidebarAction {
    Select(String),
    SearchEdit(TextFieldEditAction),
    FocusSearch,
    ActivateSearch,
    OpenContextMenu { id: String, x: f32, y: f32 },
    CloseContextMenu,
    ContextOpen,
    ContextToggleMute,
    ContextDelete,
}

#[derive(Debug, Clone)]
pub enum ChatSidebarEvent {
    Selected(String),
}

struct SidebarRow {
    id: String,
    title: String,
    preview: String,
    time: String,
    online: bool,
    unread: u32,
    /// OS label for avatar initials (`PC` / `iOS` / …).
    os: String,
    muted: bool,
}

pub struct ChatSidebarView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    rows: Vec<SidebarRow>,
    conversations: Vec<ChatConversationDto>,
    cluster: Option<ClusterStatusDto>,
    remarks: BTreeMap<String, String>,
    ui_prefs: ChatUiPrefs,
    selecting: Option<String>,
    search: String,
    search_field: TextFieldState,
    search_focused: bool,
    caret_blink: CaretBlink,
    status: String,
    scroll: ClippedScrollStateHandle,
    context_menu: Option<(String, f32, f32)>,
    last_prefs_tick: u64,
}

impl ChatSidebarView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            selection,
            shell_state,
            font,
            rows: Vec::new(),
            conversations: Vec::new(),
            cluster: None,
            remarks: BTreeMap::new(),
            ui_prefs: ChatUiPrefs::default(),
            selecting: None,
            search: String::new(),
            search_field: TextFieldState::new(),
            search_focused: false,
            caret_blink: CaretBlink::new(),
            status: String::new(),
            scroll: ClippedScrollStateHandle::new(),
            context_menu: None,
            last_prefs_tick: 0,
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
                load_chat_ui_prefs(&state.data_dir).await.unwrap_or_default()
            },
            |view, prefs, ctx| {
                view.ui_prefs = prefs;
                view.rows = view.build_rows(view.conversations.clone(), view.cluster.as_ref());
                ctx.notify();
            },
        );
    }

    pub(crate) fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let cfg = chat_config(app, &state).await;
                let list = chat_list_conversations(app, &state).await;
                let cluster = cluster_status(&state).await;
                let remarks = load_device_remarks(&state.data_dir).await.unwrap_or_default();
                let ui_prefs = load_chat_ui_prefs(&state.data_dir).await.unwrap_or_default();
                (cfg, list, cluster, remarks, ui_prefs)
            },
            |view, output, ctx| {
                let (cfg, list, cluster, remarks, ui_prefs) = output;
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
                view.rows = view.build_rows(conversations, view.cluster.as_ref());
                ctx.notify();
            },
        );
    }

    fn apply_selection(&mut self, conv_id: String, ctx: &mut ViewContext<Self>) {
        if let Ok(mut guard) = self.selection.lock() {
            *guard = Some(conv_id.clone());
        }
        self.selecting = None;
        if let Ok(mut state) = self.shell_state.lock() {
            state.clear_pending_open();
            state.clear_open_error();
            state.bump_selection_tick();
        }
        ctx.emit(ChatSidebarEvent::Selected(conv_id));
        ctx.notify();
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
            .unwrap_or_else(|| "打开会话…".into());
        let (os, online) = self
            .cluster
            .as_ref()
            .and_then(|cluster| {
                cluster.nodes.iter().find_map(|node| {
                    let endpoint = node.chat_endpoint_id.as_deref().unwrap_or(node.node_id.as_str());
                    if endpoint == peer.as_str() || node.node_id == peer {
                        Some((node.os.clone(), node.online))
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| (String::new(), false));
        if let Ok(mut state) = self.shell_state.lock() {
            state.set_pending_open(title, os, online);
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
                            let message = format!("无法开始会话: {err}");
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
        self.conversations.iter().any(|conv| {
            conv.id == selected && (conv.peer_endpoint == row.id || conv.id == row.id)
        })
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
            let remark = find_cluster_node(conv, cluster)
                .and_then(|node| self.remarks.get(&node.node_id).map(String::as_str));
            let title = display_name_with_remark(remark, || {
                conversation_device_title(conv, cluster)
            });
            let preview = conversation_preview(conv, cluster);
            let time = conv
                .last_message_at
                .map(format_message_time_pub)
                .unwrap_or_default();
            let online = find_cluster_node(conv, cluster)
                .map(|node| node.online)
                .unwrap_or(true);
            rows.push(SidebarRow {
                id: conv.id.clone(),
                title,
                preview,
                time,
                online,
                unread: 0,
                os: conversation_os_label(conv, cluster),
                muted: self.ui_prefs.is_muted(&conv.id),
            });
        }
        if let Some(cluster) = cluster {
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
                        "在线 · 等待消息…".into()
                    } else {
                        "离线".into()
                    },
                    time: String::new(),
                    online: node.online,
                    unread: 0,
                    os: node.os.clone(),
                    muted: self.ui_prefs.is_muted(&id),
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
                    state.show_toast("无法给本机发信息", StatusTone::Danger);
                }
                ctx.notify();
                return;
            }
            if let Some(node) = cluster.nodes.iter().find(|node| node.node_id == node_id) {
                let Some(peer) = node.chat_endpoint_id.clone().filter(|id| !id.trim().is_empty())
                else {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast("该终端尚无聊天地址", StatusTone::Danger);
                    }
                    ctx.notify();
                    return;
                };
                if let Some(conv) = self
                    .conversations
                    .iter()
                    .find(|conv| conv.peer_endpoint == peer)
                {
                    self.apply_selection(conv.id.clone(), ctx);
                    return;
                }
                let display_name = Some(format!("{} · {}", node.os, node.hostname));
                let bootstrap_addrs = node.chat_bootstrap_addrs.clone();
                self.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                return;
            }
        }
        // Cluster cache miss: refresh then open.
        let core = self.core.clone();
        self.selecting = Some(node_id.clone());
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let list = chat_list_conversations(app, &state).await;
                let cluster = cluster_status(&state).await?;
                Ok::<_, String>((list.unwrap_or_default(), cluster, node_id))
            },
            |view, output, ctx| {
                view.selecting = None;
                match output {
                    Ok((conversations, cluster, node_id)) => {
                        view.conversations = conversations.clone();
                        view.cluster = Some(cluster.clone());
                        view.rows = view.build_rows(conversations, Some(&cluster));
                        if node_id == cluster.local_node_id {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast("无法给本机发信息", StatusTone::Danger);
                            }
                            ctx.notify();
                            return;
                        }
                        let Some(node) = cluster.nodes.iter().find(|node| node.node_id == node_id)
                        else {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast("未找到该终端", StatusTone::Danger);
                            }
                            ctx.notify();
                            return;
                        };
                        let Some(peer) =
                            node.chat_endpoint_id.clone().filter(|id| !id.trim().is_empty())
                        else {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast("该终端尚无聊天地址", StatusTone::Danger);
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
                            state.show_toast(format!("无法打开聊天: {err}"), StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn filtered_rows(&self) -> Vec<&SidebarRow> {
        let q = self.search.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|row| {
                q.is_empty()
                    || row.title.to_lowercase().contains(&q)
                    || row.preview.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn chat_item(&self, row: &SidebarRow, selected: bool) -> Box<dyn Element> {
        let id = row.id.clone();
        let menu_id = row.id.clone();
        let title_color = if row.muted {
            theme::muted()
        } else {
            theme::text()
        };
        let mut top = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        top.add_child(
            ui_text::chat_sidebar_name(row.title.clone(), self.font)
                .with_color(title_color)
                .finish(),
        );
        if !row.time.is_empty() {
            top.add_child(
                ui_text::chat_sidebar_time(row.time.clone(), self.font)
                    .with_color(theme::muted())
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
            .with_main_axis_size(MainAxisSize::Min);
        let preview_text = if row.muted && !row.preview.is_empty() {
            format!("🔇 {}", row.preview)
        } else {
            row.preview.clone()
        };
        preview_row.add_child(
            ui_text::chat_preview(preview_text, self.font)
                .with_color(preview_color)
                .finish(),
        );
        if row.unread > 0 && !row.muted {
            preview_row.add_child(
                Container::new(
                    Align::new(
                        ui_text::device_meta(row.unread.to_string(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .with_uniform_padding(4.0)
                .with_background(theme::accent_cool())
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

        let row_body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(tg_avatar(
                chat_avatar_for_os(&row.os),
                self.font,
                TG_SIDEBAR_AVATAR,
            ))
            .with_child(
                Container::new(col.finish())
                    .with_margin_left(10.0)
                    .finish(),
            )
            .finish();

        let interactive = EventHandler::new(
            ConstrainedBox::new(row_body)
                .with_width(CHAT_ITEM_INNER_WIDTH)
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(ChatSidebarAction::Select(id.clone()));
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
        let field = render_search_field_with_caret(
            &draft,
            &marked,
            "搜索终端…",
            self.font,
            search_focused,
            false,
            self.caret_blink.visible,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ChatSidebarAction::SearchEdit(action));
        })
        .focused(search_focused)
        .ime_preedit(!marked.is_empty())
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "tab" || keystroke.key == "escape" {
                ctx.dispatch_typed_action(ChatSidebarAction::FocusSearch);
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ChatSidebarAction::ActivateSearch);
        });

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

        chat_search_pill(
            row,
            chat_sidebar_search_bg(),
            border_color,
            7.0,
            10.0,
            999.0,
        )
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
        label: &str,
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
        let handler = EventHandler::new(
            Container::new(
                ui_text::body(label.to_string(), self.font)
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
        let can_delete = self.rows.len() > 1;
        let mut menu = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        menu.add_child(self.context_menu_item(
            "打开对话",
            ChatSidebarAction::ContextOpen,
            ContextItemStyle::Normal,
            true,
        ));
        menu.add_child(self.context_menu_item(
            if muted {
                "取消免打扰"
            } else {
                "消息免打扰"
            },
            ChatSidebarAction::ContextToggleMute,
            ContextItemStyle::Accent,
            true,
        ));
        menu.add_child(Self::context_menu_divider());
        menu.add_child(self.context_menu_item(
            "删除对话",
            ChatSidebarAction::ContextDelete,
            ContextItemStyle::Danger,
            can_delete,
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
                        if self.search.is_empty() {
                            if self.status.is_empty() {
                                "暂无会话".to_string()
                            } else {
                                format!("暂无会话 · {}", self.status)
                            }
                        } else {
                            "无匹配结果".to_string()
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
            for row in rows {
                let active = self.row_is_active(row, selected.as_deref());
                list.add_child(self.chat_item(row, active));
            }
        }

        let body = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .with_child(
                            Expanded::new(
                                1.0,
                                ConstrainedBox::new(self.search_box())
                                    .with_width(CHAT_ITEM_INNER_WIDTH)
                                    .finish(),
                            )
                            .finish(),
                        )
                        .finish(),
                )
                .with_horizontal_padding(12.0)
                .with_vertical_padding(10.0)
                .with_background(theme::panel())
                .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.0,
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
                .finish(),
            )
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
                            state.set_open_error("该终端尚无聊天地址");
                            state.show_toast("该终端尚无聊天地址", StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            }
            ChatSidebarAction::FocusSearch => {
                self.search_focused = !self.search_focused;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatSidebarAction::ActivateSearch => {
                if !self.search_focused {
                    self.search_focused = true;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
            ChatSidebarAction::SearchEdit(edit) => {
                self.search_field.apply(&mut self.search, edit);
                self.search_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatSidebarAction::OpenContextMenu { id, x, y } => {
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
                            state.set_open_error("该终端尚无聊天地址");
                            state.show_toast("该终端尚无聊天地址", StatusTone::Danger);
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
                                        "已开启消息免打扰"
                                    } else {
                                        "已取消免打扰"
                                    },
                                    StatusTone::Success,
                                );
                            }
                            ctx.notify();
                        }
                        Err(err) => {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(
                                    format!("无法更新免打扰: {err}"),
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
                if self.rows.len() <= 1 {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast("至少保留一个对话", StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                }
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
                                state.show_toast("已删除对话", StatusTone::Muted);
                            }
                            ctx.notify();
                        }
                        Err(err) => {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(
                                    format!("无法删除对话: {err}"),
                                    StatusTone::Danger,
                                );
                            }
                            ctx.notify();
                        }
                    },
                );
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
    if let Some(conv) = conversations
        .iter()
        .find(|conv| conv.peer_endpoint == id)
    {
        return SelectTarget::Conversation(conv.id.clone());
    }
    if let Some(cluster) = cluster {
        if let Some(node) = cluster.nodes.iter().find(|node| {
            node.chat_endpoint_id.as_deref() == Some(id) || node.node_id == id
        }) {
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
            doc_ticket: String::new(),
            created_at: 0,
            last_message_at: None,
            last_message_preview: None,
        }
    }

    #[test]
    fn resolve_select_target_uses_existing_conv_id() {
        let convs = vec![sample_conv("conv-hash", "peer-endpoint")];
        let target = resolve_select_target("conv-hash", &convs, None);
        assert_eq!(
            target,
            SelectTarget::Conversation("conv-hash".to_string())
        );
    }

    #[test]
    fn resolve_select_target_maps_peer_endpoint_to_conv_id() {
        let convs = vec![sample_conv("conv-hash", "peer-endpoint")];
        let target = resolve_select_target("peer-endpoint", &convs, None);
        assert_eq!(
            target,
            SelectTarget::Conversation("conv-hash".to_string())
        );
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
}
