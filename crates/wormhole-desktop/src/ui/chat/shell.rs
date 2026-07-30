use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use tokio::sync::broadcast::error::RecvError;
use warpui::elements::{
    Align, Border, ChildView, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{
    AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext, ViewHandle,
};

use crate::ui::chat::calls_panel::{open_video_viewer_url, CallsPanelEvent, CallsPanelView};
use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::contacts_panel::{ContactsPanelEvent, ContactsPanelView};
use crate::ui::chat::header::{ChatHeaderEvent, ChatHeaderView, TG_HEADER_HEIGHT};
use crate::ui::chat::profile_panel::{ChatProfileEvent, ChatProfilePanelView};
use crate::ui::chat::shell_state::{
    chat_event_triggers_refresh, new_shared_shell_state, SharedChatShellState,
};
use crate::ui::chat::sidebar::{ChatSidebarEvent, ChatSidebarView};
use crate::ui::chat::thread::ChatThreadView;
use crate::ui::chat::thread_search::ChatThreadSearchView;
use crate::ui::chat::voice_call_ui::{accept, apply_voice_status, decline, voice_error_toast};
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{load_device_gate, wrap_with_device_gate, DeviceGateStatus};
use crate::ui::panel_primitives::{status_line, tab_content_fill, StatusTone};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::ChatEventDto;

pub const SIDEBAR_WIDTH: f32 = 300.0;
pub const PROFILE_PANEL_WIDTH: f32 = 300.0;

pub type ConversationSelection = Arc<Mutex<Option<String>>>;

#[derive(Debug, Clone)]
pub enum ChatShellAction {
    DismissOverlays,
    VoiceCallAccept,
    VoiceCallDecline,
}

#[derive(Debug, Clone)]
pub enum ChatShellEvent {
    BrowseNodeShares(String),
    OpenRemoteDesktop { peer: String },
    OpenLiveViewer { peer: String, title: String },
}

pub struct ChatShellView {
    core: CoreHandle,
    font: FamilyId,
    gate: DeviceGateStatus,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    event_rx: Arc<
        tokio::sync::Mutex<tokio::sync::broadcast::Receiver<wormhole_desktop_core::DesktopEvent>>,
    >,
    sidebar: ViewHandle<ChatSidebarView>,
    header: ViewHandle<ChatHeaderView>,
    thread_search: ViewHandle<ChatThreadSearchView>,
    thread: ViewHandle<ChatThreadView>,
    compose: ViewHandle<ChatComposeView>,
    profile: ViewHandle<ChatProfilePanelView>,
    contacts: ViewHandle<ContactsPanelView>,
    calls: ViewHandle<CallsPanelView>,
}

impl ChatShellView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let selection = Arc::new(Mutex::new(None));
        let shell_state = new_shared_shell_state();
        let sidebar = ctx.add_typed_action_view(|ctx| {
            ChatSidebarView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let header = ctx.add_typed_action_view(|ctx| {
            ChatHeaderView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let thread_search =
            ctx.add_typed_action_view(|ctx| ChatThreadSearchView::new(ctx, shell_state.clone()));
        let thread = ctx.add_typed_action_view(|ctx| {
            ChatThreadView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let compose = ctx.add_typed_action_view(|ctx| {
            ChatComposeView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let profile = ctx.add_typed_action_view(|ctx| {
            ChatProfilePanelView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let contacts = ctx.add_typed_action_view(|ctx| {
            ContactsPanelView::new(ctx, core.clone(), shell_state.clone())
        });
        let calls = ctx.add_typed_action_view(|ctx| {
            CallsPanelView::new(ctx, core.clone(), shell_state.clone())
        });
        ctx.subscribe_to_view(&profile, |view, _, event, ctx| match event {
            ChatProfileEvent::BrowseNodeShares(node_id) => {
                ctx.emit(ChatShellEvent::BrowseNodeShares(node_id.clone()));
            }
            ChatProfileEvent::OpenRemoteDesktop { peer } => {
                ctx.emit(ChatShellEvent::OpenRemoteDesktop { peer: peer.clone() });
            }
            ChatProfileEvent::StartVoiceCall => {
                let header = view.header.clone();
                ctx.update_view(&header, |header, ctx| header.trigger_voice_call(ctx));
                ctx.notify();
            }
        });
        ctx.subscribe_to_view(&header, |_, _, event, ctx| match event {
            ChatHeaderEvent::OpenRemoteDesktop { peer } => {
                ctx.emit(ChatShellEvent::OpenRemoteDesktop { peer: peer.clone() });
            }
            ChatHeaderEvent::OpenLiveViewer { peer, title } => {
                ctx.emit(ChatShellEvent::OpenLiveViewer {
                    peer: peer.clone(),
                    title: title.clone(),
                });
            }
        });
        let selected_header = header.clone();
        let selected_thread = thread.clone();
        let selected_sidebar = sidebar.clone();
        ctx.subscribe_to_view(&sidebar, move |_, _, event, ctx| match event {
            ChatSidebarEvent::Selected(_) => {
                ctx.update_view(&selected_header, |header, ctx| {
                    header.selection_changed(ctx);
                });
                ctx.update_view(&selected_thread, |thread, ctx| {
                    thread.selection_changed(ctx);
                });
                ctx.notify();
            }
            ChatSidebarEvent::OpenContacts | ChatSidebarEvent::OpenCalls => {
                ctx.notify();
            }
        });
        let open_header = header.clone();
        let open_thread = thread.clone();
        ctx.subscribe_to_view(&contacts, move |view, _, event, ctx| match event {
            ContactsPanelEvent::OpenConversation(conv_id) => {
                if let Ok(mut guard) = view.selection.lock() {
                    *guard = Some(conv_id.clone());
                }
                if let Ok(mut state) = view.shell_state.lock() {
                    state.selection_tick = state.selection_tick.saturating_add(1);
                }
                ctx.update_view(&open_header, |header, ctx| header.selection_changed(ctx));
                ctx.update_view(&open_thread, |thread, ctx| thread.selection_changed(ctx));
                ctx.update_view(&selected_sidebar, |sidebar, ctx| sidebar.refresh(ctx));
                ctx.notify();
            }
            ContactsPanelEvent::Closed => ctx.notify(),
        });
        let call_header = header.clone();
        let call_thread = thread.clone();
        let call_sidebar = sidebar.clone();
        ctx.subscribe_to_view(&calls, move |view, _, event, ctx| match event {
            CallsPanelEvent::OpenConversation(conv_id) => {
                if let Ok(mut guard) = view.selection.lock() {
                    *guard = Some(conv_id.clone());
                }
                if let Ok(mut state) = view.shell_state.lock() {
                    state.selection_tick = state.selection_tick.saturating_add(1);
                }
                ctx.update_view(&call_header, |header, ctx| header.selection_changed(ctx));
                ctx.update_view(&call_thread, |thread, ctx| thread.selection_changed(ctx));
                ctx.update_view(&call_sidebar, |sidebar, ctx| sidebar.refresh(ctx));
                ctx.notify();
            }
            CallsPanelEvent::OpenVideoViewerUrl(url) => {
                if let Err(err) = open_video_viewer_url(&url) {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(format!("无法打开视频窗口: {err}"), StatusTone::Danger);
                    }
                }
                ctx.notify();
            }
            CallsPanelEvent::OpenLiveViewer { peer, title } => {
                ctx.emit(ChatShellEvent::OpenLiveViewer {
                    peer: peer.clone(),
                    title: title.clone(),
                });
                ctx.notify();
            }
            CallsPanelEvent::Closed => ctx.notify(),
        });
        let font = crate::ui::fonts::load_ui_font(ctx);
        let event_rx = Arc::new(tokio::sync::Mutex::new(
            core.runtime().ctx.events.subscribe(),
        ));
        let mut view = Self {
            core: core.clone(),
            font,
            gate: DeviceGateStatus::default(),
            selection,
            shell_state,
            event_rx,
            sidebar,
            header,
            thread_search,
            thread,
            compose,
            profile,
            contacts,
            calls,
        };
        view.poll_gate(ctx);
        view.start_chat_event_listener(ctx);
        view
    }

    fn start_chat_event_listener(&self, ctx: &mut ViewContext<Self>) {
        let event_rx = Arc::clone(&self.event_rx);
        let shell_state = self.shell_state.clone();
        let sidebar = self.sidebar.clone();
        let thread = self.thread.clone();
        let selection = self.selection.clone();
        Self::poll_chat_event_once(ctx, event_rx, shell_state, sidebar, thread, selection);
    }

    fn poll_chat_event_once(
        ctx: &mut ViewContext<Self>,
        event_rx: Arc<
            tokio::sync::Mutex<
                tokio::sync::broadcast::Receiver<wormhole_desktop_core::DesktopEvent>,
            >,
        >,
        shell_state: SharedChatShellState,
        sidebar: ViewHandle<ChatSidebarView>,
        thread: ViewHandle<ChatThreadView>,
        selection: ConversationSelection,
    ) {
        let rx = Arc::clone(&event_rx);
        ctx.spawn(
            async move {
                let mut guard = rx.lock().await;
                guard.recv().await
            },
            move |_view, output, ctx| {
                let mut lagged = false;
                match output {
                    Ok(event) if event.name == "chat-event" => {
                        if let Ok(chat_event) =
                            serde_json::from_value::<ChatEventDto>(event.payload.clone())
                        {
                            if chat_event.kind == "sync_required" {
                                lagged = true;
                            }
                            if chat_event_triggers_refresh(&chat_event.kind) {
                                if let Some(message) = chat_event.message.clone() {
                                    let conv_id = chat_event.conv_id.clone();
                                    ctx.update_view(&thread, |thread, ctx| {
                                        thread.apply_incoming_message(message, &conv_id, ctx);
                                    });
                                }
                                if let Ok(mut state) = shell_state.lock() {
                                    state.bump_message_tick();
                                }
                                ctx.update_view(&sidebar, |sidebar, ctx| {
                                    sidebar.refresh(ctx);
                                });
                                ctx.notify();
                            }
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        lagged = true;
                    }
                    Err(RecvError::Closed) => return,
                    _ => {}
                }
                if lagged {
                    if let Ok(mut state) = shell_state.lock() {
                        state.bump_message_tick();
                    }
                    let active_conv = selection.lock().ok().and_then(|guard| guard.clone());
                    if let Some(conv_id) = active_conv {
                        ctx.update_view(&thread, |thread, ctx| {
                            thread.force_sync_messages(&conv_id, ctx);
                        });
                    }
                    ctx.update_view(&sidebar, |sidebar, ctx| {
                        sidebar.refresh(ctx);
                    });
                    ctx.notify();
                }
                Self::poll_chat_event_once(ctx, event_rx, shell_state, sidebar, thread, selection);
            },
        );
    }

    pub fn refresh_cluster_display(&mut self, ctx: &mut ViewContext<Self>) {
        let sidebar = self.sidebar.clone();
        ctx.update_view(&sidebar, |sidebar, ctx| {
            sidebar.refresh(ctx);
        });
        self.poll_gate(ctx);
    }

    pub fn set_remote_desktop_result(
        &mut self,
        result: Result<(), String>,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Ok(mut state) = self.shell_state.lock() {
            match result {
                Ok(()) => state.show_toast("已打开远程桌面窗口", StatusTone::Success),
                Err(err) => {
                    state.show_toast(format!("无法打开远程桌面：{err}"), StatusTone::Danger)
                }
            }
        }
        ctx.notify();
    }

    pub fn poll_gate(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let apply: Arc<dyn Fn(&mut Self, DeviceGateStatus) + Send + Sync> =
            Arc::new(|view, gate| {
                view.gate = gate;
            });
        load_device_gate(core, ctx, apply);
    }

    /// Switch-in entry from Devices Tab 「发信息」.
    pub fn open_chat_for_cluster_node(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        let sidebar = self.sidebar.clone();
        ctx.update_view(&sidebar, |sidebar, ctx| {
            sidebar.open_chat_for_cluster_node(node_id, ctx);
        });
        ctx.notify();
    }

    fn overlay_open(&self) -> bool {
        self.shell_state
            .lock()
            .map(|state| {
                state.header_menu_open
                    || state.profile_open
                    || state.thread_search_open
                    || state.sidebar_menu_open
                    || state.contacts_open
                    || state.calls_open
            })
            .unwrap_or(false)
    }

    fn toast_line(&self) -> Option<Box<dyn Element>> {
        let (text, tone) = self
            .shell_state
            .lock()
            .ok()
            .filter(|state| !state.toast.is_empty())
            .map(|state| (state.toast.clone(), state.toast_tone))?;
        Some(status_line(text, self.font, tone))
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
        wrap_with_device_gate(self.font, "P2P 聊天", &self.gate, self.chat_body())
    }
}

impl TypedActionView for ChatShellView {
    type Action = ChatShellAction;

    fn handle_action(&mut self, action: &ChatShellAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatShellAction::DismissOverlays => {
                if let Ok(mut state) = self.shell_state.lock() {
                    // Esc hierarchy: confirm → menu → subview → close panel → other overlays
                    if state.calls_open {
                        if state.calls_subview == "confirm" {
                            state.calls_subview = "overview".into();
                            state.calls_menu_open = false;
                        } else if state.calls_menu_open {
                            state.calls_menu_open = false;
                        } else if state.calls_subview != "overview" {
                            state.calls_subview = "overview".into();
                        } else {
                            state.close_calls();
                        }
                    } else if state.contacts_add_open {
                        state.contacts_add_open = false;
                    } else if state.contacts_open {
                        state.close_contacts();
                    } else if state.sidebar_menu_open {
                        state.sidebar_menu_open = false;
                    } else {
                        state.close_overlays();
                        state.profile_open = false;
                        state.close_thread_search();
                    }
                }
                ctx.notify();
            }
            ChatShellAction::VoiceCallAccept => {
                self.spawn_voice_accept(ctx);
            }
            ChatShellAction::VoiceCallDecline => {
                self.spawn_voice_decline(ctx);
            }
        }
    }
}

impl ChatShellView {
    fn chat_body(&self) -> Box<dyn Element> {
        let profile_open = self
            .shell_state
            .lock()
            .map(|state| state.profile_open)
            .unwrap_or(false);

        let mut main_col = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                ConstrainedBox::new(ChildView::new(&self.header).finish())
                    .with_height(TG_HEADER_HEIGHT)
                    .finish(),
            )
            .with_child(self.incoming_call_banner())
            .with_child(ChildView::new(&self.thread_search).finish())
            .with_child(Expanded::new(1.0, ChildView::new(&self.thread).finish()).finish())
            .with_child(ChildView::new(&self.compose).finish());
        if let Some(toast) = self.toast_line() {
            main_col.add_child(toast);
        }

        let mut row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ConstrainedBox::new(ChildView::new(&self.sidebar).finish())
                    .with_width(SIDEBAR_WIDTH)
                    .finish(),
            )
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(main_col.finish())
                        .with_background(theme::canvas())
                        .with_border(Border::left(1.0).with_border_fill(theme::border()))
                        .finish(),
                )
                .finish(),
            );
        if profile_open {
            row.add_child(
                ConstrainedBox::new(
                    Container::new(ChildView::new(&self.profile).finish())
                        .with_background(theme::panel())
                        .with_border(Border::left(1.0).with_border_fill(theme::border()))
                        .finish(),
                )
                .with_width(PROFILE_PANEL_WIDTH)
                .finish(),
            );
        }

        let body = tab_content_fill(row.finish());
        let mut stack = Stack::new();
        stack.add_child(
            EventHandler::new(body)
                .on_keydown(|ctx, _, keystroke| {
                    if keystroke.key == "escape" {
                        ctx.dispatch_typed_action(ChatShellAction::DismissOverlays);
                        return DispatchEventResult::StopPropagation;
                    }
                    DispatchEventResult::PropagateToParent
                })
                .finish(),
        );
        stack.add_child(ChildView::new(&self.contacts).finish());
        stack.add_child(ChildView::new(&self.calls).finish());
        stack.finish()
    }

    fn incoming_call_banner(&self) -> Box<dyn Element> {
        let incoming = self
            .shell_state
            .lock()
            .map(|state| state.voice_call_phase == "incoming")
            .unwrap_or(false);
        if !incoming {
            return Container::new(Flex::row().finish()).finish();
        }
        let font = self.font;
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Expanded::new(
                        1.0,
                        ui_text::body("语音来电".to_string(), font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .with_child(incoming_call_button(
                    font,
                    "接听",
                    false,
                    ChatShellAction::VoiceCallAccept,
                ))
                .with_child(
                    Container::new(incoming_call_button(
                        font,
                        "拒绝",
                        true,
                        ChatShellAction::VoiceCallDecline,
                    ))
                    .with_margin_left(8.0)
                    .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(theme::accent_cool_bg(24))
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn spawn_voice_accept(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(conv_id) = conv_id else {
            return;
        };
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let header = self.header.clone();
        let request_conv_id = conv_id.clone();
        ctx.spawn(
            async move { accept(&core, &request_conv_id).await },
            move |_view, output, ctx| match output {
                Ok(status) => {
                    apply_voice_status(&shell_state, &status);
                    if let Ok(mut state) = shell_state.lock() {
                        if status.phase == "active" {
                            state.show_toast("语音通话已连接", StatusTone::Success);
                        }
                    }
                    let header = header.clone();
                    let conv = conv_id.clone();
                    ctx.update_view(&header, |header, ctx| {
                        header.poll_voice_status(&conv, ctx);
                    });
                    ctx.notify();
                }
                Err(err) => {
                    let (text, tone) = voice_error_toast(&err);
                    if let Ok(mut state) = shell_state.lock() {
                        state.show_toast(text, tone);
                    }
                    ctx.notify();
                }
            },
        );
    }

    fn spawn_voice_decline(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(conv_id) = conv_id else {
            return;
        };
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let header = self.header.clone();
        let request_conv_id = conv_id.clone();
        ctx.spawn(
            async move { decline(&core, &request_conv_id).await },
            move |_view, output, ctx| {
                match output {
                    Ok(status) => {
                        apply_voice_status(&shell_state, &status);
                        if let Ok(mut state) = shell_state.lock() {
                            state.show_toast("已拒绝来电", StatusTone::Muted);
                        }
                        let header = header.clone();
                        let conv = conv_id.clone();
                        ctx.update_view(&header, |header, ctx| {
                            header.poll_voice_status(&conv, ctx);
                        });
                    }
                    Err(err) => {
                        let (text, tone) = voice_error_toast(&err);
                        if let Ok(mut state) = shell_state.lock() {
                            state.show_toast(text, tone);
                        }
                    }
                }
                ctx.notify();
            },
        );
    }
}

fn incoming_call_button(
    font: FamilyId,
    label: &str,
    danger: bool,
    action: ChatShellAction,
) -> Box<dyn Element> {
    let label = label.to_string();
    let action = action.clone();
    EventHandler::new(
        Container::new(
            Align::new(
                ui_text::body(label, font)
                    .with_color(if danger {
                        theme::danger()
                    } else {
                        theme::accent_cool()
                    })
                    .finish(),
            )
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(if danger {
            ColorU::new(232, 93, 76, 32)
        } else {
            theme::accent_bg_default()
        })
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}
