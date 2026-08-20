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
use crate::ui::chat::channel_create_panel::{ChannelCreateEvent, ChannelCreatePanelView};
use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::contacts_panel::{ContactsPanelEvent, ContactsPanelView};
use crate::ui::chat::header::{ChatHeaderEvent, ChatHeaderView, TG_HEADER_HEIGHT};
use crate::ui::chat::image_viewer::{attachment_context_menu_overlay, image_viewer_overlay};
use crate::ui::chat::media_upload_modal::MediaUploadModalView;
use crate::ui::chat::outgoing_call_panel::{OutgoingCallEvent, OutgoingCallPanelView};
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
use crate::ui::panel_primitives::{chat_thread_tint, status_line, tab_content_fill, StatusTone};
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
    SetDropHover(bool),
    DropFiles(Vec<String>),
    ForwardPick(String),
    ForwardCancel,
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
    channel_create: ViewHandle<ChannelCreatePanelView>,
    calls: ViewHandle<CallsPanelView>,
    media_upload: ViewHandle<MediaUploadModalView>,
    outgoing_call: ViewHandle<OutgoingCallPanelView>,
    drop_hover: bool,
    last_overlay_tick: u64,
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
        let channel_create = ctx.add_typed_action_view(|ctx| {
            ChannelCreatePanelView::new(ctx, core.clone(), shell_state.clone())
        });
        let calls = ctx.add_typed_action_view(|ctx| {
            CallsPanelView::new(ctx, core.clone(), shell_state.clone())
        });
        let media_upload = ctx.add_typed_action_view(|ctx| {
            MediaUploadModalView::new(
                ctx,
                core.clone(),
                selection.clone(),
                shell_state.clone(),
            )
        });
        let outgoing_call = {
            let font = crate::ui::fonts::load_ui_font(ctx);
            let shell_state = shell_state.clone();
            ctx.add_typed_action_view(move |ctx| {
                OutgoingCallPanelView::new(ctx, shell_state.clone(), font)
            })
        };
        ctx.subscribe_to_view(&profile, |view, _, event, ctx| match event {
            ChatProfileEvent::OpenRemoteDesktop { peer } => {
                ctx.emit(ChatShellEvent::OpenRemoteDesktop { peer: peer.clone() });
            }
            ChatProfileEvent::FocusCompose => {
                let compose = view.compose.clone();
                ctx.update_view(&compose, |compose, ctx| compose.focus_input(ctx));
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
        let selected_compose = compose.clone();
        ctx.subscribe_to_view(&sidebar, move |_, _, event, ctx| match event {
            ChatSidebarEvent::Selected(_) => {
                ctx.update_view(&selected_header, |header, ctx| {
                    header.selection_changed(ctx);
                });
                ctx.update_view(&selected_thread, |thread, ctx| {
                    thread.selection_changed(ctx);
                });
                ctx.update_view(&selected_compose, |compose, ctx| {
                    compose.selection_changed(ctx);
                });
                ctx.notify();
            }
            ChatSidebarEvent::OpenContacts
            | ChatSidebarEvent::OpenCalls
            | ChatSidebarEvent::OpenChannel => {
                ctx.notify();
            }
        });
        let open_header = header.clone();
        let open_thread = thread.clone();
        let open_compose = compose.clone();
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
                ctx.update_view(&open_compose, |compose, ctx| compose.selection_changed(ctx));
                ctx.update_view(&selected_sidebar, |sidebar, ctx| sidebar.refresh(ctx));
                ctx.notify();
            }
            ContactsPanelEvent::Closed => ctx.notify(),
        });
        let channel_header = header.clone();
        let channel_thread = thread.clone();
        let channel_sidebar = sidebar.clone();
        let channel_compose = compose.clone();
        ctx.subscribe_to_view(&channel_create, move |view, _, event, ctx| match event {
            ChannelCreateEvent::Created(conv_id) => {
                if let Ok(mut guard) = view.selection.lock() {
                    *guard = Some(conv_id.clone());
                }
                if let Ok(mut state) = view.shell_state.lock() {
                    state.selection_tick = state.selection_tick.saturating_add(1);
                }
                ctx.update_view(&channel_header, |header, ctx| header.selection_changed(ctx));
                ctx.update_view(&channel_thread, |thread, ctx| thread.selection_changed(ctx));
                ctx.update_view(&channel_compose, |compose, ctx| compose.selection_changed(ctx));
                ctx.update_view(&channel_sidebar, |sidebar, ctx| sidebar.refresh(ctx));
                ctx.notify();
            }
            ChannelCreateEvent::Closed => ctx.notify(),
        });
        let call_header = header.clone();
        let call_thread = thread.clone();
        let call_sidebar = sidebar.clone();
        let call_compose = compose.clone();
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
                ctx.update_view(&call_compose, |compose, ctx| compose.selection_changed(ctx));
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
        let outgoing_header = header.clone();
        ctx.subscribe_to_view(&outgoing_call, move |_view, _, event, ctx| match event {
            OutgoingCallEvent::StartVoice => {
                ctx.update_view(&outgoing_header, |header, ctx| {
                    header.confirm_outgoing_voice(ctx);
                });
                ctx.notify();
            }
            OutgoingCallEvent::StartVideo => {
                ctx.update_view(&outgoing_header, |header, ctx| {
                    header.confirm_outgoing_video(ctx);
                });
                ctx.notify();
            }
            OutgoingCallEvent::Cancel => {
                ctx.update_view(&outgoing_header, |header, ctx| {
                    header.cancel_outgoing_panel(ctx);
                });
                ctx.notify();
            }
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
            channel_create,
            calls,
            media_upload,
            outgoing_call,
            drop_hover: false,
            last_overlay_tick: 0,
        };
        view.poll_gate(ctx);
        view.start_chat_event_listener(ctx);
        view.start_overlay_poll(ctx);
        view
    }

    fn start_overlay_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            },
            |view, _, ctx| {
                let tick = view
                    .shell_state
                    .lock()
                    .map(|state| state.overlay_tick)
                    .unwrap_or(0);
                if tick != view.last_overlay_tick {
                    view.last_overlay_tick = tick;
                    let media = view.media_upload.clone();
                    ctx.update_view(&media, |modal, ctx| {
                        modal.queue_missing_previews(ctx);
                    });
                    let outgoing = view.outgoing_call.clone();
                    ctx.update_view(&outgoing, |_panel, ctx| {
                        ctx.notify();
                    });
                    ctx.notify();
                }
                view.start_overlay_poll(ctx);
            },
        );
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
                            if chat_event.kind == "announce_warning" {
                                if let Ok(mut state) = shell_state.lock() {
                                    state.show_toast(
                                        "消息已保存，但对端拨号地址为空，可能无法即时同步到对方",
                                        StatusTone::Warn,
                                    );
                                }
                                ctx.notify();
                            }
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
                    || state.channel_create_open
                    || state.calls_open
                    || state.media_upload_open
                    || state.outgoing_call_ui.is_some()
                    || state.image_viewer.is_some()
                    || state.attachment_context_menu.is_some()
                    || state.forward_draft.is_some()
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
                    if state.outgoing_call_ui.is_some() {
                        let ringing = state
                            .outgoing_call_ui
                            .as_ref()
                            .is_some_and(|ui| ui.is_ringing());
                        if ringing {
                            drop(state);
                            let header = self.header.clone();
                            ctx.update_view(&header, |header, ctx| {
                                header.cancel_outgoing_panel(ctx);
                            });
                        } else {
                            state.clear_outgoing_call_ui();
                        }
                        ctx.notify();
                        return;
                    }
                    if state.image_viewer.is_some() || state.attachment_context_menu.is_some() {
                        state.close_image_viewer();
                    } else if state.forward_draft.is_some() {
                        state.clear_forward_draft();
                    } else if state.reply_draft.is_some() {
                        state.clear_reply_draft();
                    } else if state.calls_open {
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
                    } else if state.media_upload_open {
                        if state.media_upload.editing_item_id.is_some() {
                            if state.media_upload.edit_paint_mode {
                                state.media_upload.edit_paint_mode = false;
                            } else {
                                state.media_upload.editing_item_id = None;
                                state.media_upload.edit_paint_mode = false;
                            }
                            state.bump_overlay_tick();
                        } else {
                            state.close_media_upload();
                        }
                    } else if state.contacts_add_open {
                        state.contacts_add_open = false;
                    } else if state.contacts_open {
                        state.close_contacts();
                    } else if state.channel_create_open {
                        state.close_channel_create();
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
            ChatShellAction::SetDropHover(hover) => {
                if self.drop_hover != *hover {
                    self.drop_hover = *hover;
                    ctx.notify();
                }
            }
            ChatShellAction::DropFiles(paths) => {
                self.drop_hover = false;
                let compose = self.compose.clone();
                let paths = paths.clone();
                ctx.update_view(&compose, |compose, ctx| {
                    compose.stage_dropped_paths(paths, ctx);
                });
                ctx.notify();
            }
            ChatShellAction::ForwardCancel => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.clear_forward_draft();
                }
                ctx.notify();
            }
            ChatShellAction::ForwardPick(conv_id) => {
                self.forward_to_conversation(conv_id.clone(), ctx);
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
                    EventHandler::new(
                        Container::new(main_col.finish())
                            .with_background(chat_thread_tint())
                            .with_border(if self.drop_hover {
                                Border::all(2.0).with_border_fill(theme::accent_cool())
                            } else {
                                Border::left(1.0).with_border_fill(theme::border())
                            })
                            .finish(),
                    )
                    .on_drag_files(|ctx, _, _| {
                        ctx.dispatch_typed_action(ChatShellAction::SetDropHover(true));
                        DispatchEventResult::StopPropagation
                    })
                    .on_drag_file_exit(|ctx, _, _| {
                        ctx.dispatch_typed_action(ChatShellAction::SetDropHover(false));
                        DispatchEventResult::StopPropagation
                    })
                    .on_drag_and_drop_files(|ctx, _, _, paths| {
                        ctx.dispatch_typed_action(ChatShellAction::DropFiles(paths.to_vec()));
                        DispatchEventResult::StopPropagation
                    })
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
        stack.add_child(ChildView::new(&self.channel_create).finish());
        stack.add_child(ChildView::new(&self.calls).finish());
        stack.add_child(ChildView::new(&self.media_upload).finish());
        stack.add_child(ChildView::new(&self.outgoing_call).finish());
        if let Ok(state) = self.shell_state.lock() {
            if let Some(viewer) = state.image_viewer.as_ref() {
                stack.add_child(image_viewer_overlay(
                    self.font,
                    viewer,
                    self.shell_state.clone(),
                ));
            }
            if let Some(menu) = state.attachment_context_menu.as_ref() {
                stack.add_child(attachment_context_menu_overlay(
                    self.font,
                    menu,
                    self.shell_state.clone(),
                ));
            }
            if state.forward_draft.is_some() {
                stack.add_child(self.forward_picker_overlay());
            }
        }
        stack.finish()
    }

    fn forward_picker_overlay(&self) -> Box<dyn Element> {
        let font = self.font;
        let shell_state = self.shell_state.clone();
        let core = self.core.clone();
        // Conversations are loaded asynchronously when the overlay opens via ForwardPick after
        // listing; for the picker UI we show a simple prompt + cancel, and let the user pick
        // from the already-visible sidebar by tapping a conversation after forward is armed.
        // Here we expose cancel + instruction; selecting happens when shell receives ForwardPick
        // from a future sidebar hook. For now provide an explicit "send to current chat" path.
        let current = self
            .selection
            .lock()
            .ok()
            .and_then(|g| g.clone());

        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::body(wormhole_i18n::t("chat.image.forward_pick"), font)
                .with_color(theme::text())
                .finish(),
        );
        if let Some(conv_id) = current {
            let conv_id_click = conv_id.clone();
            col.add_child(
                Container::new(
                    EventHandler::new(
                        Container::new(
                            ui_text::body(wormhole_i18n::t("chat.image.forward_here"), font)
                                .with_color(theme::accent_cool())
                                .finish(),
                        )
                        .with_uniform_padding(10.0)
                        .finish(),
                    )
                    .with_automation_label(wormhole_i18n::t("chat.image.forward_here"))
                    .with_automation_id("chat:forward_here")
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(ChatShellAction::ForwardPick(
                            conv_id_click.clone(),
                        ));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(
                EventHandler::new(
                    Container::new(
                        ui_text::body(wormhole_i18n::t("chat.image.close"), font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_uniform_padding(10.0)
                    .finish(),
                )
                .with_automation_label(wormhole_i18n::t("chat.image.close"))
                .with_automation_id("chat:forward_cancel")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(ChatShellAction::ForwardCancel);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );

        let panel = Container::new(col.finish())
            .with_uniform_padding(16.0)
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(warpui::elements::CornerRadius::with_all(
                warpui::elements::Radius::Pixels(12.0),
            ))
            .finish();

        let _ = (shell_state, core);
        Align::new(
            Container::new(panel)
                .with_background(ColorU::new(0, 0, 0, 160))
                .finish(),
        )
        .finish()
    }

    fn forward_to_conversation(&mut self, conv_id: String, ctx: &mut ViewContext<Self>) {
        let draft = self
            .shell_state
            .lock()
            .ok()
            .and_then(|mut state| state.forward_draft.take());
        let Some(draft) = draft else {
            return;
        };
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = wormhole_desktop_core::chat_commands::SendChatMessageParams {
                    conv_id,
                    body: String::new(),
                    backend: None,
                    peer_bootstrap_addrs: Vec::new(),
                    sticker: None,
                    attachments: vec![
                        wormhole_desktop_core::chat_commands::SendChatAttachmentDto {
                            kind: draft.kind,
                            path: draft.local_path,
                        },
                    ],
                    reply_to: None,
                    forwarded_from: Some(draft.forwarded_from),
                };
                wormhole_desktop_core::chat_commands::chat_send_message(
                    runtime.ctx.as_ref(),
                    &runtime.state,
                    params,
                )
                .await
            },
            move |view, result, ctx| {
                if let Ok(mut state) = shell_state.lock() {
                    match result {
                        Ok(_) => {
                            state.clear_forward_draft();
                            state.show_toast(
                                wormhole_i18n::t("chat.image.forwarded"),
                                StatusTone::Success,
                            );
                            state.bump_message_tick();
                        }
                        Err(err) => {
                            state.show_toast(
                                format!(
                                    "{}: {err}",
                                    wormhole_i18n::t("chat.image.forward_failed")
                                ),
                                StatusTone::Danger,
                            );
                        }
                    }
                }
                let _ = view;
                ctx.notify();
            },
        );
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
    let btn_label = label.to_string();
    let automation_id = format!("chat:call:{btn_label}");
    EventHandler::new(
        Container::new(
            Align::new(
                ui_text::body(btn_label.clone(), font)
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
    .with_automation_label(btn_label)
    .with_automation_id(automation_id)
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}
