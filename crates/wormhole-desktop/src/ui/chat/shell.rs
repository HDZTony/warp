use std::sync::{Arc, Mutex};

use warpui::elements::{
    Border, ChildView, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext, ViewHandle};

use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::header::{ChatHeaderView, TG_HEADER_HEIGHT};
use crate::ui::chat::profile_panel::{ChatProfileEvent, ChatProfilePanelView};
use crate::ui::chat::sidebar::ChatSidebarView;
use crate::ui::chat::shell_state::{
    chat_event_triggers_refresh, new_shared_shell_state, SharedChatShellState,
};
use wormhole_desktop_core::chat_commands::ChatEventDto;
use wormhole_desktop_core::DesktopEventBus;
use crate::ui::chat::thread::ChatThreadView;
use crate::ui::chat::thread_search::ChatThreadSearchView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{load_device_gate, wrap_with_device_gate, DeviceGateStatus};
use crate::ui::panel_primitives::{status_line, tab_content_fill, StatusTone};
use crate::ui::theme;

pub const SIDEBAR_WIDTH: f32 = 300.0;
pub const PROFILE_PANEL_WIDTH: f32 = 300.0;

pub type ConversationSelection = Arc<Mutex<Option<String>>>;

#[derive(Debug, Clone)]
pub enum ChatShellAction {
    DismissOverlays,
}

#[derive(Debug, Clone)]
pub enum ChatShellEvent {
    BrowseNodeShares(String),
}

pub struct ChatShellView {
    core: CoreHandle,
    font: FamilyId,
    gate: DeviceGateStatus,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    sidebar: ViewHandle<ChatSidebarView>,
    header: ViewHandle<ChatHeaderView>,
    thread_search: ViewHandle<ChatThreadSearchView>,
    thread: ViewHandle<ChatThreadView>,
    compose: ViewHandle<ChatComposeView>,
    profile: ViewHandle<ChatProfilePanelView>,
}

impl ChatShellView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let selection = Arc::new(Mutex::new(None));
        let shell_state = new_shared_shell_state();
        let sidebar = ctx.add_typed_action_view(|ctx| {
            ChatSidebarView::new(
                ctx,
                core.clone(),
                selection.clone(),
                shell_state.clone(),
            )
        });
        let header = ctx.add_typed_action_view(|ctx| {
            ChatHeaderView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let thread_search = ctx.add_typed_action_view(|ctx| {
            ChatThreadSearchView::new(ctx, shell_state.clone())
        });
        let thread = ctx.add_view(|ctx| {
            ChatThreadView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let compose = ctx.add_typed_action_view(|ctx| {
            ChatComposeView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        let profile = ctx.add_typed_action_view(|ctx| {
            ChatProfilePanelView::new(ctx, core.clone(), selection.clone(), shell_state.clone())
        });
        ctx.subscribe_to_view(&profile, |_, _, event, ctx| {
            if let ChatProfileEvent::BrowseNodeShares(node_id) = event {
                ctx.emit(ChatShellEvent::BrowseNodeShares(node_id.clone()));
            }
        });
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core: core.clone(),
            font,
            gate: DeviceGateStatus::default(),
            selection,
            shell_state,
            sidebar,
            header,
            thread_search,
            thread,
            compose,
            profile,
        };
        view.poll_gate(ctx);
        view.start_chat_event_listener(ctx);
        view
    }

    fn start_chat_event_listener(&self, ctx: &mut ViewContext<Self>) {
        let events = self.core.runtime().ctx.events.clone();
        let shell_state = self.shell_state.clone();
        let sidebar = self.sidebar.clone();
        Self::poll_chat_event_once(ctx, events, shell_state, sidebar);
    }

    fn poll_chat_event_once(
        ctx: &mut ViewContext<Self>,
        events: DesktopEventBus,
        shell_state: SharedChatShellState,
        sidebar: ViewHandle<ChatSidebarView>,
    ) {
        let mut rx = events.subscribe();
        ctx.spawn(async move { rx.recv().await }, move |_view, output, ctx| {
            if let Ok(event) = output {
                if event.name == "chat-event" {
                    if let Ok(chat_event) =
                        serde_json::from_value::<ChatEventDto>(event.payload.clone())
                    {
                        if chat_event_triggers_refresh(&chat_event.kind) {
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
            }
            Self::poll_chat_event_once(ctx, events, shell_state, sidebar);
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

    fn overlay_open(&self) -> bool {
        self.shell_state
            .lock()
            .map(|state| {
                state.header_menu_open
                    || state.profile_open
                    || state.thread_search_open
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
                    state.close_overlays();
                    state.profile_open = false;
                    state.close_thread_search();
                }
                ctx.notify();
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
        EventHandler::new(body)
            .on_keydown(|ctx, _, keystroke| {
                if keystroke.key == "escape" {
                    ctx.dispatch_typed_action(ChatShellAction::DismissOverlays);
                    return DispatchEventResult::StopPropagation;
                }
                DispatchEventResult::PropagateToParent
            })
            .finish()
    }
}
