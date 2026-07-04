use std::sync::{Arc, Mutex};

use pathfinder_geometry::vector::vec2f;
use warpui::elements::{
    Border, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Empty, EventHandler, Expanded, Flex, MainAxisSize, OffsetPositioning,
    ParentAnchor, ParentElement, ParentOffsetBounds, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext, ViewHandle};

use crate::coordinator::{CoordinatorState, UiCommand};
use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::header::{ChatHeaderEvent, ChatHeaderView, TG_HEADER_HEIGHT};
use crate::ui::chat::sidebar::ChatSidebarView;
use crate::ui::chat::thread::ChatThreadView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{load_device_gate, wrap_with_device_gate, DeviceGateStatus};
use crate::ui::panel_primitives::tab_content_fill;
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

pub const SIDEBAR_WIDTH: f32 = 300.0;
pub const PROFILE_PANEL_WIDTH: f32 = 280.0;

pub type ConversationSelection = Arc<Mutex<Option<String>>>;

#[derive(Default, Clone)]
pub struct ChatShellUiState {
    pub search_open: bool,
    pub search_query: String,
    pub profile_open: bool,
    pub more_menu_open: bool,
    pub mute_submenu_open: bool,
    pub rdp_connected: bool,
    pub rdp_peer: Option<String>,
    pub muted: bool,
    pub peer_node_id: Option<String>,
    pub peer_os: String,
    pub peer_hostname: String,
    pub peer_online: bool,
    pub toast: String,
}

#[derive(Debug, Clone)]
pub enum ChatShellEvent {
    NavigateToDevices { node_id: String },
}

#[derive(Debug, Clone)]
pub enum ChatShellAction {
    CloseOverlays,
    ToggleMuteSubmenu,
    SetMuted(bool),
    MuteRingtone,
    MuteDisableSound,
    MuteDuration,
    MuteForever,
    ViewProfile,
    SetWallpaper,
    DisableSharing,
    ClearHistory,
    DeleteChat,
    SearchEdit(TextFieldEditAction),
    FocusSearch,
    ProfileRemoteDesktop,
    ProfileVoiceCall,
    ProfileOpenSharedFiles,
}

pub struct ChatShellView {
    core: CoreHandle,
    coordinator: Arc<Mutex<CoordinatorState>>,
    font: FamilyId,
    mono: FamilyId,
    gate: DeviceGateStatus,
    selection: ConversationSelection,
    ui_state: Arc<Mutex<ChatShellUiState>>,
    search_field: TextFieldState,
    search_focused: bool,
    caret_blink: CaretBlink,
    sidebar: ViewHandle<ChatSidebarView>,
    header: ViewHandle<ChatHeaderView>,
    thread: ViewHandle<ChatThreadView>,
    compose: ViewHandle<ChatComposeView>,
}

impl ChatShellView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: Arc<Mutex<CoordinatorState>>,
    ) -> Self {
        let selection = Arc::new(Mutex::new(None));
        let ui_state = Arc::new(Mutex::new(ChatShellUiState::default()));
        let sidebar = ctx.add_typed_action_view(|ctx| {
            ChatSidebarView::new(
                ctx,
                core.clone(),
                selection.clone(),
                ui_state.clone(),
            )
        });
        ctx.subscribe_to_view(&sidebar, |view, _, event, ctx| {
            view.handle_sidebar_event(event, ctx);
        });
        let header = ctx.add_typed_action_view(|ctx| {
            ChatHeaderView::new(
                ctx,
                core.clone(),
                selection.clone(),
                ui_state.clone(),
            )
        });
        let thread = ctx.add_view(|ctx| ChatThreadView::new(ctx, core.clone(), selection.clone()));
        let compose = ctx.add_typed_action_view(|ctx| {
            ChatComposeView::new(ctx, core.clone(), selection.clone())
        });
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);

        ctx.subscribe_to_view(&header, |view, _, event, ctx| {
            view.handle_header_event(event, ctx);
        });

        let mut view = Self {
            core: core.clone(),
            coordinator,
            font,
            mono,
            gate: DeviceGateStatus::default(),
            selection,
            ui_state,
            search_field: TextFieldState::new(),
            search_focused: false,
            caret_blink: CaretBlink::new(),
            sidebar,
            header,
            thread,
            compose,
        };
        view.poll_gate(ctx);
        view
    }

    fn handle_sidebar_event(
        &mut self,
        event: &crate::ui::chat::sidebar::ChatSidebarEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let crate::ui::chat::sidebar::ChatSidebarEvent::Selected(_) = event {
            if let Ok(mut ui) = self.ui_state.lock() {
                ui.rdp_connected = false;
                ui.rdp_peer = None;
            }
            ctx.notify();
        }
    }

    fn handle_header_event(&mut self, event: &ChatHeaderEvent, ctx: &mut ViewContext<Self>) {
        match event {
            ChatHeaderEvent::ToggleInThreadSearch => {
                {
                    if let Ok(mut ui) = self.ui_state.lock() {
                        ui.search_open = !ui.search_open;
                        if !ui.search_open {
                            ui.search_query.clear();
                            self.search_field.clear_marked();
                        }
                        ui.more_menu_open = false;
                        ui.mute_submenu_open = false;
                    }
                }
                let search_open = self
                    .ui_state
                    .lock()
                    .map(|u| u.search_open)
                    .unwrap_or(false);
                self.sync_thread_filter(ctx);
                if search_open {
                    self.search_focused = true;
                    sync_caret_blink(self, ctx);
                }
                ctx.notify();
            }
            ChatHeaderEvent::VoiceCall => {
                self.set_toast("语音通话（演示）");
                ctx.notify();
            }
            ChatHeaderEvent::ToggleRemoteDesktop => {
                self.toggle_remote_desktop(ctx);
            }
            ChatHeaderEvent::ToggleProfilePanel => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.profile_open = !ui.profile_open;
                    ui.more_menu_open = false;
                    ui.mute_submenu_open = false;
                }
                ctx.notify();
            }
            ChatHeaderEvent::ToggleMoreMenu => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.more_menu_open = !ui.more_menu_open;
                    ui.mute_submenu_open = false;
                    if ui.more_menu_open {
                        ui.profile_open = false;
                    }
                }
                ctx.notify();
            }
            ChatHeaderEvent::OpenProfileFromAvatar => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.profile_open = true;
                    ui.more_menu_open = false;
                }
                ctx.notify();
            }
        }
    }

    fn toggle_remote_desktop(&mut self, ctx: &mut ViewContext<Self>) {
        let (peer, title, currently_connected) = {
            let ui = match self.ui_state.lock() {
                Ok(u) => u,
                Err(_) => return,
            };
            (
                ui.peer_node_id.clone(),
                format!("{} · {}", ui.peer_os, ui.peer_hostname),
                ui.rdp_connected,
            )
        };
        let Some(peer) = peer else {
            self.set_toast("请先选择在线终端");
            ctx.notify();
            return;
        };
        if currently_connected {
            let window_key = crate::wormhole_native_ipc::rdp_window_key(&peer);
            if let Ok(mut guard) = self.coordinator.lock() {
                guard.enqueue(UiCommand::CloseRdp { window_key });
            }
            if let Ok(mut ui) = self.ui_state.lock() {
                ui.rdp_connected = false;
                ui.rdp_peer = None;
                ui.toast = format!("已关闭 {title} 的远程桌面窗口");
            }
            ctx.notify();
            return;
        }
        let window_key = crate::wormhole_native_ipc::rdp_window_key(&peer);
        let rdp_title = format!("RDP · {peer}");
        if let Ok(mut guard) = self.coordinator.lock() {
            guard.enqueue(UiCommand::OpenRdp {
                peer: peer.clone(),
                title: rdp_title,
                reconnect: false,
                window_key,
                password: None,
                totp_code: None,
                fps: 60,
            });
        }
        if let Ok(mut ui) = self.ui_state.lock() {
            ui.rdp_connected = true;
            ui.rdp_peer = Some(peer);
            ui.toast = format!("正在连接 {title} 的远程桌面…");
        }
        let thread = self.thread.clone();
        ctx.update_view(&thread, |thread, ctx| {
            thread.push_system_message(format!("正在连接 {title} 的远程桌面…"), ctx);
        });
        ctx.notify();
    }

    pub fn on_rdp_window_closed(&mut self, window_key: &str, ctx: &mut ViewContext<Self>) {
        let mut changed = false;
        if let Ok(mut ui) = self.ui_state.lock() {
            if ui.rdp_connected {
                if let Some(ref peer) = ui.rdp_peer {
                    if crate::wormhole_native_ipc::rdp_window_key(peer) == window_key {
                        ui.rdp_connected = false;
                        ui.rdp_peer = None;
                        ui.toast = "远程桌面窗口已关闭".into();
                        changed = true;
                    }
                }
            }
        }
        if changed {
            ctx.notify();
        }
    }

    fn set_toast(&mut self, message: &str) {
        if let Ok(mut ui) = self.ui_state.lock() {
            ui.toast = message.to_string();
        }
    }

    fn close_overlays(&mut self, ctx: &mut ViewContext<Self>) {
        let had_search = {
            if let Ok(mut ui) = self.ui_state.lock() {
                ui.more_menu_open = false;
                ui.mute_submenu_open = false;
                ui.profile_open = false;
                let had = ui.search_open;
                if ui.search_open {
                    ui.search_open = false;
                    ui.search_query.clear();
                    self.search_field.clear_marked();
                }
                had
            } else {
                false
            }
        };
        if had_search {
            self.sync_thread_filter(ctx);
        }
        self.search_focused = false;
        ctx.notify();
    }

    fn sync_thread_filter(&mut self, ctx: &mut ViewContext<Self>) {
        let query = self
            .ui_state
            .lock()
            .map(|u| u.search_query.clone())
            .unwrap_or_default();
        let thread = self.thread.clone();
        ctx.update_view(&thread, |thread, ctx| {
            thread.set_search_filter(query, ctx);
        });
    }

    pub fn poll_gate(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let apply: Arc<dyn Fn(&mut Self, DeviceGateStatus) + Send + Sync> =
            Arc::new(|view, gate| {
                view.gate = gate;
            });
        load_device_gate(core, ctx, apply);
    }
}

impl Entity for ChatShellView {
    type Event = ChatShellEvent;
}

impl View for ChatShellView {
    fn ui_name() -> &'static str {
        "ChatShellView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        wrap_with_device_gate(self.font, "P2P 聊天", &self.gate, self.chat_stack())
    }
}

impl ChatShellView {
    fn chat_stack(&self) -> Box<dyn Element> {
        let ui = self.ui_state.lock().ok();
        let more_open = ui.as_ref().map(|u| u.more_menu_open).unwrap_or(false);
        let mute_sub = ui.as_ref().map(|u| u.mute_submenu_open).unwrap_or(false);

        let mut stack = Stack::new();
        stack.add_child(self.chat_body());
        if more_open {
            stack.add_positioned_child(
                self.more_menu_panel(mute_sub),
                OffsetPositioning::offset_from_parent(
                    vec2f(-16.0, TG_HEADER_HEIGHT + 4.0),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        EventHandler::new(stack.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                if more_open {
                    ctx.dispatch_typed_action(ChatShellAction::CloseOverlays);
                    return DispatchEventResult::StopPropagation;
                }
                DispatchEventResult::PropagateToParent
            })
            .on_keydown(move |ctx, _, keystroke| {
                if keystroke.key.as_str() != "escape" {
                    return DispatchEventResult::PropagateToParent;
                }
                ctx.dispatch_typed_action(ChatShellAction::CloseOverlays);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn chat_body(&self) -> Box<dyn Element> {
        let ui = self.ui_state.lock().ok();
        let search_open = ui.as_ref().map(|u| u.search_open).unwrap_or(false);
        let profile_open = ui.as_ref().map(|u| u.profile_open).unwrap_or(false);

        let mut main_col = Flex::column().with_main_axis_size(MainAxisSize::Max);
        main_col.add_child(
            ConstrainedBox::new(ChildView::new(&self.header).finish())
                .with_height(TG_HEADER_HEIGHT)
                .finish(),
        );
        if search_open {
            main_col.add_child(self.in_thread_search_bar());
        }
        if let Some(toast) = ui
            .as_ref()
            .map(|u| u.toast.clone())
            .filter(|t| !t.is_empty())
        {
            main_col.add_child(self.toast_strip(toast));
        }
        main_col.add_child(
            Expanded::new(1.0, ChildView::new(&self.thread).finish())
                .finish(),
        );
        main_col.add_child(ChildView::new(&self.compose).finish());

        let mut chat_main = Flex::row().with_main_axis_size(MainAxisSize::Max);
        chat_main.add_child(
            Expanded::new(
                1.0,
                Container::new(main_col.finish())
                    .with_background(theme::canvas())
                    .finish(),
            )
            .finish(),
        );
        if profile_open {
            chat_main.add_child(
                ConstrainedBox::new(self.profile_panel())
                    .with_width(PROFILE_PANEL_WIDTH)
                    .finish(),
            );
        }

        tab_content_fill(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    ConstrainedBox::new(ChildView::new(&self.sidebar).finish())
                        .with_width(SIDEBAR_WIDTH)
                        .finish(),
                )
                .with_child(
                    Expanded::new(
                        1.0,
                        Container::new(chat_main.finish())
                            .with_border(Border::left(1.0).with_border_fill(theme::border()))
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
    }

    fn toast_strip(&self, message: String) -> Box<dyn Element> {
        Container::new(
            ui_text::body(message, self.font)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_cool_bg(24))
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn in_thread_search_bar(&self) -> Box<dyn Element> {
        let draft = self.search_query();
        let marked = self.search_field.marked_text.clone();
        let field = render_field_with_caret(
            &draft,
            &marked,
            "搜索对话内容…",
            self.font,
            self.search_focused,
            false,
            self.caret_blink.visible,
        );
        let input = wrap_text_field_focus_on_click(
            TextFieldInput::builder(field, |ctx, action| {
                ctx.dispatch_typed_action(ChatShellAction::SearchEdit(action));
            })
            .focused(self.search_focused)
            .ime_preedit(!marked.is_empty())
            .on_keydown(|ctx, keystroke| {
                if keystroke.key == "escape" {
                    ctx.dispatch_typed_action(ChatShellAction::CloseOverlays);
                    return DispatchEventResult::StopPropagation;
                }
                DispatchEventResult::PropagateToParent
            })
            .finish(),
            |ctx| ctx.dispatch_typed_action(ChatShellAction::FocusSearch),
        );

        Container::new(input)
            .with_uniform_padding(10.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn search_query(&self) -> String {
        self.ui_state
            .lock()
            .map(|u| u.search_query.clone())
            .unwrap_or_default()
    }

    fn profile_panel(&self) -> Box<dyn Element> {
        let ui = self.ui_state.lock().ok();
        let node_id = ui
            .as_ref()
            .and_then(|u| u.peer_node_id.clone())
            .unwrap_or_else(|| "—".to_string());
        let title = ui
            .as_ref()
            .map(|u| format!("{} · {}", u.peer_os, u.peer_hostname))
            .unwrap_or_else(|| "未选择终端".to_string());
        let online = ui.as_ref().map(|u| u.peer_online).unwrap_or(false);
        let status = if online { "在线" } else { "离线" };

        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        col.add_child(
            Container::new(
                ui_text::body("终端资料", self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(16.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish(),
        );
        col.add_child(
            Container::new(
                Flex::column()
                    .with_child(
                        ui_text::body(title, self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_child(
                        Container::new(
                            ui_text::mono(node_id, self.mono)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .with_margin_top(8.0)
                        .finish(),
                    )
                    .with_child(
                        Container::new(
                            ui_text::body(status.to_string(), self.font)
                                .with_color(if online {
                                    theme::success()
                                } else {
                                    theme::muted()
                                })
                                .finish(),
                        )
                        .with_margin_top(6.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_uniform_padding(16.0)
            .finish(),
        );

        let action_row = |label: &str, action: ChatShellAction| {
            EventHandler::new(
                Container::new(
                    ui_text::body(label.to_string(), self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_uniform_padding(12.0)
                .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish()
        };

        col.add_child(action_row("语音通话", ChatShellAction::ProfileVoiceCall));
        col.add_child(action_row(
            "远程桌面",
            ChatShellAction::ProfileRemoteDesktop,
        ));
        col.add_child(action_row(
            "共享文件",
            ChatShellAction::ProfileOpenSharedFiles,
        ));

        Container::new(col.finish())
            .with_background(theme::panel())
            .with_border(Border::left(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn more_menu_panel(&self, mute_submenu_open: bool) -> Box<dyn Element> {
        let mut menu = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        menu.add_child(self.more_menu_row(
            "消息免打扰",
            ChatShellAction::ToggleMuteSubmenu,
            false,
            true,
        ));
        if mute_submenu_open {
            menu.add_child(self.more_menu_row(
                "设置铃声",
                ChatShellAction::MuteRingtone,
                true,
                false,
            ));
            menu.add_child(self.more_menu_row(
                "关闭通知音",
                ChatShellAction::MuteDisableSound,
                true,
                false,
            ));
            menu.add_child(self.more_menu_row(
                "静音 1 小时",
                ChatShellAction::MuteDuration,
                true,
                false,
            ));
            menu.add_child(self.more_menu_row(
                "永久静音",
                ChatShellAction::MuteForever,
                true,
                false,
            ));
        }
        menu.add_child(self.more_menu_row(
            "查看个人资料",
            ChatShellAction::ViewProfile,
            false,
            false,
        ));
        menu.add_child(self.more_menu_row(
            "设置壁纸",
            ChatShellAction::SetWallpaper,
            false,
            false,
        ));
        menu.add_child(self.more_menu_row(
            "禁用分享",
            ChatShellAction::DisableSharing,
            false,
            false,
        ));
        menu.add_child(
            ConstrainedBox::new(Empty::new().finish())
                .with_height(1.0)
                .finish(),
        );
        menu.add_child(self.more_menu_row(
            "清空历史记录",
            ChatShellAction::ClearHistory,
            false,
            false,
        ));
        menu.add_child(self.more_menu_row(
            "删除聊天",
            ChatShellAction::DeleteChat,
            false,
            false,
        ));

        Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(200.0)
                .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
        .finish()
    }

    fn more_menu_row(
        &self,
        label: &str,
        action: ChatShellAction,
        indent: bool,
        has_flyout: bool,
    ) -> Box<dyn Element> {
        let danger = label == "删除聊天";
        let color = if danger {
            theme::danger()
        } else {
            theme::text()
        };
        let label_text = if has_flyout {
            format!("{label}  ›")
        } else {
            label.to_string()
        };
        EventHandler::new(
            Container::new(
                ui_text::body(label_text, self.font)
                    .with_color(color)
                    .finish(),
            )
            .with_padding_left(if indent { 24.0 } else { 12.0 })
            .with_padding_right(12.0)
            .with_padding_top(10.0)
            .with_padding_bottom(10.0)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }
}

impl TypedActionView for ChatShellView {
    type Action = ChatShellAction;

    fn handle_action(&mut self, action: &ChatShellAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatShellAction::CloseOverlays => self.close_overlays(ctx),
            ChatShellAction::ToggleMuteSubmenu => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.mute_submenu_open = !ui.mute_submenu_open;
                }
                ctx.notify();
            }
            ChatShellAction::SetMuted(value) => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.muted = *value;
                    ui.mute_submenu_open = false;
                    ui.more_menu_open = false;
                    ui.toast = if *value {
                        "已开启消息免打扰".into()
                    } else {
                        "已关闭消息免打扰".into()
                    };
                }
                ctx.notify();
            }
            ChatShellAction::MuteRingtone => {
                self.set_toast("设置铃声（即将支持）");
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.mute_submenu_open = false;
                    ui.more_menu_open = false;
                }
                ctx.notify();
            }
            ChatShellAction::MuteDisableSound => {
                self.handle_action(&ChatShellAction::SetMuted(true), ctx);
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.toast = "已关闭通知音".into();
                }
                ctx.notify();
            }
            ChatShellAction::MuteDuration => {
                self.handle_action(&ChatShellAction::SetMuted(true), ctx);
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.toast = "已静音 1 小时".into();
                }
                ctx.notify();
            }
            ChatShellAction::MuteForever => {
                self.handle_action(&ChatShellAction::SetMuted(true), ctx);
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.toast = "已永久静音此会话".into();
                }
                ctx.notify();
            }
            ChatShellAction::ViewProfile => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.profile_open = true;
                    ui.more_menu_open = false;
                }
                ctx.notify();
            }
            ChatShellAction::SetWallpaper => {
                self.set_toast("设置壁纸（即将支持）");
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.more_menu_open = false;
                }
                ctx.notify();
            }
            ChatShellAction::DisableSharing => {
                self.set_toast("已禁用分享（演示）");
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.more_menu_open = false;
                }
                ctx.notify();
            }
            ChatShellAction::ClearHistory => {
                let thread = self.thread.clone();
                ctx.update_view(&thread, |thread, ctx| {
                    thread.clear_local_messages(ctx);
                });
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.more_menu_open = false;
                    ui.toast = "已清空本地消息视图".into();
                }
                ctx.notify();
            }
            ChatShellAction::DeleteChat => {
                if let Ok(mut guard) = self.selection.lock() {
                    *guard = None;
                }
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.more_menu_open = false;
                    ui.profile_open = false;
                    ui.rdp_connected = false;
                    ui.toast = "已删除本地会话选择".into();
                }
                ctx.notify();
            }
            ChatShellAction::SearchEdit(edit) => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    self.search_field.apply(&mut ui.search_query, edit);
                }
                self.search_focused = true;
                sync_caret_blink(self, ctx);
                self.sync_thread_filter(ctx);
                ctx.notify();
            }
            ChatShellAction::FocusSearch => {
                self.search_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatShellAction::ProfileRemoteDesktop => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.profile_open = false;
                }
                self.toggle_remote_desktop(ctx);
            }
            ChatShellAction::ProfileVoiceCall => {
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.profile_open = false;
                    ui.toast = "语音通话（演示）".into();
                }
                ctx.notify();
            }
            ChatShellAction::ProfileOpenSharedFiles => {
                let node_id = self
                    .ui_state
                    .lock()
                    .ok()
                    .and_then(|u| u.peer_node_id.clone());
                if let Ok(mut ui) = self.ui_state.lock() {
                    ui.profile_open = false;
                    ui.more_menu_open = false;
                }
                if let Some(node_id) = node_id {
                    ctx.emit(ChatShellEvent::NavigateToDevices { node_id });
                } else {
                    self.set_toast("请先选择在线终端");
                    ctx.notify();
                }
            }
        }
    }
}

impl CaretBlinkHost for ChatShellView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused
    }
}