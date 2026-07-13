use std::collections::BTreeMap;

use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::labels::chat_avatar_for_os;
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{tg_avatar, ui_title, TG_AVATAR_LG_SIZE};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{chat_set_peer_display_name, SetChatPeerDisplayNameParams};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::{
    display_name_with_remark, load_device_remarks, set_device_remark,
};

const MAX_REMARK_CHARS: usize = 40;

#[derive(Debug, Clone)]
pub enum ChatProfileAction {
    Call,
    RemoteDesktop,
    BrowseSharedFiles,
    FocusRemark,
    RemarkEdit(TextFieldEditAction),
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
    conv_id: Option<String>,
    status: String,
    online: bool,
    cluster_name: String,
    os_label: String,
    default_title: String,
    remark: String,
    remark_field: TextFieldState,
    remark_focused: bool,
    remark_dirty: bool,
    caret_blink: CaretBlink,
    refresh_in_flight: bool,
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
            conv_id: None,
            status: String::new(),
            online: false,
            cluster_name: String::new(),
            os_label: String::new(),
            default_title: String::new(),
            remark: String::new(),
            remark_field: TextFieldState::new(),
            remark_focused: false,
            remark_dirty: false,
            caret_blink: CaretBlink::new(),
            refresh_in_flight: false,
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
            self.conv_id = None;
            self.remark.clear();
            ctx.notify();
            return;
        };
        if self.remark_focused || self.remark_dirty {
            return;
        }
        if self.refresh_in_flight {
            return;
        }
        self.refresh_in_flight = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let conversations =
                    wormhole_desktop_core::chat_commands::chat_list_conversations(app, &state).await;
                let cluster = cluster_status_hud(&state).await;
                let remarks = load_device_remarks(&state.data_dir).await.unwrap_or_default();
                (conversations, cluster, remarks)
            },
            |view, output, ctx| {
                view.refresh_in_flight = false;
                if view.remark_focused || view.remark_dirty {
                    ctx.notify();
                    return;
                }
                let selected = view.selection.lock().ok().and_then(|g| g.clone());
                let Some(selected) = selected else {
                    return;
                };
                let remote_active = view
                    .shell_state
                    .lock()
                    .map(|state| state.remote_desktop_active)
                    .unwrap_or(false);
                let (conversations, cluster, remarks) = output;
                let conv = conversations.ok().and_then(|list| {
                    list.into_iter().find(|conv| conv.id == selected)
                });
                if let Some(ref conv) = conv {
                    view.conv_id = Some(conv.id.clone());
                } else {
                    view.conv_id = Some(selected.clone());
                }
                if let Ok(cluster) = cluster {
                    view.cluster_name = cluster
                        .clusters
                        .iter()
                        .find(|c| c.active)
                        .and_then(|c| c.name.clone())
                        .or(cluster.cluster_id.clone())
                        .unwrap_or_else(|| "—".into());
                    let peer_key = conv
                        .as_ref()
                        .map(|c| c.peer_endpoint.as_str())
                        .unwrap_or(selected.as_str());
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(peer_key)
                            || n.node_id == peer_key
                            || n.node_id == selected
                    }) {
                        view.default_title = format!("{} · {}", node.os, node.hostname);
                        view.node_id = node.node_id.clone();
                        view.os_label = node.os.clone();
                        view.online = node.online;
                        view.remark = remarks
                            .get(&node.node_id)
                            .cloned()
                            .unwrap_or_default();
                        view.title = display_name_with_remark(
                            Some(view.remark.as_str()),
                            || view.default_title.clone(),
                        );
                        view.status = if remote_active {
                            "远程桌面 · 已连接".into()
                        } else if node.online {
                            "在线".into()
                        } else {
                            "离线".into()
                        };
                    } else {
                        view.default_title = conv
                            .as_ref()
                            .and_then(|c| c.title.clone().or(c.peer_display_name.clone()))
                            .unwrap_or_else(|| "未知设备".into());
                        view.title = view.default_title.clone();
                        view.node_id = peer_key.to_string();
                        view.os_label = "—".into();
                        view.online = false;
                        view.remark = remarks
                            .get(&view.node_id)
                            .cloned()
                            .unwrap_or_default();
                        if !view.remark.is_empty() {
                            view.title = view.remark.clone();
                        }
                        view.status = "会话信息同步中".into();
                    }
                }
                ctx.notify();
            },
        );
    }

    fn persist_remark(&mut self, ctx: &mut ViewContext<Self>) {
        if self.node_id.is_empty() {
            return;
        }
        let node_id = self.node_id.clone();
        let conv_id = self.conv_id.clone();
        let remark = self.remark.chars().take(MAX_REMARK_CHARS).collect::<String>();
        let remark_for_save = remark.trim().to_string();
        self.remark = remark_for_save.clone();
        self.title = display_name_with_remark(Some(self.remark.as_str()), || {
            self.default_title.clone()
        });
        self.remark_dirty = false;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let remarks = set_device_remark(
                    &state.data_dir,
                    &node_id,
                    if remark_for_save.is_empty() {
                        None
                    } else {
                        Some(remark_for_save.as_str())
                    },
                )
                .await?;
                if let Some(conv_id) = conv_id {
                    let peer_display_name = if remark_for_save.is_empty() {
                        None
                    } else {
                        Some(remark_for_save.clone())
                    };
                    let _ = chat_set_peer_display_name(
                        &state,
                        SetChatPeerDisplayNameParams {
                            conv_id,
                            peer_display_name,
                        },
                    )
                    .await;
                }
                Ok::<BTreeMap<String, String>, String>(remarks)
            },
            |view, output, ctx| {
                if let Err(e) = output {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            format!("备注保存失败: {e}"),
                            crate::ui::panel_primitives::StatusTone::Danger,
                        );
                    }
                }
                ctx.notify();
            },
        );
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

    fn remark_section(&self) -> Box<dyn Element> {
        let draft = self.remark.clone();
        let marked = self.remark_field.marked_text.clone();
        let border = if self.remark_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };
        let field = render_field_with_caret(
            &draft,
            &marked,
            "用作聊天和终端名",
            self.font,
            self.remark_focused,
            false,
            self.caret_blink.visible,
        );
        let input = wrap_text_field_focus_on_click(
            TextFieldInput::builder(field, |ctx, action| {
                ctx.dispatch_typed_action(ChatProfileAction::RemarkEdit(action));
            })
            .focused(self.remark_focused)
            .ime_preedit(!marked.is_empty())
            .on_keydown(|ctx, keystroke| match keystroke.key.as_str() {
                "enter" | "return" => {
                    ctx.dispatch_typed_action(ChatProfileAction::RemarkEdit(
                        TextFieldEditAction::ClearMarkedText,
                    ));
                    DispatchEventResult::StopPropagation
                }
                "escape" => DispatchEventResult::PropagateToParent,
                _ => DispatchEventResult::PropagateToParent,
            })
            .finish(),
            |ctx| ctx.dispatch_typed_action(ChatProfileAction::FocusRemark),
        );

        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(
            Container::new(ui_title("终端".to_string(), self.font))
                .with_margin_bottom(8.0)
                .finish(),
        );
        let mut remark_row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        remark_row.add_child(
            ui_text::body("备注".to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        remark_row.add_child(
            Container::new(
                ConstrainedBox::new(
                    Container::new(input)
                        .with_padding_left(10.0)
                        .with_padding_right(10.0)
                        .with_padding_top(6.0)
                        .with_padding_bottom(6.0)
                        .with_background(theme::canvas())
                        .with_border(Border::all(1.0).with_border_fill(border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                        .finish(),
                )
                .with_min_width(120.0)
                .finish(),
            )
            .with_margin_left(10.0)
            .finish(),
        );
        col.add_child(
            Container::new(remark_row.finish())
                .with_vertical_padding(4.0)
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::chat_sidebar_time(
                    "留空则使用系统默认名；填写后同步到聊天列表与终端卡片".to_string(),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_padding_top(6.0)
            .with_padding_bottom(4.0)
            .finish(),
        );
        col.add_child(
            Container::new(self.profile_row("系统", &self.os_label))
                .with_vertical_padding(4.0)
                .finish(),
        );
        col.add_child(
            Container::new(self.profile_row("node_id", &self.node_id))
                .with_vertical_padding(4.0)
                .finish(),
        );
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
                    if self.os_label.is_empty() || self.os_label == "—" {
                        chat_avatar_for_os("")
                    } else {
                        chat_avatar_for_os(&self.os_label)
                    },
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
            vec![("集群".into(), self.cluster_name.clone())],
        );

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(head)
            .with_child(actions)
            .with_child(cluster_section)
            .with_child(self.remark_section())
            .finish()
    }
}

impl TypedActionView for ChatProfilePanelView {
    type Action = ChatProfileAction;

    fn handle_action(&mut self, action: &ChatProfileAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatProfileAction::Call => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(
                        "语音通话（演示）",
                        crate::ui::panel_primitives::StatusTone::Muted,
                    );
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
            ChatProfileAction::FocusRemark => {
                self.remark_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatProfileAction::RemarkEdit(edit) => {
                self.remark_field.apply(&mut self.remark, edit);
                if self.remark.chars().count() > MAX_REMARK_CHARS {
                    self.remark = self.remark.chars().take(MAX_REMARK_CHARS).collect();
                }
                self.remark_dirty = true;
                self.title = display_name_with_remark(Some(self.remark.as_str()), || {
                    self.default_title.clone()
                });
                sync_caret_blink(self, ctx);
                self.persist_remark(ctx);
                ctx.notify();
            }
        }
    }
}

impl CaretBlinkHost for ChatProfilePanelView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.remark_focused
    }
}
