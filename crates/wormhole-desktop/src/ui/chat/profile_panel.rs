use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{tg_avatar, ui_title, TG_AVATAR_LG_SIZE};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status;

#[derive(Debug, Clone)]
pub enum ChatProfileAction {
    Call,
    RemoteDesktop,
    BrowseSharedFiles,
}

#[derive(Debug, Clone)]
pub enum ChatProfileEvent {
    BrowseNodeShares(String),
}

pub struct ChatProfilePanelView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    title: String,
    node_id: String,
    status: String,
    online: bool,
    cluster_name: String,
    os_label: String,
}

impl ChatProfilePanelView {
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
            title: String::new(),
            node_id: String::new(),
            status: String::new(),
            online: false,
            cluster_name: String::new(),
            os_label: String::new(),
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            },
            |view, _, ctx| {
                view.refresh(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(selected) = selected else {
            self.title.clear();
            self.node_id.clear();
            ctx.notify();
            return;
        };
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
                    view.cluster_name = cluster
                        .clusters
                        .iter()
                        .find(|c| c.active)
                        .and_then(|c| c.name.clone())
                        .or(cluster.cluster_id.clone())
                        .unwrap_or_else(|| "—".into());
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(selected.as_str())
                            || n.node_id == selected
                    }) {
                        view.title = format!("{} · {}", node.os, node.hostname);
                        view.node_id = node.node_id.clone();
                        view.os_label = node.os.clone();
                        view.online = node.online;
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
                        view.os_label = "—".into();
                        view.online = false;
                        view.status = "会话信息同步中".into();
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
            "WH".into()
        } else {
            compact.to_uppercase()
        }
    }

    fn profile_action(
        &self,
        label: &str,
        icon_path: &'static str,
        action: ChatProfileAction,
    ) -> Box<dyn Element> {
        EventHandler::new(
            Container::new(
                Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(icons::chat_header_icon(icon_path, theme::muted()))
                    .with_child(
                        Container::new(
                            ui_text::chat_sidebar_time(label.to_string(), self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .with_margin_top(6.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(8.0)
            .with_padding_bottom(8.0)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn profile_row(&self, label: &str, value: &str) -> Box<dyn Element> {
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                Align::new(
                    ui_text::mono(value.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .right()
                .finish(),
            )
            .finish()
    }

    fn profile_section(
        &self,
        title: &str,
        rows: Vec<(String, String)>,
    ) -> Box<dyn Element> {
        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(
            Container::new(ui_title(title.to_string(), self.font))
                .with_margin_bottom(8.0)
                .finish(),
        );
        for (label, value) in rows {
            col.add_child(
                Container::new(self.profile_row(&label, &value))
                    .with_vertical_padding(4.0)
                    .finish(),
            );
        }
        Container::new(col.finish())
            .with_padding_left(16.0)
            .with_padding_right(16.0)
            .with_padding_top(12.0)
            .with_padding_bottom(12.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish()
    }
}

impl Entity for ChatProfilePanelView {
    type Event = ChatProfileEvent;
}

impl View for ChatProfilePanelView {
    fn ui_name() -> &'static str {
        "ChatProfilePanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|state| state.profile_open)
            .unwrap_or(false);
        if !open {
            return Flex::column().finish();
        }

        let status_color = if self.online {
            theme::success()
        } else {
            theme::muted()
        };

        let head = Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(tg_avatar(
                    Self::avatar_initials(&self.title),
                    self.font,
                    TG_AVATAR_LG_SIZE,
                ))
                .with_child(
                    Container::new(
                        ui_text::section_title(self.title.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_margin_top(12.0)
                    .with_margin_bottom(4.0)
                    .finish(),
                )
                .with_child(
                    ui_text::chat_sidebar_time(self.node_id.clone(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_child(
                    Container::new(
                        ui_text::body(self.status.clone(), self.font)
                            .with_color(status_color)
                            .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                )
                .finish(),
        )
        .with_padding_left(16.0)
        .with_padding_right(16.0)
        .with_padding_top(20.0)
        .with_padding_bottom(16.0)
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish();

        let actions = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    self.profile_action("通话", "chat-header-phone.svg", ChatProfileAction::Call),
                )
                .with_child(self.profile_action(
                    "远程桌面",
                    "chat-header-rdp.svg",
                    ChatProfileAction::RemoteDesktop,
                ))
                .with_child(self.profile_action(
                    "共享文件",
                    "chat-header-profile.svg",
                    ChatProfileAction::BrowseSharedFiles,
                ))
                .finish(),
        )
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish();

        let cluster_section = self.profile_section(
            "集群",
            vec![
                ("集群".into(), self.cluster_name.clone()),
                ("加密".into(), "E2E · Iroh".into()),
            ],
        );
        let terminal_section = self.profile_section(
            "终端",
            vec![
                ("系统".into(), self.os_label.clone()),
                ("node_id".into(), self.node_id.clone()),
            ],
        );

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(head)
            .with_child(actions)
            .with_child(cluster_section)
            .with_child(terminal_section)
            .finish()
    }
}

impl TypedActionView for ChatProfilePanelView {
    type Action = ChatProfileAction;

    fn handle_action(&mut self, action: &ChatProfileAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatProfileAction::Call => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast("语音通话（演示）", crate::ui::panel_primitives::StatusTone::Muted);
                }
                ctx.notify();
            }
            ChatProfileAction::RemoteDesktop => {
                let toast = if let Ok(mut state) = self.shell_state.lock() {
                    state.remote_desktop_active = !state.remote_desktop_active;
                    if state.remote_desktop_active {
                        "远程桌面 · 已连接（演示）"
                    } else {
                        "远程桌面已断开（演示）"
                    }
                } else {
                    "远程桌面（演示）"
                };
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(toast, crate::ui::panel_primitives::StatusTone::Muted);
                }
                self.refresh(ctx);
            }
            ChatProfileAction::BrowseSharedFiles => {
                if !self.node_id.is_empty() {
                    ctx.emit(ChatProfileEvent::BrowseNodeShares(self.node_id.clone()));
                }
            }
        }
    }
}
