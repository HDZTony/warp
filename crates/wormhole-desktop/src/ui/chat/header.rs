use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Expanded, Flex, MainAxisSize, ParentElement, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::labels::{
    chat_avatar_for_os, conversation_device_title, conversation_os_label, find_cluster_node,
};
use crate::ui::chat::header_menu::{header_button, header_menu_panel};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{online_dot, tg_avatar, StatusTone, TG_AVATAR_SM_SIZE};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::chat_list_conversations;
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};

pub const TG_HEADER_HEIGHT: f32 = 56.0;
const TG_HEADER_BTN: f32 = 36.0;
const TG_HEADER_PAD_X: f32 = 16.0;
const TG_HEADER_PAD_Y: f32 = 8.0;
const TG_HEADER_INFO_GAP: f32 = 10.0;
const TG_HEADER_ACTION_GAP: f32 = 2.0;

#[derive(Debug, Clone)]
pub enum ChatHeaderAction {
    ToggleThreadSearch,
    ToggleRemoteDesktop,
    ToggleProfile,
    ToggleHeaderMenu,
    ToggleMuteFlyout,
    OpenProfile,
    OpenProfileFromInfo,
    MenuToast(String, StatusTone),
    MuteForever,
    ClearHistory,
    DeleteChat,
}

pub struct ChatHeaderView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    title: String,
    status: String,
    online: bool,
    node_id: String,
    os: String,
    last_selection: Option<String>,
    last_selection_tick: u64,
}

impl ChatHeaderView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let view = Self {
            core,
            selection,
            shell_state,
            font,
            title: "选择左侧终端".into(),
            status: "从列表中选择会话".into(),
            online: false,
            node_id: String::new(),
            os: String::new(),
            last_selection: None,
            last_selection_tick: 0,
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            },
            |view, _, ctx| {
                view.poll(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn poll(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let (selection_tick, pending) = self
            .shell_state
            .lock()
            .map(|state| (state.selection_tick, state.pending_open.clone()))
            .unwrap_or((0, None));
        let selection_changed = selected != self.last_selection;
        let tick_changed = selection_tick != self.last_selection_tick;
        if !selection_changed && !tick_changed {
            return;
        }
        self.last_selection = selected.clone();
        self.last_selection_tick = selection_tick;

        if let Some(pending) = pending {
            self.title = pending.title;
            self.status = "正在打开会话…".into();
            self.online = pending.online;
            self.os = pending.os;
            self.node_id.clear();
            ctx.notify();
            if selected.is_some() {
                // Still resolve once conv_id lands.
                self.refresh_from_selection(ctx);
            }
            return;
        }

        if selected.is_none() {
            self.title = "选择左侧终端".into();
            self.status = "从列表中选择会话".into();
            self.online = false;
            self.node_id.clear();
            self.os.clear();
            ctx.notify();
            return;
        }
        self.refresh_from_selection(ctx);
    }

    fn refresh_from_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(selected) = selected else {
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let conv_id = selected.clone();
                let conversations = chat_list_conversations(app, &state).await;
                let cluster = cluster_status_hud(&state).await;
                let remarks = load_device_remarks(&state.data_dir).await.unwrap_or_default();
                (conv_id, conversations, cluster, remarks)
            },
            |view, output, ctx| {
                let selected = view.selection.lock().ok().and_then(|g| g.clone());
                let Some(selected) = selected else {
                    return;
                };
                let (conv_id, conversations, cluster, remarks) = output;
                if conv_id != selected {
                    return;
                }
                if view
                    .shell_state
                    .lock()
                    .map(|state| state.pending_open.is_some())
                    .unwrap_or(false)
                {
                    return;
                }
                let remote_active = view
                    .shell_state
                    .lock()
                    .map(|state| state.remote_desktop_active)
                    .unwrap_or(false);
                let conv = conversations
                    .ok()
                    .and_then(|list| list.into_iter().find(|conv| conv.id == selected));
                if let Some(conv) = conv {
                    let cluster_ref = cluster.as_ref().ok();
                    let remark = find_cluster_node(&conv, cluster_ref)
                        .and_then(|node| remarks.get(&node.node_id).map(String::as_str));
                    view.title = display_name_with_remark(remark, || {
                        conversation_device_title(&conv, cluster_ref)
                    });
                    view.os = conversation_os_label(&conv, cluster_ref);
                    view.node_id = conv.peer_endpoint.clone();
                    let peer_online = find_cluster_node(&conv, cluster_ref)
                        .map(|node| node.online)
                        .unwrap_or(false);
                    view.online = peer_online;
                    view.status = if remote_active {
                        "远程桌面 · 已连接".into()
                    } else if peer_online {
                        "在线".into()
                    } else {
                        "离线".into()
                    };
                } else if let Ok(cluster) = cluster {
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(selected.as_str())
                            || n.node_id == selected
                    }) {
                        view.title = display_name_with_remark(
                            remarks.get(&node.node_id).map(String::as_str),
                            || format!("{} · {}", node.os, node.hostname),
                        );
                        view.online = node.online;
                        view.node_id = node.node_id.clone();
                        view.os = node.os.clone();
                        view.status = if remote_active {
                            "远程桌面 · 已连接".into()
                        } else if node.online {
                            "在线".into()
                        } else {
                            "离线".into()
                        };
                    } else {
                        view.title = selected.clone();
                        view.node_id = selected.clone();
                        view.os.clear();
                        view.status = if remote_active {
                            "远程桌面 · 已连接".into()
                        } else {
                            "会话信息同步中".into()
                        };
                        view.online = false;
                    }
                } else {
                    view.title = selected.clone();
                    view.node_id = selected.clone();
                    view.os.clear();
                    view.status = if remote_active {
                        "远程桌面 · 已连接".into()
                    } else {
                        "会话信息同步中".into()
                    };
                    view.online = false;
                }
                ctx.notify();
            },
        );
    }

    fn avatar_label(&self) -> String {
        if self.title == "选择左侧终端" {
            return "WH".to_string();
        }
        if !self.os.is_empty() {
            return chat_avatar_for_os(&self.os);
        }
        chat_avatar_for_os("")
    }

    fn shell_flags(&self) -> (bool, bool, bool, bool) {
        self.shell_state
            .lock()
            .map(|state| {
                (
                    state.thread_search_open,
                    state.profile_open,
                    state.remote_desktop_active,
                    state.header_menu_open,
                )
            })
            .unwrap_or((false, false, false, false))
    }
}

impl Entity for ChatHeaderView {
    type Event = ();
}

impl View for ChatHeaderView {
    fn ui_name() -> &'static str {
        "ChatHeaderView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let (search_open, profile_open, rdp_active, menu_open) = self.shell_flags();
        let mute_flyout_open = self
            .shell_state
            .lock()
            .map(|state| state.mute_flyout_open)
            .unwrap_or(false);

        let info_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(tg_avatar(
                self.avatar_label(),
                self.font,
                TG_AVATAR_SM_SIZE,
            ))
            .with_child(
                Container::new({
                    let mut text_col = Flex::column().with_main_axis_size(MainAxisSize::Min);
                    text_col.add_child(
                        ui_text::chat_header_title(self.title.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    );
                    let status_color = if self.online && !rdp_active {
                        theme::success()
                    } else {
                        theme::muted()
                    };
                    text_col.add_child(
                        Container::new(
                            Flex::row()
                                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                                .with_child(if self.online && !rdp_active {
                                    Container::new(online_dot())
                                        .with_horizontal_margin(4.0)
                                        .finish()
                                } else {
                                    Flex::row().finish()
                                })
                                .with_child(
                                    ui_text::chat_header_status(self.status.clone(), self.font)
                                        .with_color(status_color)
                                        .finish(),
                                )
                                .finish(),
                        )
                        .with_margin_top(1.0)
                        .finish(),
                    );
                    text_col.finish()
                })
                .with_margin_left(TG_HEADER_INFO_GAP)
                .finish(),
            )
            .finish();

        let info_clickable = EventHandler::new(
            ConstrainedBox::new(info_row)
                .with_min_height(TG_AVATAR_SM_SIZE)
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::OpenProfileFromInfo);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Min);
        let buttons = [
            (
                "chat-header-search.svg",
                search_open,
                ChatHeaderAction::ToggleThreadSearch,
            ),
            (
                "chat-header-phone.svg",
                false,
                ChatHeaderAction::MenuToast("语音通话（演示）".into(), StatusTone::Muted),
            ),
            (
                "chat-header-rdp.svg",
                rdp_active,
                ChatHeaderAction::ToggleRemoteDesktop,
            ),
            (
                "chat-header-profile.svg",
                profile_open,
                ChatHeaderAction::ToggleProfile,
            ),
            (
                "chat-header-more.svg",
                menu_open,
                ChatHeaderAction::ToggleHeaderMenu,
            ),
        ];
        for (index, (icon, active, action)) in buttons.into_iter().enumerate() {
            actions.add_child(
                Container::new(
                    ConstrainedBox::new(header_button(
                        icon,
                        theme::muted(),
                        active,
                        action,
                        TG_HEADER_BTN,
                    ))
                    .with_width(TG_HEADER_BTN)
                    .with_height(TG_HEADER_BTN)
                    .finish(),
                )
                .with_margin_left(if index == 0 {
                    0.0
                } else {
                    TG_HEADER_ACTION_GAP
                })
                .finish(),
            );
        }

        let header_bar = Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Expanded::new(
                        1.0,
                        Container::new(info_clickable)
                            .with_uniform_padding(4.0)
                            .finish(),
                    )
                    .finish(),
                )
                .with_child(actions.finish())
                .finish(),
        )
        .with_padding_left(TG_HEADER_PAD_X)
        .with_padding_right(TG_HEADER_PAD_X)
        .with_padding_top(TG_HEADER_PAD_Y)
        .with_padding_bottom(TG_HEADER_PAD_Y)
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .with_background(theme::panel())
        .finish();

        if !menu_open {
            return header_bar;
        }

        let mut stack = Stack::new();
        stack.add_child(header_bar);
        stack.add_child(
            Align::new(
                Container::new(header_menu_panel(self.font, mute_flyout_open))
                    .with_margin_top(TG_HEADER_HEIGHT + 6.0)
                    .with_margin_right(TG_HEADER_PAD_X)
                    .finish(),
            )
            .top_right()
            .finish(),
        );
        stack.finish()
    }
}

impl TypedActionView for ChatHeaderView {
    type Action = ChatHeaderAction;

    fn handle_action(&mut self, action: &ChatHeaderAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatHeaderAction::ToggleThreadSearch => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.thread_search_open = !state.thread_search_open;
                    if !state.thread_search_open {
                        state.thread_search_query.clear();
                    }
                    state.close_overlays();
                }
                ctx.notify();
            }
            ChatHeaderAction::ToggleRemoteDesktop => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.remote_desktop_active = !state.remote_desktop_active;
                    state.close_overlays();
                }
                self.refresh_from_selection(ctx);
            }
            ChatHeaderAction::ToggleProfile => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.profile_open = !state.profile_open;
                    state.close_overlays();
                }
                ctx.notify();
            }
            ChatHeaderAction::OpenProfile | ChatHeaderAction::OpenProfileFromInfo => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.profile_open = true;
                    state.header_menu_open = false;
                    state.mute_flyout_open = false;
                }
                ctx.notify();
            }
            ChatHeaderAction::ToggleHeaderMenu => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.header_menu_open = !state.header_menu_open;
                    if !state.header_menu_open {
                        state.mute_flyout_open = false;
                    }
                }
                ctx.notify();
            }
            ChatHeaderAction::ToggleMuteFlyout => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.mute_flyout_open = !state.mute_flyout_open;
                }
                ctx.notify();
            }
            ChatHeaderAction::MenuToast(text, tone) => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(text.clone(), *tone);
                    state.header_menu_open = false;
                    state.mute_flyout_open = false;
                }
                ctx.notify();
            }
            ChatHeaderAction::MuteForever => {
                let selected = self.selection.lock().ok().and_then(|g| g.clone());
                if let Ok(mut state) = self.shell_state.lock() {
                    state.header_menu_open = false;
                    state.mute_flyout_open = false;
                }
                let Some(conv_id) = selected else {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast("请先选择会话", StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                };
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        let state = runtime.state.clone();
                        wormhole_desktop_core::chat_ui_prefs::set_chat_muted(
                            &state.data_dir,
                            &conv_id,
                            true,
                        )
                        .await
                    },
                    |view, output, ctx| match output {
                        Ok(_) => {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast("已开启消息免打扰", StatusTone::Success);
                                state.bump_prefs_tick();
                            }
                            view.status = "已静音".into();
                            view.online = false;
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
            ChatHeaderAction::ClearHistory => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast("历史记录已清空（演示）", StatusTone::Muted);
                    state.header_menu_open = false;
                    state.message_tick += 1;
                }
                ctx.notify();
            }
            ChatHeaderAction::DeleteChat => {
                let selected = self.selection.lock().ok().and_then(|g| g.clone());
                if let Ok(mut state) = self.shell_state.lock() {
                    state.header_menu_open = false;
                }
                let Some(conv_id) = selected else {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast("请先选择会话", StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                };
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        let state = runtime.state.clone();
                        wormhole_desktop_core::chat_ui_prefs::set_chat_hidden(
                            &state.data_dir,
                            &conv_id,
                            true,
                        )
                        .await
                    },
                    |view, output, ctx| match output {
                        Ok(_) => {
                            if let Ok(mut guard) = view.selection.lock() {
                                *guard = None;
                            }
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast("已删除对话", StatusTone::Muted);
                                state.clear_pending_open();
                                state.bump_selection_tick();
                                state.bump_prefs_tick();
                            }
                            view.title = "选择左侧终端".into();
                            view.status = "从列表中选择会话".into();
                            view.online = false;
                            view.node_id.clear();
                            view.os.clear();
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
