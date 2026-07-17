use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, ChildAnchor, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, Empty, EventHandler,
    Expanded, Fill, Flex, Hoverable, MainAxisSize, MouseStateHandle, OffsetPositioning,
    ParentAnchor, ParentElement, ParentOffsetBounds, Radius, ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{
    assets::asset_cache::AssetCache, AccessibilityData, AppContext, Element, Entity,
    SingletonEntity as _, TypedActionView, UpdateView, View, ViewContext, ViewHandle, WindowId,
};
use warpui_core::image_cache::{CustomImageFormat, CustomImageHeader, ImageType};
use warpui_core::keymap::Keystroke;

use crate::coordinator::{CoordinatorState, CoordinatorView, UiCommand};
use crate::ui::agent_panel::AgentPanelView;
use crate::ui::chat::{ChatShellEvent, ChatShellView};
use crate::ui::clipboard::write_clipboard_text;
use crate::ui::core_handle::CoreHandle;
use crate::ui::desktop_prefs::{self, redeem_history_from_ledger, RedeemHistoryEntry};
use crate::ui::devices_view::{DevicesEvent, DevicesView};
use crate::ui::display_view::DisplayView;
use crate::ui::hud_avatar_panel::{
    build_avatar_panel, build_avatar_slot, build_purchase_modal, build_redeem_modal,
    PurchaseProductUi,
};
use crate::ui::hud_effects::HudBackdrop;
use crate::ui::icons;
use crate::ui::login_modal::{LoginModalAction, LoginModalEvent, LoginModalView};
use crate::ui::panel_primitives::StatusTone;
use crate::ui::panel_primitives::{section_hint, tab_content_fill, HUD_RADIUS};
use crate::ui::settings_view::{SettingsEvent, SettingsView};
use crate::ui::sync_views::SyncView;
use crate::ui::text_field_input::{
    sync_caret_blink, CaretBlink, CaretBlinkHost, TextFieldEditAction, TextFieldState,
};
use crate::ui::theme;
use crate::ui::toolbox_view::ToolboxView;
use crate::ui::w_drive_view::SharedVaultView;
use crate::ui::window_chrome::{
    self, TrafficLightActions, TrafficLightMouseStates, CHROME_ROW_HEIGHT,
};
use crate::ui_text;
use wormhole_desktop_core::cloud_auth_status;
use wormhole_desktop_core::cloud_credits::{CloudCreditProductDto, CloudCreditRedeemRequest};
use wormhole_desktop_core::cluster_commands::{cluster_status, cluster_status_hud};
use wormhole_desktop_core::cluster_gossip_coordinator::ClusterGossipCoordinator;
use wormhole_desktop_core::warp_embed_prefs::PreferredAgent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    WDrive,
    Sync,
    Devices,
    Display,
    Chat,
    Warp,
    Toolbox,
    Settings,
}

impl AppTab {
    pub fn persist_id(self) -> &'static str {
        match self {
            AppTab::WDrive => "w_drive",
            AppTab::Sync => "sync",
            AppTab::Devices => "devices",
            AppTab::Display => "display",
            AppTab::Chat => "chat",
            AppTab::Warp => "warp",
            AppTab::Toolbox => "toolbox",
            AppTab::Settings => "settings",
        }
    }

    pub fn from_persist_id(id: &str) -> Option<Self> {
        match id {
            "w_drive" | "sync" | "devices" | "terminals" => Some(AppTab::Devices),
            "display" => Some(AppTab::Devices),
            "chat" => Some(AppTab::Chat),
            "warp" | "agent" => Some(AppTab::Warp),
            "toolbox" => Some(AppTab::Toolbox),
            "settings" => Some(AppTab::Settings),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSelectSource {
    Mouse,
    Keyboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteDesktopOrigin {
    Chat,
    Devices,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedeemTab {
    Redeem,
    History,
}

/// Persisted [`MouseStateHandle`]s for primary chrome tabs.
///
/// Must outlive each rebuild so [`Hoverable`] can clear hover when the pointer leaves
/// (ephemeral handles reset `is_hovered` and can leave tooltips stuck).
#[derive(Default)]
struct TabHoverMouseStates {
    devices: MouseStateHandle,
    chat: MouseStateHandle,
    warp: MouseStateHandle,
    toolbox: MouseStateHandle,
    settings: MouseStateHandle,
}

impl TabHoverMouseStates {
    fn for_tab(&self, tab: AppTab) -> MouseStateHandle {
        match tab {
            AppTab::Devices | AppTab::WDrive | AppTab::Sync | AppTab::Display => {
                self.devices.clone()
            }
            AppTab::Chat => self.chat.clone(),
            AppTab::Warp => self.warp.clone(),
            AppTab::Toolbox => self.toolbox.clone(),
            AppTab::Settings => self.settings.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppShellAction {
    SelectTab(AppTab, TabSelectSource),
    DismissOnboarding,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
    OpenLogin,
    ToggleAvatarPanel,
    CloseAvatarPanel,
    PurchaseBalance,
    ClosePurchaseModal,
    RefreshCreditProducts,
    CopyPurchaseLink(String),
    OpenPurchaseLink(String),
    OpenRedeemModal,
    CloseRedeemModal,
    SwitchRedeemTab(RedeemTab),
    RedeemCodeEdit(TextFieldEditAction),
    FocusRedeemCode,
    SubmitRedeem,
}

pub struct AppShellView {
    tab: AppTab,
    tab_focus: AppTab,
    tab_bar_keyboard_focus: bool,
    tab_hover_mouse_states: TabHoverMouseStates,
    core: CoreHandle,
    coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
    #[allow(dead_code)]
    coordinator_view: ViewHandle<CoordinatorView>,
    w_drive: ViewHandle<SharedVaultView>,
    sync: ViewHandle<SyncView>,
    devices: ViewHandle<DevicesView>,
    display: ViewHandle<DisplayView>,
    chat: ViewHandle<ChatShellView>,
    warp: ViewHandle<AgentPanelView>,
    toolbox: ViewHandle<ToolboxView>,
    settings: ViewHandle<SettingsView>,
    login_modal: ViewHandle<LoginModalView>,
    login_modal_open: bool,
    auth_authenticated: bool,
    auth_device_id: Option<String>,
    auth_email: Option<String>,
    font: FamilyId,
    mono: FamilyId,
    hud_nodes: usize,
    device_ready: bool,
    avatar_panel_open: bool,
    balance_credits: i64,
    balance_amount_yuan: Option<String>,
    balance_busy: bool,
    balance_feedback: Option<String>,
    purchase_modal_open: bool,
    purchase_products: Vec<CloudCreditProductDto>,
    purchase_busy: bool,
    purchase_feedback: Option<String>,
    purchase_qr_loaded: HashSet<String>,
    redeem_modal_open: bool,
    redeem_tab: RedeemTab,
    redeem_code_draft: String,
    redeem_code_field: TextFieldState,
    redeem_code_focused: bool,
    redeem_caret_blink: CaretBlink,
    redeem_feedback: Option<String>,
    redeem_feedback_tone: StatusTone,
    redeem_busy: bool,
    redeem_history: Vec<RedeemHistoryEntry>,
    redeem_history_busy: bool,
    redeem_history_error: Option<String>,
    redeem_code_overrides: HashMap<String, String>,
    redeem_history_scroll: ClippedScrollStateHandle,
    tab_scroll: ClippedScrollStateHandle,
    show_onboarding: bool,
    window_id: WindowId,
    traffic_light_mouse_states: TrafficLightMouseStates,
    desktop_event_rx: Arc<
        tokio::sync::Mutex<tokio::sync::broadcast::Receiver<wormhole_desktop_core::DesktopEvent>>,
    >,
    #[cfg(any(windows, target_os = "linux"))]
    tray: std::sync::Arc<wormhole_desktop_tray::TrayController>,
}

impl AppShellView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
        #[cfg(any(windows, target_os = "linux"))] tray: std::sync::Arc<
            wormhole_desktop_tray::TrayController,
        >,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let coordinator_view =
            ctx.add_typed_action_view(|ctx| CoordinatorView::new(ctx, coordinator.clone()));
        #[cfg(any(windows, target_os = "linux"))]
        crate::ui::windows_shell::register_main_shell_window(ctx.window_id(), &coordinator);
        let w_drive = ctx.add_view(|ctx| SharedVaultView::new(ctx, core.clone()));
        let sync = ctx.add_typed_action_view(|ctx| SyncView::new(ctx, core.clone()));
        let devices = ctx.add_typed_action_view(|ctx| DevicesView::new(ctx, core.clone()));
        let display = ctx.add_view(|ctx| DisplayView::new(ctx, core.clone()));
        let chat = ctx.add_typed_action_view(|ctx| ChatShellView::new(ctx, core.clone()));
        ctx.subscribe_to_view(&chat, |view, _, event, ctx| match event {
            ChatShellEvent::BrowseNodeShares(node_id) => {
                view.tab = AppTab::Devices;
                let devices = view.devices.clone();
                ctx.update_view(&devices, |devices, ctx| {
                    devices.open_node_from_chat(node_id.clone(), ctx);
                });
                ctx.notify();
            }
            ChatShellEvent::OpenRemoteDesktop { peer } => {
                view.open_remote_desktop_for_identity(peer, RemoteDesktopOrigin::Chat, ctx);
            }
            ChatShellEvent::OpenLiveViewer { peer, title } => {
                view.open_live_viewer_for_peer(peer, title.clone(), ctx);
            }
        });
        ctx.subscribe_to_view(&devices, |view, _, event, ctx| match event {
            DevicesEvent::OpenChat { node_id } => {
                let warp_handle = view.warp.clone();
                ctx.update_view(&warp_handle, |panel, ctx| {
                    panel.set_tab_visible(false, ctx);
                });
                view.tab = AppTab::Chat;
                view.tab_focus = AppTab::Chat;
                view.persist_last_tab();
                view.prompt_login_if_needed(ctx);
                let chat = view.chat.clone();
                let node_id = node_id.clone();
                ctx.update_view(&chat, |chat, ctx| {
                    chat.open_chat_for_cluster_node(node_id, ctx);
                });
                ctx.notify();
            }
            DevicesEvent::OpenRemoteDesktop { node_id } => {
                view.open_remote_desktop_for_identity(node_id, RemoteDesktopOrigin::Devices, ctx);
            }
        });
        let warp = ctx.add_typed_action_view(|ctx| AgentPanelView::new(ctx, core.clone()));
        let toolbox = ctx
            .add_typed_action_view(|ctx| ToolboxView::new(ctx, core.clone(), coordinator.clone()));
        let settings = ctx.add_typed_action_view(|ctx| SettingsView::new(ctx, core.clone()));
        let login_modal = ctx.add_typed_action_view(|ctx| LoginModalView::new(ctx, core.clone()));
        ctx.subscribe_to_view(&login_modal, |view, _, event, ctx| {
            match event {
                LoginModalEvent::AuthChanged {
                    authenticated,
                    device_id,
                    email,
                } => {
                    view.auth_authenticated = *authenticated;
                    view.auth_device_id = device_id.clone();
                    view.auth_email = email.clone();
                    view.login_modal_open = false;
                    let settings_handle = view.settings.clone();
                    ctx.update_view(&settings_handle, |settings, ctx| {
                        settings.refresh_account(ctx);
                    });
                    view.refresh_auth_gated_views(ctx);
                }
                LoginModalEvent::OpenChanged { open } => {
                    view.login_modal_open = *open;
                }
            }
            ctx.notify();
        });
        ctx.subscribe_to_view(&settings, |view, _, event, ctx| {
            match event {
                SettingsEvent::AccountChanged { authenticated } => {
                    view.auth_authenticated = *authenticated;
                    if !authenticated {
                        view.auth_device_id = None;
                        view.auth_email = None;
                        view.device_ready = false;
                        view.prompt_login_if_needed(ctx);
                    }
                    view.refresh_auth_gated_views(ctx);
                }
                SettingsEvent::OpenLogin => {
                    view.open_login_modal(ctx);
                }
                SettingsEvent::OpenClusterManagement => {
                    let warp_handle = view.warp.clone();
                    ctx.update_view(&warp_handle, |warp, ctx| {
                        warp.set_tab_visible(false, ctx);
                    });
                    view.tab = AppTab::Devices;
                    view.tab_focus = AppTab::Devices;
                    view.tab_bar_keyboard_focus = false;
                    view.persist_last_tab();
                    ctx.notify();
                }
                SettingsEvent::RestoreArchivedSession(id) => {
                    let warp_handle = view.warp.clone();
                    let session_id = id.clone();
                    ctx.update_view(&warp_handle, |warp, ctx| {
                        warp.restore_archived_session(session_id, ctx);
                    });
                    let settings_handle = view.settings.clone();
                    ctx.update_view(&settings_handle, |_settings, ctx| {
                        ctx.notify();
                    });
                }
                SettingsEvent::DeleteArchivedSession(id) => {
                    let warp_handle = view.warp.clone();
                    let session_id = id.clone();
                    ctx.update_view(&warp_handle, |warp, ctx| {
                        warp.delete_archived_session(session_id, ctx);
                    });
                    let settings_handle = view.settings.clone();
                    ctx.update_view(&settings_handle, |_settings, ctx| {
                        ctx.notify();
                    });
                }
            }
            ctx.notify();
        });
        let prefs = desktop_prefs::load(&core.data_dir());
        let mut tab = prefs
            .last_tab
            .as_deref()
            .and_then(AppTab::from_persist_id)
            .unwrap_or(AppTab::Chat);
        if matches!(tab, AppTab::WDrive | AppTab::Sync | AppTab::Display) {
            tab = AppTab::Devices;
        }
        let show_onboarding = !prefs.onboarding_dismissed;
        let window_id = ctx.window_id();
        let desktop_event_rx = Arc::new(tokio::sync::Mutex::new(
            core.runtime().ctx.events.subscribe(),
        ));
        let view = Self {
            tab,
            tab_focus: tab,
            tab_bar_keyboard_focus: false,
            tab_hover_mouse_states: TabHoverMouseStates::default(),
            core,
            coordinator,
            coordinator_view,
            w_drive,
            sync,
            devices,
            display,
            chat,
            warp,
            toolbox,
            settings,
            login_modal,
            login_modal_open: false,
            auth_authenticated: false,
            auth_device_id: None,
            auth_email: None,
            font,
            mono,
            hud_nodes: 1,
            device_ready: false,
            avatar_panel_open: false,
            balance_credits: 0,
            balance_amount_yuan: None,
            balance_busy: false,
            balance_feedback: None,
            purchase_modal_open: false,
            purchase_products: Vec::new(),
            purchase_busy: false,
            purchase_feedback: None,
            purchase_qr_loaded: HashSet::new(),
            redeem_modal_open: false,
            redeem_tab: RedeemTab::Redeem,
            redeem_code_draft: String::new(),
            redeem_code_field: TextFieldState::new(),
            redeem_code_focused: false,
            redeem_caret_blink: CaretBlink::new(),
            redeem_feedback: None,
            redeem_feedback_tone: StatusTone::Neutral,
            redeem_busy: false,
            redeem_history: Vec::new(),
            redeem_history_busy: false,
            redeem_history_error: None,
            redeem_code_overrides: prefs.redeem_code_overrides,
            redeem_history_scroll: ClippedScrollStateHandle::new(),
            tab_scroll: ClippedScrollStateHandle::new(),
            show_onboarding,
            window_id,
            traffic_light_mouse_states: TrafficLightMouseStates::default(),
            desktop_event_rx,
            #[cfg(any(windows, target_os = "linux"))]
            tray,
        };
        view.start_warp_focus_poll(ctx);
        view.start_hud_poll(ctx);
        view.refresh_auth_status(ctx);
        view.start_event_listener(ctx);
        #[cfg(any(windows, target_os = "linux"))]
        view.start_tray_poll(ctx);
        Self::sync_titlebar_height(ctx);
        window_chrome::sync_window_button_visibility(ctx);
        view
    }

    fn sync_titlebar_height(ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        if let Some(window) = ctx.windows().platform_window(window_id) {
            window.set_titlebar_height(CHROME_ROW_HEIGHT as f64);
        }
    }

    fn start_hud_poll(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let core_for_gossip = self.core.clone();
        ctx.spawn(
            async move {
                if wormhole_desktop_core::device_identity::is_device_ready(
                    core_for_gossip.app_state(),
                )
                .await
                    && !ClusterGossipCoordinator::global().is_gossip_ready()
                {
                    ClusterGossipCoordinator::global()
                        .ensure_background(core_for_gossip.runtime().state.clone());
                }
            },
            |_, _, _| {},
        );
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::poll_hud_once(ctx, tick_rx, core);
    }

    fn poll_hud_once(
        ctx: &mut ViewContext<Self>,
        tick_rx: async_channel::Receiver<()>,
        core: CoreHandle,
    ) {
        let waiter = tick_rx.clone();
        let core_for_task = core.clone();
        ctx.spawn(
            async move {
                let _ = waiter.recv().await;
                let state = core_for_task.runtime().state.clone();
                cluster_status_hud(&state).await
            },
            move |view, output, ctx| {
                if let Ok(status) = output {
                    view.hud_nodes = status.nodes.len().max(1);
                    let auth_now = !status.auth_required;
                    let auth_changed = auth_now != view.auth_authenticated;
                    view.auth_authenticated = auth_now;
                    if !auth_now {
                        view.auth_device_id = None;
                        view.auth_email = None;
                    }
                    let device_ready = auth_now && !status.device_bootstrap_required;
                    let device_ready_changed = device_ready != view.device_ready;
                    if auth_changed || device_ready_changed {
                        view.refresh_auth_gated_views(ctx);
                        if auth_changed {
                            let settings_handle = view.settings.clone();
                            ctx.update_view(&settings_handle, |settings, ctx| {
                                settings.refresh_account(ctx);
                            });
                            if !auth_now {
                                view.prompt_login_if_needed(ctx);
                            }
                        }
                    }
                    view.device_ready = device_ready;
                    ctx.notify();
                }
                Self::poll_hud_once(ctx, tick_rx, core);
            },
        );
    }

    fn start_event_listener(&self, ctx: &mut ViewContext<Self>) {
        let event_rx = Arc::clone(&self.desktop_event_rx);
        let settings = self.settings.clone();
        Self::poll_events_once(
            ctx,
            event_rx,
            settings,
            self.core.clone(),
            self.devices.clone(),
            self.chat.clone(),
        );
    }

    fn poll_events_once(
        ctx: &mut ViewContext<Self>,
        event_rx: Arc<
            tokio::sync::Mutex<
                tokio::sync::broadcast::Receiver<wormhole_desktop_core::DesktopEvent>,
            >,
        >,
        settings: ViewHandle<SettingsView>,
        core: CoreHandle,
        devices: ViewHandle<DevicesView>,
        chat: ViewHandle<ChatShellView>,
    ) {
        let rx = Arc::clone(&event_rx);
        ctx.spawn(
            async move {
                let mut guard = rx.lock().await;
                guard.recv().await
            },
            move |view, output, ctx| {
                match output {
                    Ok(event) => match event.name.as_str() {
                        "cloud-auth-changed" => {
                            view.refresh_auth_status(ctx);
                            view.refresh_auth_gated_views(ctx);
                            let settings_handle = settings.clone();
                            ctx.update_view(&settings_handle, |settings, ctx| {
                                settings.refresh_account(ctx);
                            });
                        }
                        "cluster-gossip-changed" => {
                            view.refresh_cluster_gossip_views(ctx, &devices, &chat, &core);
                        }
                        _ => {}
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        view.refresh_cluster_gossip_views(ctx, &devices, &chat, &core);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
                Self::poll_events_once(ctx, event_rx, settings, core, devices, chat);
            },
        );
    }

    fn refresh_cluster_gossip_views(
        &self,
        ctx: &mut ViewContext<Self>,
        devices: &ViewHandle<DevicesView>,
        chat: &ViewHandle<ChatShellView>,
        core: &CoreHandle,
    ) {
        ctx.update_view(devices, |devices, ctx| {
            devices.refresh_cluster(ctx);
        });
        ctx.update_view(chat, |chat, ctx| {
            chat.refresh_cluster_display(ctx);
        });
        let core = core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status_hud(&state).await
            },
            |view, output, ctx| {
                if let Ok(status) = output {
                    view.hud_nodes = status.nodes.len().max(1);
                    ctx.notify();
                }
            },
        );
    }

    fn start_warp_focus_poll(&self, ctx: &mut ViewContext<Self>) {
        let coordinator = Arc::clone(&self.coordinator);
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::poll_warp_focus_once(ctx, tick_rx, coordinator);
    }

    fn poll_warp_focus_once(
        ctx: &mut ViewContext<Self>,
        tick_rx: async_channel::Receiver<()>,
        coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
    ) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    if let Ok(mut guard) = coordinator.lock() {
                        if let Some(agent) = guard.take_pending_warp_focus() {
                            view.focus_warp_tab(agent, ctx);
                        }
                    }
                    Self::poll_warp_focus_once(ctx, tick_rx, coordinator);
                }
            },
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    fn start_tray_poll(&self, ctx: &mut ViewContext<Self>) {
        let tray = self.tray.clone();
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::poll_tray_once(ctx, tick_rx, tray);
    }

    #[cfg(any(windows, target_os = "linux"))]
    fn poll_tray_once(
        ctx: &mut ViewContext<Self>,
        tick_rx: async_channel::Receiver<()>,
        tray: std::sync::Arc<wormhole_desktop_tray::TrayController>,
    ) {
        use wormhole_desktop_tray::TrayAction;

        let waiter = tick_rx.clone();
        ctx.spawn(
            async move {
                let _ = waiter.recv().await;
            },
            move |_view, _, ctx| {
                if let Some(action) = tray.try_recv() {
                    match action {
                        TrayAction::Show => {
                            crate::ui::windows_shell::show_main_window_from_view(ctx);
                        }
                        TrayAction::Quit => {
                            crate::ui::windows_shell::quit_desktop(ctx);
                        }
                    }
                }
                Self::poll_tray_once(ctx, tick_rx, tray);
            },
        );
    }

    fn focus_warp_tab(&mut self, agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
        let warp_handle = self.warp.clone();
        ctx.update_view(&warp_handle, |view, ctx| {
            view.focus_with_agent(agent, ctx);
        });
        if self.tab != AppTab::Warp {
            self.tab = AppTab::Warp;
            self.tab_focus = AppTab::Warp;
            self.tab_bar_keyboard_focus = false;
            ctx.notify();
        }
    }

    fn tab_label(tab: AppTab) -> &'static str {
        match tab {
            AppTab::WDrive => "共享",
            AppTab::Sync => "同步",
            AppTab::Devices => "终端",
            AppTab::Display => "显示器",
            AppTab::Chat => "聊天",
            AppTab::Warp => "AI",
            AppTab::Toolbox => "工具箱",
            AppTab::Settings => "设置",
        }
    }

    fn visible_tabs() -> [AppTab; 5] {
        [
            AppTab::Devices,
            AppTab::Chat,
            AppTab::Warp,
            AppTab::Toolbox,
            AppTab::Settings,
        ]
    }

    fn tabs() -> [AppTab; 8] {
        [
            AppTab::WDrive,
            AppTab::Sync,
            AppTab::Devices,
            AppTab::Display,
            AppTab::Chat,
            AppTab::Warp,
            AppTab::Toolbox,
            AppTab::Settings,
        ]
    }

    fn adjacent_tab(current: AppTab, delta: i32) -> AppTab {
        let tabs = Self::visible_tabs();
        let idx = tabs.iter().position(|&t| t == current).unwrap_or(0);
        let len = tabs.len() as i32;
        let next = (idx as i32 + delta).rem_euclid(len) as usize;
        tabs[next]
    }

    pub(crate) fn tab_from_arrow(keystroke: &Keystroke, current: AppTab) -> Option<AppTab> {
        if keystroke.ctrl || keystroke.meta || keystroke.alt {
            return None;
        }
        match keystroke.key.as_str() {
            "left" => Some(Self::adjacent_tab(current, -1)),
            "right" => Some(Self::adjacent_tab(current, 1)),
            "home" => Some(Self::visible_tabs()[0]),
            "end" => Some(Self::visible_tabs()[4]),
            _ => None,
        }
    }

    fn tab_from_keystroke(keystroke: &Keystroke) -> Option<AppTab> {
        if !keystroke.ctrl {
            return None;
        }
        match keystroke.key.as_str() {
            "1" => Some(AppTab::Devices),
            "2" => Some(AppTab::Chat),
            "3" => Some(AppTab::Warp),
            "4" => Some(AppTab::Toolbox),
            "5" => Some(AppTab::Settings),
            _ => None,
        }
    }

    fn tab_requires_login_prompt(tab: AppTab) -> bool {
        matches!(
            tab,
            AppTab::Devices | AppTab::Chat | AppTab::Sync | AppTab::Display
        )
    }

    fn open_live_viewer_for_peer(&self, peer: &str, title: String, _ctx: &mut ViewContext<Self>) {
        let peer = peer.trim();
        if peer.is_empty() {
            return;
        }
        let window_key = wormhole_native_ipc::live_viewer_window_key(peer);
        if let Ok(mut guard) = self.coordinator.lock() {
            guard.enqueue(UiCommand::OpenLiveViewer {
                window_key,
                title,
                peer: peer.to_string(),
                password: None,
                totp_code: None,
                fps: 30,
            });
        }
    }

    fn open_remote_desktop_for_identity(
        &self,
        identity: &str,
        origin: RemoteDesktopOrigin,
        ctx: &mut ViewContext<Self>,
    ) {
        let identity = identity.trim().to_string();
        if identity.is_empty() {
            self.set_remote_desktop_result(origin, Err("当前终端缺少 P2P endpoint".into()), ctx);
            return;
        }
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                wormhole_desktop_core::cluster_commands::resolve_cluster_remote_desktop_target(
                    &runtime.state,
                    &identity,
                )
                .await
            },
            move |view, result, ctx| {
                let result = result.and_then(|target| view.enqueue_remote_desktop(target));
                view.set_remote_desktop_result(origin, result, ctx);
            },
        );
    }

    fn enqueue_remote_desktop(
        &self,
        target: wormhole_desktop_core::cluster_commands::ClusterRemoteDesktopTargetDto,
    ) -> Result<(), String> {
        let label = if target.hostname.trim().is_empty() {
            target.endpoint_id.chars().take(8).collect::<String>()
        } else if target.os.trim().is_empty() {
            target.hostname.clone()
        } else {
            format!("{} · {}", target.os, target.hostname)
        };
        let window_key = crate::wormhole_native_ipc::rdp_window_key(&target.endpoint_id);
        let mut guard = self
            .coordinator
            .lock()
            .map_err(|_| "远程桌面窗口协调器不可用".to_string())?;
        guard.enqueue(UiCommand::OpenRdp {
            peer: target.endpoint_addr,
            title: format!("远程桌面 · {label}"),
            reconnect: false,
            window_key,
            password: None,
            totp_code: None,
            fps: 60,
        });
        Ok(())
    }

    fn set_remote_desktop_result(
        &self,
        origin: RemoteDesktopOrigin,
        result: Result<(), String>,
        ctx: &mut ViewContext<Self>,
    ) {
        match origin {
            RemoteDesktopOrigin::Chat => {
                let chat = self.chat.clone();
                ctx.update_view(&chat, |chat, ctx| {
                    chat.set_remote_desktop_result(result, ctx);
                });
            }
            RemoteDesktopOrigin::Devices => {
                let devices = self.devices.clone();
                ctx.update_view(&devices, |devices, ctx| {
                    devices.set_remote_desktop_result(result, ctx);
                });
            }
        }
    }

    fn open_login_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.login_modal_open = true;
        let login = self.login_modal.clone();
        ctx.update_view(&login, |modal, ctx| {
            modal.open(ctx);
        });
    }

    /// Auth-gated tabs open the login dialog instead of sending users to Settings.
    fn prompt_login_if_needed(&mut self, ctx: &mut ViewContext<Self>) {
        if self.auth_authenticated || self.login_modal_open {
            return;
        }
        if !Self::tab_requires_login_prompt(self.tab) {
            return;
        }
        self.open_login_modal(ctx);
        ctx.notify();
    }

    fn refresh_auth_gated_views(&self, ctx: &mut ViewContext<Self>) {
        let devices = self.devices.clone();
        ctx.update_view(&devices, |devices, ctx| {
            devices.refresh_cluster(ctx);
        });
        let chat = self.chat.clone();
        ctx.update_view(&chat, |chat, ctx| {
            chat.poll_gate(ctx);
        });
        let sync = self.sync.clone();
        ctx.update_view(&sync, |sync, ctx| {
            sync.refresh(ctx);
        });
        let display = self.display.clone();
        ctx.update_view(&display, |display, ctx| {
            display.refresh(ctx);
        });
        ctx.dispatch_typed_action(&AppShellAction::RefreshCreditProducts);
    }

    fn persist_last_tab(&self) {
        let data_dir = self.core.data_dir();
        let tab_id = self.tab.persist_id().to_string();
        let _ = desktop_prefs::update(&data_dir, |prefs| {
            prefs.last_tab = Some(tab_id);
        });
    }

    fn refresh_auth_status(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let status = cloud_auth_status(&state).await;
                let prefs_email = crate::ui::desktop_prefs::load_login_prefs(&state.data_dir).email;
                (status, prefs_email)
            },
            |view, output, ctx| {
                let (status, prefs_email) = output;
                if let Ok(status) = status {
                    let auth_resolved = status.authenticated && !view.auth_authenticated;
                    view.auth_authenticated = status.authenticated;
                    view.auth_device_id = status.device_id;
                    view.auth_email = status.email.or(prefs_email);
                    if auth_resolved {
                        view.refresh_auth_gated_views(ctx);
                    } else if !status.authenticated {
                        let login = view.login_modal.clone();
                        let mut auto_started = false;
                        ctx.update_view(&login, |modal, ctx| {
                            auto_started = modal.try_auto_login(ctx);
                        });
                        if !auto_started {
                            view.prompt_login_if_needed(ctx);
                        }
                    }
                    ctx.notify();
                }
            },
        );
    }

    fn close_avatar_panel(&mut self, ctx: &mut ViewContext<Self>) {
        if self.avatar_panel_open {
            self.avatar_panel_open = false;
            ctx.notify();
        }
    }

    fn toggle_avatar_panel(&mut self, ctx: &mut ViewContext<Self>) {
        self.avatar_panel_open = !self.avatar_panel_open;
        if !self.avatar_panel_open {
            self.balance_feedback = None;
        } else {
            self.refresh_cloud_credit_balance(ctx);
        }
        ctx.notify();
    }

    fn purchase_balance(&mut self, ctx: &mut ViewContext<Self>) {
        self.avatar_panel_open = false;
        self.purchase_modal_open = true;
        self.purchase_feedback = None;
        ctx.notify();
        self.refresh_credit_products(ctx);
    }

    fn close_purchase_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.purchase_modal_open = false;
        self.purchase_feedback = None;
        ctx.notify();
    }

    fn refresh_cloud_credit_balance(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.auth_authenticated {
            return;
        }
        self.balance_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                wormhole_desktop_core::cloud_credits::cloud_credit_balance(&state).await
            },
            |view, output, ctx| {
                view.balance_busy = false;
                match output {
                    Ok(balance) => {
                        view.balance_credits = balance.balance_credits.max(0);
                        view.balance_amount_yuan = balance.balance_amount_yuan;
                    }
                    Err(err) => {
                        view.balance_feedback = Some(format!("余额同步失败: {err}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_credit_products(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.auth_authenticated {
            self.purchase_feedback = Some("请先登录 Wormhole 账户".into());
            ctx.notify();
            return;
        }
        self.purchase_busy = true;
        if self.purchase_products.is_empty() {
            self.purchase_feedback = None;
        }
        self.refresh_cloud_credit_balance(ctx);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                wormhole_desktop_core::cloud_credits::cloud_credit_products(&state).await
            },
            |view, output, ctx| {
                view.purchase_busy = false;
                match output {
                    Ok(products) => {
                        view.purchase_products = products.products;
                        view.purchase_feedback = None;
                        let product_ids = view
                            .purchase_products
                            .iter()
                            .map(|product| product.product_id.clone())
                            .collect::<Vec<_>>();
                        for product_id in product_ids {
                            view.queue_product_qr_load(product_id, ctx);
                        }
                    }
                    Err(err) => {
                        view.purchase_feedback = Some(format!("读取商品失败: {err}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn queue_product_qr_load(&mut self, product_id: String, ctx: &mut ViewContext<Self>) {
        if self.purchase_qr_loaded.contains(&product_id) {
            return;
        }
        let core = self.core.clone();
        ctx.spawn(
            {
                let product_id = product_id.clone();
                async move {
                    let state = core.runtime().state.clone();
                    let bytes = wormhole_desktop_core::cloud_credits::cloud_credit_product_qr_png(
                        &state,
                        &product_id,
                    )
                    .await?;
                    Ok::<_, String>((product_id, bytes))
                }
            },
            |view, output, ctx| {
                match output {
                    Ok((product_id, bytes)) => match Self::decode_qr_asset_payload(bytes) {
                        Ok(payload) => {
                            let asset_id = Self::product_qr_asset_id(&product_id);
                            AssetCache::handle(ctx).update(ctx, |cache, model_ctx| {
                                cache.insert_raw_asset_bytes::<ImageType>(
                                    asset_id, &payload, model_ctx,
                                );
                            });
                            view.purchase_qr_loaded.insert(product_id);
                        }
                        Err(err) => {
                            view.purchase_feedback = Some(format!("二维码解析失败: {err}"));
                        }
                    },
                    Err(err) => {
                        view.purchase_feedback = Some(format!("二维码同步失败: {err}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn decode_qr_asset_payload(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|err| err.to_string())?;
        let image = reader.decode().map_err(|err| err.to_string())?.to_rgb8();
        let (width, height) = image.dimensions();
        CustomImageHeader::prepend_custom_header(
            image.into_raw(),
            width,
            height,
            CustomImageFormat::Rgb,
        )
        .map_err(|err| format!("{err:?}"))
    }

    fn product_qr_asset_id(product_id: &str) -> String {
        format!("wormhole-credit-product-qr-{product_id}")
    }

    fn purchase_product_ui(&self) -> Vec<PurchaseProductUi> {
        self.purchase_products
            .iter()
            .map(|product| PurchaseProductUi {
                product_id: product.product_id.clone(),
                label: product.label.clone(),
                pay_url: product.pay_url.clone(),
                qr_asset_id: Self::product_qr_asset_id(&product.product_id),
                qr_loaded: self.purchase_qr_loaded.contains(&product.product_id),
            })
            .collect()
    }

    fn copy_purchase_link(&mut self, link: &str, ctx: &mut ViewContext<Self>) {
        match write_clipboard_text(link) {
            Ok(()) => self.purchase_feedback = Some("支付链接已复制".into()),
            Err(err) => self.purchase_feedback = Some(format!("复制失败: {err}")),
        }
        ctx.notify();
    }

    fn open_purchase_link(&mut self, link: &str, ctx: &mut ViewContext<Self>) {
        match open_external_url(link) {
            Ok(()) => self.purchase_feedback = Some("已打开支付链接".into()),
            Err(err) => self.purchase_feedback = Some(format!("打开失败: {err}")),
        }
        ctx.notify();
    }

    fn open_redeem_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.avatar_panel_open = false;
        self.redeem_modal_open = true;
        self.redeem_tab = RedeemTab::Redeem;
        self.redeem_feedback = None;
        self.redeem_feedback_tone = StatusTone::Neutral;
        self.redeem_code_focused = true;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn close_redeem_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.redeem_modal_open = false;
        self.redeem_tab = RedeemTab::Redeem;
        self.redeem_code_draft.clear();
        self.redeem_code_field.clear_marked();
        self.redeem_code_focused = false;
        self.redeem_feedback = None;
        self.redeem_feedback_tone = StatusTone::Neutral;
        self.redeem_busy = false;
        self.redeem_history_busy = false;
        self.redeem_history_error = None;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn switch_redeem_tab(&mut self, tab: RedeemTab, ctx: &mut ViewContext<Self>) {
        self.redeem_tab = tab;
        if tab == RedeemTab::Redeem {
            self.redeem_code_focused = true;
            sync_caret_blink(self, ctx);
        } else {
            self.redeem_code_focused = false;
            sync_caret_blink(self, ctx);
            self.refresh_redeem_history(ctx);
            return;
        }
        ctx.notify();
    }

    fn refresh_redeem_history(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.auth_authenticated {
            self.redeem_history.clear();
            self.redeem_history_busy = false;
            self.redeem_history_error = Some("请先登录后查看兑换记录".into());
            ctx.notify();
            return;
        }
        if self.redeem_history_busy {
            return;
        }
        self.redeem_history_busy = true;
        self.redeem_history_error = None;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                wormhole_desktop_core::cloud_credits::cloud_credit_ledger(&state, 50).await
            },
            |view, output, ctx| {
                view.redeem_history_busy = false;
                match output {
                    Ok(ledger) => {
                        view.redeem_history = redeem_history_from_ledger(
                            &ledger.entries,
                            &view.redeem_code_overrides,
                        );
                        view.redeem_history_error = None;
                    }
                    Err(err) => {
                        view.redeem_history.clear();
                        view.redeem_history_error = Some(err);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn persist_redeem_code_override(&self, card_key_id: &str, code: &str) {
        let mut overrides = self.redeem_code_overrides.clone();
        overrides.insert(card_key_id.to_string(), code.to_string());
        let data_dir = self.core.data_dir();
        if let Err(err) = desktop_prefs::update(&data_dir, |prefs| {
            prefs.redeem_code_overrides = overrides;
        }) {
            tracing::warn!("无法保存兑换码缓存: {err}");
        }
    }

    fn focus_redeem_code(&mut self, ctx: &mut ViewContext<Self>) {
        self.redeem_code_focused = true;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn edit_redeem_code(&mut self, edit: &TextFieldEditAction, ctx: &mut ViewContext<Self>) {
        self.redeem_code_field
            .apply(&mut self.redeem_code_draft, edit);
        self.redeem_code_focused = true;
        self.redeem_feedback = None;
        self.redeem_feedback_tone = StatusTone::Neutral;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn submit_redeem(&mut self, ctx: &mut ViewContext<Self>) {
        if self.redeem_busy {
            return;
        }
        let code = self.redeem_code_draft.trim().to_string();
        if code.is_empty() {
            self.redeem_feedback = Some("请输入兑换码。".into());
            self.redeem_feedback_tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        self.redeem_busy = true;
        self.redeem_feedback = Some("正在验证卡密…".into());
        self.redeem_feedback_tone = StatusTone::Neutral;
        sync_caret_blink(self, ctx);
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                wormhole_desktop_core::cloud_credits::cloud_credit_redeem_card_key(
                    &state,
                    CloudCreditRedeemRequest { code },
                )
                .await
            },
            |view, output, ctx| {
                view.redeem_busy = false;
                match output {
                    Ok(redeem) => {
                        let redeemed_code = view.redeem_code_draft.trim().to_string();
                        if let Some(card_key_id) = redeem
                            .card_key_id
                            .as_ref()
                            .map(|value| value.trim())
                            .filter(|value| !value.is_empty())
                        {
                            view.redeem_code_overrides
                                .insert(card_key_id.to_string(), redeemed_code.clone());
                            view.persist_redeem_code_override(card_key_id, &redeemed_code);
                        }
                        view.balance_credits = redeem.balance_credits.max(0);
                        view.balance_amount_yuan = redeem.balance_amount_yuan.clone();
                        view.redeem_modal_open = false;
                        view.redeem_tab = RedeemTab::Redeem;
                        view.redeem_code_draft.clear();
                        view.redeem_code_field.clear_marked();
                        view.redeem_code_focused = false;
                        view.redeem_feedback = None;
                        view.redeem_feedback_tone = StatusTone::Neutral;
                        view.balance_feedback = Some(format!(
                            "兑换成功 +{} · 当前 {}",
                            desktop_prefs::format_balance_display(
                                redeem.amount_added_yuan.as_deref(),
                                redeem.credits_added,
                            ),
                            desktop_prefs::format_balance_display(
                                redeem.balance_amount_yuan.as_deref(),
                                redeem.balance_credits,
                            ),
                        ));
                        view.avatar_panel_open = true;
                    }
                    Err(err) => {
                        view.redeem_feedback = Some(err);
                        view.redeem_feedback_tone = StatusTone::Danger;
                    }
                }
                sync_caret_blink(view, ctx);
                ctx.notify();
            },
        );
    }

    fn tab_button(&self, tab: AppTab) -> Box<dyn Element> {
        let selected = self.tab == tab;
        let keyboard_focused = self.tab_bar_keyboard_focus && self.tab_focus == tab;
        let mono = self.mono;
        let label = Self::tab_label(tab);
        let mouse_state = self.tab_hover_mouse_states.for_tab(tab);

        Hoverable::new(mouse_state, move |state| {
            let hovered = state.is_hovered();
            let text_color = if selected || hovered {
                theme::accent_cool()
            } else {
                theme::muted()
            };
            let bg = if selected {
                theme::accent_cool_bg_default()
            } else if hovered {
                theme::accent_cool_bg(10)
            } else {
                ColorU::new(0, 0, 0, 0)
            };

            let bottom_accent = if selected {
                theme::accent_cool()
            } else {
                ColorU::transparent_black()
            };

            let content_height = icons::TAB_ICON_SIZE;
            let bottom_border = 2.0;
            let vertical_pad =
                ((CHROME_ROW_HEIGHT - content_height - bottom_border) / 2.0).max(0.0);

            let mut container = Container::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_child(icons::tab_button_content(
                        tab, false, text_color, label, mono,
                    ))
                    .finish(),
            )
            .with_vertical_padding(vertical_pad)
            .with_horizontal_padding(14.0)
            .with_background(bg)
            .with_border(Border::bottom(2.0).with_border_fill(bottom_accent))
            .with_border(Border::right(1.0).with_border_fill(theme::border()));
            if keyboard_focused {
                container = container
                    .with_border(Border::all(2.0).with_border_color(theme::accent_cool()));
            }
            let button = container.finish();

            if !hovered {
                return button;
            }

            let tooltip = Container::new(
                ui_text::cluster_ctrl(label, mono)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_vertical_padding(5.0)
            .with_horizontal_padding(8.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .finish();

            let mut stack = Stack::new();
            stack.add_child(button);
            stack.add_positioned_overlay_child(
                tooltip,
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, 6.0),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::BottomMiddle,
                    ChildAnchor::TopMiddle,
                ),
            );
            stack.finish()
        })
        .on_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(AppShellAction::SelectTab(tab, TabSelectSource::Mouse));
        })
        .finish()
    }

    fn tab_bar(&self, app: &AppContext) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        for tab in Self::visible_tabs() {
            row.add_child(self.tab_button(tab));
        }
        let scrollable_tabs = ConstrainedBox::new(
            ClippedScrollable::horizontal(
                self.tab_scroll.clone(),
                row.finish(),
                ScrollbarWidth::None,
                Fill::None,
                Fill::None,
                Fill::None,
            )
            .finish(),
        )
        .with_height(CHROME_ROW_HEIGHT)
        .finish();

        let zoom_factor = 1.0;
        let traffic_light_data = window_chrome::traffic_light_data(app, self.window_id);
        let is_fullscreen = app
            .windows()
            .platform_window(self.window_id)
            .map(|window| window.fullscreen_state() != warpui::platform::FullscreenState::Normal)
            .unwrap_or(false);
        let left_padding = window_chrome::tab_bar_left_padding(
            traffic_light_data.as_ref(),
            zoom_factor,
            is_fullscreen,
        );

        let mut tab_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);

        if left_padding > 0.0 {
            tab_row.add_child(window_chrome::left_padding_spacer(left_padding));
        }

        tab_row.add_child(Shrinkable::new(1.0, scrollable_tabs).finish());
        tab_row.add_child(Expanded::new(1.0, Empty::new().finish()).finish());
        tab_row.add_child(build_avatar_slot(
            self.auth_authenticated,
            self.auth_email.as_deref(),
            self.avatar_panel_open,
            self.font,
        ));

        if let Some(data) = traffic_light_data.as_ref() {
            if window_chrome::traffic_lights_inline_in_tab_bar() {
                tab_row.add_child(window_chrome::render_traffic_lights(
                    self.window_id,
                    app,
                    &self.traffic_light_mouse_states,
                    TrafficLightActions {
                        minimize: AppShellAction::MinimizeWindow,
                        toggle_maximize: AppShellAction::ToggleMaximizeWindow,
                        close: AppShellAction::CloseWindow,
                    },
                    self.font,
                ));
            } else if let Some(spacer) = window_chrome::traffic_light_spacer(data, zoom_factor) {
                tab_row.add_child(spacer);
            }
        }

        ConstrainedBox::new(
            Container::new(tab_row.finish())
                .with_background(theme::panel_elevated())
                .with_border(Border::bottom(1.0).with_border_fill(theme::border_bright()))
                .with_horizontal_padding(4.0)
                .finish(),
        )
        .with_height(CHROME_ROW_HEIGHT)
        .finish()
    }

    fn onboarding_chip(&self, label: &str, tab: AppTab) -> Box<dyn Element> {
        let text = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(text, self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(AppShellAction::SelectTab(tab, TabSelectSource::Mouse));
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(theme::accent_bg(24))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish()
    }

    fn onboarding_banner(&self) -> Option<Box<dyn Element>> {
        if !self.show_onboarding {
            return None;
        }
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
        col.add_child(
            ui_text::title("快速开始", self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(section_hint(
            "约 2 分钟：浏览终端共享、P2P 聊天、AI 任务。可随时跳过。",
            self.font,
        ));
        let mut steps = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        steps.add_child(self.onboarding_chip("1 · 终端", AppTab::Devices));
        steps.add_child(self.onboarding_chip("2 · 聊天", AppTab::Chat));
        steps.add_child(self.onboarding_chip("3 · AI", AppTab::Warp));
        steps.add_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("稍后再说", self.font)
                        .with_color(theme::placeholder())
                        .finish(),
                )
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(AppShellAction::DismissOnboarding);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_uniform_padding(10.0)
            .finish(),
        );
        col.add_child(steps.finish());
        Some(
            Container::new(col.finish())
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::accent_cool()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                .with_uniform_padding(12.0)
                .finish(),
        )
    }

    fn body(&self, app: &AppContext) -> Box<dyn Element> {
        let content: Box<dyn Element> = match self.tab {
            AppTab::WDrive => ChildView::new(&self.w_drive).finish(),
            AppTab::Sync => ChildView::new(&self.sync).finish(),
            AppTab::Devices => ChildView::new(&self.devices).finish(),
            AppTab::Display => ChildView::new(&self.display).finish(),
            AppTab::Chat => ChildView::new(&self.chat).finish(),
            AppTab::Warp => ChildView::new(&self.warp).finish(),
            AppTab::Toolbox => ChildView::new(&self.toolbox).finish(),
            AppTab::Settings => ChildView::new(&self.settings).finish(),
        };
        let mut column = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(self.tab_bar(app));
        if let Some(banner) = self.onboarding_banner() {
            column.add_child(Container::new(banner).with_uniform_padding(8.0).finish());
        }
        column.add_child(Expanded::new(1.0, HudBackdrop::live(tab_content_fill(content))).finish());
        column.finish()
    }
}

impl CaretBlinkHost for AppShellView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.redeem_caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.redeem_modal_open && self.redeem_code_focused
    }
}

impl Entity for AppShellView {
    type Event = ();
}

impl View for AppShellView {
    fn ui_name() -> &'static str {
        "AppShellView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let current_tab = self.tab;
        let login_modal_open = self.login_modal_open;
        let avatar_panel_open = self.avatar_panel_open;
        let redeem_modal_open = self.redeem_modal_open;
        let purchase_modal_open = self.purchase_modal_open;
        let shell = Container::new(self.body(app))
            .with_background(theme::canvas())
            .with_uniform_padding(0.0)
            .finish();
        let shell = EventHandler::new(shell)
            .with_always_handle()
            .on_keydown(move |ctx, _, keystroke| {
                if login_modal_open {
                    return DispatchEventResult::PropagateToParent;
                }
                if keystroke.key.as_str() == "escape" {
                    if redeem_modal_open {
                        ctx.dispatch_typed_action(AppShellAction::CloseRedeemModal);
                        return DispatchEventResult::StopPropagation;
                    }
                    if purchase_modal_open {
                        ctx.dispatch_typed_action(AppShellAction::ClosePurchaseModal);
                        return DispatchEventResult::StopPropagation;
                    }
                    if avatar_panel_open {
                        ctx.dispatch_typed_action(AppShellAction::CloseAvatarPanel);
                        return DispatchEventResult::StopPropagation;
                    }
                }
                if let Some(tab) = Self::tab_from_keystroke(keystroke) {
                    ctx.dispatch_typed_action(AppShellAction::SelectTab(
                        tab,
                        TabSelectSource::Keyboard,
                    ));
                    return DispatchEventResult::StopPropagation;
                }
                if !login_modal_open {
                    if let Some(tab) = Self::tab_from_arrow(keystroke, current_tab) {
                        ctx.dispatch_typed_action(AppShellAction::SelectTab(
                            tab,
                            TabSelectSource::Keyboard,
                        ));
                        return DispatchEventResult::StopPropagation;
                    }
                }
                DispatchEventResult::PropagateToParent
            })
            .finish();

        let mut stack = Stack::new();
        stack.add_child(shell);
        let zoom_factor = 1.0;
        let traffic_light_data = window_chrome::traffic_light_data(app, self.window_id);

        if self.avatar_panel_open
            && self.auth_authenticated
            && !self.redeem_modal_open
            && !self.purchase_modal_open
        {
            stack.add_child(
                EventHandler::new(Container::new(Flex::column().finish()).finish())
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(AppShellAction::CloseAvatarPanel);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            );
            let panel_right_inset = traffic_light_data
                .as_ref()
                .map(|data| data.width(zoom_factor) + 8.0)
                .unwrap_or(8.0);
            stack.add_positioned_child(
                build_avatar_panel(
                    self.auth_device_id.as_deref(),
                    self.balance_credits,
                    self.balance_amount_yuan.as_deref(),
                    self.balance_busy,
                    self.balance_feedback.as_deref(),
                    self.font,
                    self.mono,
                ),
                OffsetPositioning::offset_from_parent(
                    vec2f(-panel_right_inset, CHROME_ROW_HEIGHT + 8.0),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
        }
        if self.redeem_modal_open {
            stack.add_child(build_redeem_modal(
                self.redeem_tab,
                &self.redeem_code_draft,
                &self.redeem_code_field.marked_text,
                self.redeem_code_focused,
                self.redeem_caret_blink.visible,
                self.redeem_feedback.as_deref(),
                self.redeem_feedback_tone,
                self.redeem_busy,
                self.redeem_history_busy,
                self.redeem_history_error.as_deref(),
                &self.redeem_history,
                &self.redeem_history_scroll,
                self.font,
                self.mono,
            ));
        }
        if self.purchase_modal_open {
            let products = self.purchase_product_ui();
            stack.add_child(build_purchase_modal(
                &products,
                self.purchase_busy,
                self.purchase_feedback.as_deref(),
                self.font,
                self.mono,
            ));
        }
        stack.add_child(ChildView::new(&self.login_modal).finish());

        if traffic_light_data
            .as_ref()
            .is_some_and(|data| data.side == window_chrome::TrafficLightSide::Right)
            && !window_chrome::traffic_lights_inline_in_tab_bar()
        {
            stack.add_positioned_child(
                window_chrome::render_traffic_lights(
                    self.window_id,
                    app,
                    &self.traffic_light_mouse_states,
                    TrafficLightActions {
                        minimize: AppShellAction::MinimizeWindow,
                        toggle_maximize: AppShellAction::ToggleMaximizeWindow,
                        close: AppShellAction::CloseWindow,
                    },
                    self.font,
                ),
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, 0.0),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        stack.finish()
    }

    fn accessibility_contents(&self, _app: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
            format!("Wormhole，当前标签：{}", Self::tab_label(self.tab)),
            "Ctrl 加数字 1 到 5 切换标签。非输入焦点时左右方向键切换相邻标签；文本框内方向键不切换标签。Home 与 End 跳到首尾标签。",
            WarpA11yRole::WindowRole,
        ))
    }

    fn accessibility_data(&self, _ctx: &mut ViewContext<Self>) -> Option<AccessibilityData> {
        Some(AccessibilityData {
            content: format!("Wormhole 主窗口，{}", Self::tab_label(self.tab)),
        })
    }
}

impl TypedActionView for AppShellView {
    type Action = AppShellAction;

    fn handle_action(&mut self, action: &AppShellAction, ctx: &mut ViewContext<Self>) {
        match action {
            AppShellAction::SelectTab(tab, source) => {
                let warp_visible = *tab == AppTab::Warp;
                let warp_handle = self.warp.clone();
                ctx.update_view(&warp_handle, |view, ctx| {
                    view.set_tab_visible(warp_visible, ctx);
                });
                self.tab = *tab;
                self.tab_focus = *tab;
                self.tab_bar_keyboard_focus = *source == TabSelectSource::Keyboard;
                self.persist_last_tab();
                self.prompt_login_if_needed(ctx);
                ctx.notify();
            }
            AppShellAction::DismissOnboarding => {
                self.show_onboarding = false;
                let data_dir = self.core.data_dir();
                let _ = desktop_prefs::update(&data_dir, |prefs| {
                    prefs.onboarding_dismissed = true;
                });
                ctx.notify();
            }
            AppShellAction::MinimizeWindow => {
                if let Some(window) = ctx.windows().platform_window(ctx.window_id()) {
                    window.minimize();
                }
            }
            AppShellAction::ToggleMaximizeWindow => {
                if let Some(window) = ctx.windows().platform_window(ctx.window_id()) {
                    window.toggle_maximized();
                }
            }
            AppShellAction::CloseWindow => {
                self.persist_last_tab();
                #[cfg(any(windows, target_os = "linux"))]
                crate::ui::windows_shell::hide_main_window_from_view(ctx);
                #[cfg(not(any(windows, target_os = "linux")))]
                ctx.close_window();
            }
            AppShellAction::OpenLogin => {
                self.open_login_modal(ctx);
                ctx.notify();
            }
            AppShellAction::ToggleAvatarPanel => self.toggle_avatar_panel(ctx),
            AppShellAction::CloseAvatarPanel => self.close_avatar_panel(ctx),
            AppShellAction::PurchaseBalance => self.purchase_balance(ctx),
            AppShellAction::ClosePurchaseModal => self.close_purchase_modal(ctx),
            AppShellAction::RefreshCreditProducts => self.refresh_credit_products(ctx),
            AppShellAction::CopyPurchaseLink(link) => self.copy_purchase_link(link, ctx),
            AppShellAction::OpenPurchaseLink(link) => self.open_purchase_link(link, ctx),
            AppShellAction::OpenRedeemModal => self.open_redeem_modal(ctx),
            AppShellAction::CloseRedeemModal => self.close_redeem_modal(ctx),
            AppShellAction::SwitchRedeemTab(tab) => self.switch_redeem_tab(*tab, ctx),
            AppShellAction::RedeemCodeEdit(edit) => self.edit_redeem_code(edit, ctx),
            AppShellAction::FocusRedeemCode => self.focus_redeem_code(ctx),
            AppShellAction::SubmitRedeem => self.submit_redeem(ctx),
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &AppShellAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let content = match action {
            AppShellAction::SelectTab(tab, _) => AccessibilityContent::new_without_help(
                format!("切换到{}", Self::tab_label(*tab)),
                WarpA11yRole::MenuItemRole,
            ),
            AppShellAction::DismissOnboarding => {
                AccessibilityContent::new_without_help("关闭快速开始引导", WarpA11yRole::ButtonRole)
            }
            AppShellAction::MinimizeWindow => {
                AccessibilityContent::new_without_help("最小化窗口", WarpA11yRole::ButtonRole)
            }
            AppShellAction::ToggleMaximizeWindow => {
                AccessibilityContent::new_without_help("切换最大化", WarpA11yRole::ButtonRole)
            }
            AppShellAction::CloseWindow => {
                AccessibilityContent::new_without_help("隐藏到系统托盘", WarpA11yRole::ButtonRole)
            }
            AppShellAction::OpenLogin => {
                AccessibilityContent::new_without_help("打开登录", WarpA11yRole::ButtonRole)
            }
            AppShellAction::ToggleAvatarPanel => AccessibilityContent::new_without_help(
                "切换账户与余额面板",
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::CloseAvatarPanel => AccessibilityContent::new_without_help(
                "关闭账户与余额面板",
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::PurchaseBalance => {
                AccessibilityContent::new_without_help("购买余额", WarpA11yRole::ButtonRole)
            }
            AppShellAction::ClosePurchaseModal => {
                AccessibilityContent::new_without_help("关闭购买", WarpA11yRole::ButtonRole)
            }
            AppShellAction::RefreshCreditProducts => {
                AccessibilityContent::new_without_help("刷新购买商品", WarpA11yRole::ButtonRole)
            }
            AppShellAction::CopyPurchaseLink(_) => {
                AccessibilityContent::new_without_help("复制支付链接", WarpA11yRole::ButtonRole)
            }
            AppShellAction::OpenPurchaseLink(_) => {
                AccessibilityContent::new_without_help("打开支付链接", WarpA11yRole::ButtonRole)
            }
            AppShellAction::OpenRedeemModal => {
                AccessibilityContent::new_without_help("打开兑换", WarpA11yRole::ButtonRole)
            }
            AppShellAction::CloseRedeemModal => {
                AccessibilityContent::new_without_help("关闭兑换", WarpA11yRole::ButtonRole)
            }
            AppShellAction::SwitchRedeemTab(tab) => AccessibilityContent::new_without_help(
                match tab {
                    RedeemTab::Redeem => "切换到兑换余额",
                    RedeemTab::History => "切换到兑换记录",
                },
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::RedeemCodeEdit(_) => {
                AccessibilityContent::new_without_help("编辑兑换码", WarpA11yRole::TextfieldRole)
            }
            AppShellAction::FocusRedeemCode => AccessibilityContent::new_without_help(
                "聚焦兑换码输入",
                WarpA11yRole::TextfieldRole,
            ),
            AppShellAction::SubmitRedeem => {
                AccessibilityContent::new_without_help("确定兑换", WarpA11yRole::ButtonRole)
            }
        };
        ActionAccessibilityContent::Custom(content)
    }
}

fn open_external_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("支付链接不是有效的 HTTP URL".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持自动打开链接，请复制后在浏览器打开".into())
}

#[cfg(test)]
mod tests {
    use super::{AppShellView, AppTab};
    use warpui_core::keymap::Keystroke;

    fn key(key: &str) -> Keystroke {
        Keystroke {
            key: key.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn tab_persist_id_roundtrip() {
        let tabs = [
            AppTab::Devices,
            AppTab::Chat,
            AppTab::Warp,
            AppTab::Toolbox,
            AppTab::Settings,
        ];
        for tab in tabs {
            let id = tab.persist_id();
            assert_eq!(AppTab::from_persist_id(id), Some(tab));
        }
        assert_eq!(AppTab::from_persist_id("terminals"), Some(AppTab::Devices));
        assert_eq!(AppTab::from_persist_id("w_drive"), Some(AppTab::Devices));
        assert_eq!(AppTab::from_persist_id("agent"), Some(AppTab::Warp));
    }

    #[test]
    fn tab_requires_login_prompt_for_cluster_features() {
        assert!(AppShellView::tab_requires_login_prompt(AppTab::Devices));
        assert!(AppShellView::tab_requires_login_prompt(AppTab::Chat));
        assert!(!AppShellView::tab_requires_login_prompt(AppTab::Settings));
        assert!(!AppShellView::tab_requires_login_prompt(AppTab::Toolbox));
    }

    #[test]
    fn tab_from_arrow_cycles_visible_tabs() {
        assert_eq!(
            AppShellView::tab_from_arrow(&key("right"), AppTab::Devices),
            Some(AppTab::Chat)
        );
        assert_eq!(
            AppShellView::tab_from_arrow(&key("left"), AppTab::Devices),
            Some(AppTab::Settings)
        );
        assert_eq!(
            AppShellView::tab_from_arrow(&key("home"), AppTab::Chat),
            Some(AppTab::Devices)
        );
        assert_eq!(
            AppShellView::tab_from_arrow(&key("end"), AppTab::Chat),
            Some(AppTab::Settings)
        );
    }

    #[test]
    fn tab_from_arrow_ignores_modified_keys() {
        let mut ks = key("right");
        ks.ctrl = true;
        assert_eq!(AppShellView::tab_from_arrow(&ks, AppTab::Devices), None);
    }
}
