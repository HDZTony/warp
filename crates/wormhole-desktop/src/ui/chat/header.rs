use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Flex, MainAxisAlignment, MainAxisSize, ParentElement, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::header_menu::{header_button, header_menu_panel};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{online_dot, tg_avatar, StatusTone, TG_AVATAR_SM_SIZE};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status;

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
            status: String::new(),
            online: false,
            node_id: String::new(),
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            },
            |view, _, ctx| {
                view.refresh_from_selection(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn refresh_from_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        if selected.is_none() {
            self.title = "选择左侧终端".into();
            self.status = "从列表中选择会话".into();
            self.online = false;
            self.node_id.clear();
            ctx.notify();
            return;
        }
        let selected = selected.unwrap();
        self.title = selected.clone();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status(&state).await
            },
            |view, output, ctx| {
                let selected = view.selection.lock().ok().and_then(|g| g.clone());
                let Some(selected) = selected else {
                    return;
                };
                let remote_active = view
                    .shell_state
                    .lock()
                    .map(|state| state.remote_desktop_active)
                    .unwrap_or(false);
                if let Ok(cluster) = output {
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(selected.as_str())
                            || n.node_id == selected
                    }) {
                        view.title = format!("{} · {}", node.os, node.hostname);
                        view.online = node.online;
                        view.node_id = node.node_id.clone();
                        view.status = if remote_active {
                            "远程桌面 · 已连接".into()
                        } else if node.online {
                            "在线".into()
                        } else {
                            "离线".into()
                        };
                    } else {
                        view.title = "未知设备".into();
                        view.node_id = selected.clone();
                        view.status = if remote_active {
                            "远程桌面 · 已连接".into()
                        } else {
                            "会话信息同步中".into()
                        };
                        view.online = false;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn avatar_initials(title: &str) -> String {
        let compact: String = title
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(2)
            .collect();
        if compact.is_empty() {
            "WH".to_string()
        } else {
            compact.to_uppercase()
        }
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

        let info_clickable = EventHandler::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(tg_avatar(
                    Self::avatar_initials(&self.title),
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
                                        ui_text::chat_header_status(
                                            self.status.clone(),
                                            self.font,
                                        )
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
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Container::new(info_clickable)
                        .with_uniform_padding(4.0)
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
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast("已永久静音此终端通知", StatusTone::Success);
                    state.header_menu_open = false;
                    state.mute_flyout_open = false;
                }
                self.status = "已静音".into();
                self.online = false;
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
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast("聊天已删除（演示 — 侧栏会话保留）", StatusTone::Muted);
                    state.header_menu_open = false;
                    state.message_tick += 1;
                }
                ctx.notify();
            }
        }
    }
}
