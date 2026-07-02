use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use std::sync::Arc;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, ChildAnchor, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, Empty, EventHandler, Expanded,
    Fill, Flex, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds,
    Radius, ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{
    AccessibilityData, AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext,
    ViewHandle, WindowId,
};
use warpui_core::keymap::Keystroke;

use crate::coordinator::{CoordinatorState, CoordinatorView};
use crate::ui::agent_panel::AgentPanelView;
use crate::ui::chat::ChatShellView;
use crate::ui::codex_provider_import_model::SharedCodexProviderImportModel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::clipboard::read_clipboard_text;
use crate::ui::desktop_prefs::{self, format_balance_yuan};
use crate::ui::hud_avatar_panel::{self, build_avatar_panel, build_avatar_slot, build_redeem_modal};
use crate::ui::devices_view::DevicesView;
use crate::ui::display_view::DisplayView;
use crate::ui::hud_effects::HudBackdrop;
use crate::ui::icons;
use crate::ui::login_modal::{LoginModalAction, LoginModalEvent, LoginModalView};
use crate::ui::panel_primitives::{section_hint, tab_content_fill, HUD_RADIUS};
use crate::ui::settings_view::{SettingsEvent, SettingsView};
use crate::ui::sync_views::SyncView;
use crate::ui::theme;
use crate::ui::toolbox_view::ToolboxView;
use crate::ui::w_drive_view::WDriveView;
use crate::ui::window_chrome::{
    self, TrafficLightActions, TrafficLightMouseStates, CHROME_ROW_HEIGHT,
};
use crate::ui_text;
use wormhole_desktop_core::cloud_auth_status;
use wormhole_desktop_core::cluster_commands::{cluster_status, cluster_status_fast};
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

#[derive(Debug, Clone, Copy)]
pub enum AppShellAction {
    SelectTab(AppTab, TabSelectSource),
    SetTabHover(Option<AppTab>),
    DismissOnboarding,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
    OpenLogin,
    ToggleAvatarPanel,
    CloseAvatarPanel,
    PurchaseBalance,
    OpenRedeemModal,
    CloseRedeemModal,
    PasteRedeemCode,
    SubmitRedeem,
}

pub struct AppShellView {
    tab: AppTab,
    hovered_tab: Option<AppTab>,
    tab_focus: AppTab,
    tab_bar_keyboard_focus: bool,
    core: CoreHandle,
    coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
    #[allow(dead_code)]
    coordinator_view: ViewHandle<CoordinatorView>,
    w_drive: ViewHandle<WDriveView>,
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
    font: FamilyId,
    mono: FamilyId,
    hud_nodes: usize,
    device_ready: bool,
    avatar_panel_open: bool,
    balance_cents: i64,
    balance_feedback: Option<String>,
    redeem_modal_open: bool,
    redeem_code_draft: String,
    redeem_feedback: Option<String>,
    tab_scroll: ClippedScrollStateHandle,
    show_onboarding: bool,
    window_id: WindowId,
    traffic_light_mouse_states: TrafficLightMouseStates,
    #[cfg(windows)]
    tray: std::sync::Arc<wormhole_desktop_platform_windows::TrayController>,
}

impl AppShellView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
        import_model: SharedCodexProviderImportModel,
        pending_deeplink: Option<String>,
        #[cfg(windows)] tray: std::sync::Arc<wormhole_desktop_platform_windows::TrayController>,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let coordinator_view =
            ctx.add_typed_action_view(|ctx| CoordinatorView::new(ctx, coordinator.clone()));
        #[cfg(windows)]
        crate::ui::windows_shell::register_main_shell_window(ctx.window_id(), &coordinator);
        let w_drive = ctx.add_view(|ctx| WDriveView::new(ctx, core.clone()));
        let sync = ctx.add_typed_action_view(|ctx| SyncView::new(ctx, core.clone()));
        let devices = ctx.add_typed_action_view(|ctx| DevicesView::new(ctx, core.clone()));
        let display = ctx.add_view(|ctx| DisplayView::new(ctx, core.clone()));
        let chat = ctx.add_view(|ctx| ChatShellView::new(ctx, core.clone()));
        let warp = ctx.add_typed_action_view(|ctx| AgentPanelView::new(ctx, core.clone()));
        let toolbox = ctx
            .add_typed_action_view(|ctx| ToolboxView::new(ctx, core.clone(), coordinator.clone()));
        let settings =
            ctx.add_typed_action_view(|ctx| SettingsView::new(ctx, core.clone(), import_model));
        let login_modal = ctx.add_typed_action_view(|ctx| LoginModalView::new(ctx, core.clone()));
        ctx.subscribe_to_view(&login_modal, |view, _, event, ctx| {
            match event {
                LoginModalEvent::AuthChanged {
                    authenticated,
                    device_id,
                } => {
                    view.auth_authenticated = *authenticated;
                    view.auth_device_id = device_id.clone();
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
                        view.device_ready = false;
                        view.prompt_login_if_needed(ctx);
                    }
                    view.refresh_auth_gated_views(ctx);
                }
                SettingsEvent::OpenLogin => {
                    view.open_login_modal(ctx);
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
        if let Some(ref url) = pending_deeplink {
            tab = AppTab::Settings;
            let settings_handle = settings.clone();
            ctx.update_view(&settings_handle, |view, ctx| {
                view.open_deeplink_url(url.clone(), ctx)
            });
        }
        let show_onboarding = pending_deeplink.is_none() && !prefs.onboarding_dismissed;
        let window_id = ctx.window_id();
        let view = Self {
            tab,
            hovered_tab: None,
            tab_focus: tab,
            tab_bar_keyboard_focus: false,
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
            font,
            mono,
            hud_nodes: 1,
            device_ready: false,
            avatar_panel_open: false,
            balance_cents: prefs.balance_cents,
            balance_feedback: None,
            redeem_modal_open: false,
            redeem_code_draft: String::new(),
            redeem_feedback: None,
            tab_scroll: ClippedScrollStateHandle::new(),
            show_onboarding,
            window_id,
            traffic_light_mouse_states: TrafficLightMouseStates::default(),
            #[cfg(windows)]
            tray,
        };
        view.start_warp_focus_poll(ctx);
        view.start_hud_poll(ctx);
        view.refresh_auth_status(ctx);
        view.start_deeplink_listener(ctx);
        #[cfg(windows)]
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
                cluster_status_fast(&state).await
            },
            move |view, output, ctx| {
                if let Ok(status) = output {
                    view.hud_nodes = status.nodes.len().max(1);
                    let auth_now = !status.auth_required;
                    let auth_changed = auth_now != view.auth_authenticated;
                    view.auth_authenticated = auth_now;
                    if !auth_now {
                        view.auth_device_id = None;
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

    fn start_deeplink_listener(&self, ctx: &mut ViewContext<Self>) {
        let events = self.core.runtime().ctx.events.clone();
        let settings = self.settings.clone();
        Self::poll_deeplink_once(ctx, events, settings);
    }

    fn poll_deeplink_once(
        ctx: &mut ViewContext<Self>,
        events: wormhole_desktop_core::DesktopEventBus,
        settings: ViewHandle<SettingsView>,
    ) {
        let mut rx = events.subscribe();
        ctx.spawn(async move { rx.recv().await }, move |view, output, ctx| {
            if let Ok(event) = output {
                match event.name.as_str() {
                    "deeplink-import" => {
                        if let Some(url) = event.payload.get("url").and_then(|v| v.as_str()) {
                            let url = url.to_string();
                            ctx.update_view(&settings, |view, ctx| {
                                view.open_deeplink_url(url, ctx);
                            });
                        }
                    }
                    "cloud-auth-changed" => {
                        view.refresh_auth_status(ctx);
                        view.refresh_auth_gated_views(ctx);
                        let settings_handle = settings.clone();
                        ctx.update_view(&settings_handle, |settings, ctx| {
                            settings.refresh_account(ctx);
                        });
                    }
                    _ => {}
                }
            }
            Self::poll_deeplink_once(ctx, events, settings);
        });
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

    #[cfg(windows)]
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

    #[cfg(windows)]
    fn poll_tray_once(
        ctx: &mut ViewContext<Self>,
        tick_rx: async_channel::Receiver<()>,
        tray: std::sync::Arc<wormhole_desktop_platform_windows::TrayController>,
    ) {
        use wormhole_desktop_platform_windows::TrayAction;

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
            AppTab::Warp => "智能体",
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
                cloud_auth_status(&state).await
            },
            |view, output, ctx| {
                if let Ok(status) = output {
                    let auth_resolved = status.authenticated && !view.auth_authenticated;
                    view.auth_authenticated = status.authenticated;
                    view.auth_device_id = status.device_id;
                    if auth_resolved {
                        view.refresh_auth_gated_views(ctx);
                    } else if !status.authenticated {
                        view.prompt_login_if_needed(ctx);
                    }
                    ctx.notify();
                }
            },
        );
    }

    fn persist_balance_cents(&self, cents: i64) {
        let data_dir = self.core.data_dir();
        let _ = desktop_prefs::update(&data_dir, |prefs| {
            prefs.balance_cents = cents.max(0);
        });
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
        }
        ctx.notify();
    }

    fn purchase_balance(&mut self, ctx: &mut ViewContext<Self>) {
        self.balance_cents = self
            .balance_cents
            .saturating_add(hud_avatar_panel::purchase_add_cents());
        self.persist_balance_cents(self.balance_cents);
        let added = format_balance_yuan(hud_avatar_panel::purchase_add_cents());
        let remaining = format_balance_yuan(self.balance_cents);
        self.balance_feedback = Some(format!("已购买 {added} · 剩余 {remaining}"));
        ctx.notify();
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(2600)).await;
            },
            |view, _, ctx| {
                view.balance_feedback = None;
                ctx.notify();
            },
        );
    }

    fn open_redeem_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.avatar_panel_open = false;
        self.redeem_modal_open = true;
        self.redeem_feedback = None;
        if self.redeem_code_draft.is_empty() {
            if let Some(text) = read_clipboard_text() {
                self.redeem_code_draft = text.trim().to_string();
            }
        }
        ctx.notify();
    }

    fn close_redeem_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.redeem_modal_open = false;
        self.redeem_code_draft.clear();
        self.redeem_feedback = None;
        ctx.notify();
    }

    fn paste_redeem_code(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(text) = read_clipboard_text() {
            self.redeem_code_draft = text.trim().to_string();
            self.redeem_feedback = None;
            ctx.notify();
        }
    }

    fn submit_redeem(&mut self, ctx: &mut ViewContext<Self>) {
        let code = self.redeem_code_draft.trim().to_string();
        if code.is_empty() {
            self.redeem_feedback = Some("请输入有效兑换码".into());
            ctx.notify();
            return;
        }
        let Some(add) = hud_avatar_panel::redeem_cents_for_code(&code) else {
            self.redeem_feedback = Some("兑换码无效".into());
            ctx.notify();
            return;
        };
        self.balance_cents = self.balance_cents.saturating_add(add);
        self.persist_balance_cents(self.balance_cents);
        self.redeem_modal_open = false;
        self.redeem_code_draft.clear();
        self.redeem_feedback = None;
        let added = format_balance_yuan(add);
        let remaining = format_balance_yuan(self.balance_cents);
        self.balance_feedback = Some(format!("兑换成功 +{added} · 剩余 {remaining}"));
        self.avatar_panel_open = true;
        ctx.notify();
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(2600)).await;
            },
            |view, _, ctx| {
                view.balance_feedback = None;
                ctx.notify();
            },
        );
    }

    fn tab_button(&self, tab: AppTab) -> Box<dyn Element> {
        let selected = self.tab == tab;
        let keyboard_focused = self.tab_bar_keyboard_focus && self.tab_focus == tab;
        let text_color = if selected {
            theme::accent_cool()
        } else {
            theme::muted()
        };
        let bg = if selected {
            theme::accent_cool_bg_default()
        } else {
            ColorU::new(0, 0, 0, 0)
        };
        let expand = self.tab == tab || self.hovered_tab == Some(tab);
        let label = Self::tab_label(tab);

        let bottom_accent = if selected {
            theme::accent_cool()
        } else {
            ColorU::transparent_black()
        };

        let content_height = icons::TAB_ICON_SIZE;
        let bottom_border = 2.0;
        let vertical_pad = ((CHROME_ROW_HEIGHT - content_height - bottom_border) / 2.0).max(0.0);

        let mut container = Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(icons::tab_button_content(
                    tab, expand, text_color, label, self.mono,
                ))
                .finish(),
        )
        .with_vertical_padding(vertical_pad)
        .with_horizontal_padding(if expand { 16.0 } else { 12.0 })
        .with_background(bg)
        .with_border(Border::bottom(2.0).with_border_fill(bottom_accent))
        .with_border(Border::right(1.0).with_border_fill(theme::border()));
        if keyboard_focused {
            container =
                container.with_border(Border::all(2.0).with_border_color(theme::accent_cool()));
        }
        EventHandler::new(container.finish())
            .on_mouse_in(
                move |ctx, _, _| {
                    ctx.dispatch_typed_action(AppShellAction::SetTabHover(Some(tab)));
                    DispatchEventResult::PropagateToParent
                },
                None,
            )
            .on_mouse_out(move |ctx, _, _| {
                ctx.dispatch_typed_action(AppShellAction::SetTabHover(None));
                DispatchEventResult::PropagateToParent
            })
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(AppShellAction::SelectTab(tab, TabSelectSource::Mouse));
                DispatchEventResult::StopPropagation
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
            self.auth_device_id.as_deref(),
            self.avatar_panel_open,
            self.font,
        ));

        if let Some(data) = traffic_light_data.as_ref() {
            if let Some(spacer) = window_chrome::traffic_light_spacer(data, zoom_factor) {
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
            "约 2 分钟：浏览终端共享、P2P 聊天、智能体任务。可随时跳过。",
            self.font,
        ));
        let mut steps = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        steps.add_child(self.onboarding_chip("1 · 终端", AppTab::Devices));
        steps.add_child(self.onboarding_chip("2 · 聊天", AppTab::Chat));
        steps.add_child(self.onboarding_chip("3 · 智能体", AppTab::Warp));
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

        if self.avatar_panel_open && self.auth_authenticated && !self.redeem_modal_open {
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
                    self.balance_cents,
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
                &self.redeem_code_draft,
                self.redeem_feedback.as_deref(),
                self.font,
                self.mono,
            ));
        }
        stack.add_child(ChildView::new(&self.login_modal).finish());

        if traffic_light_data
            .as_ref()
            .is_some_and(|data| data.side == window_chrome::TrafficLightSide::Right)
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
            AppShellAction::SetTabHover(tab) => {
                self.hovered_tab = *tab;
                ctx.notify();
            }
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
                #[cfg(windows)]
                crate::ui::windows_shell::hide_main_window_from_view(ctx);
                #[cfg(not(windows))]
                ctx.close_window();
            }
            AppShellAction::OpenLogin => {
                self.open_login_modal(ctx);
                ctx.notify();
            }
            AppShellAction::ToggleAvatarPanel => self.toggle_avatar_panel(ctx),
            AppShellAction::CloseAvatarPanel => self.close_avatar_panel(ctx),
            AppShellAction::PurchaseBalance => self.purchase_balance(ctx),
            AppShellAction::OpenRedeemModal => self.open_redeem_modal(ctx),
            AppShellAction::CloseRedeemModal => self.close_redeem_modal(ctx),
            AppShellAction::PasteRedeemCode => self.paste_redeem_code(ctx),
            AppShellAction::SubmitRedeem => self.submit_redeem(ctx),
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &AppShellAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let content = match action {
            AppShellAction::SetTabHover(_) => return ActionAccessibilityContent::Empty,
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
            AppShellAction::OpenRedeemModal => {
                AccessibilityContent::new_without_help("打开兑换", WarpA11yRole::ButtonRole)
            }
            AppShellAction::CloseRedeemModal => {
                AccessibilityContent::new_without_help("关闭兑换", WarpA11yRole::ButtonRole)
            }
            AppShellAction::PasteRedeemCode => AccessibilityContent::new_without_help(
                "从剪贴板粘贴兑换码",
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::SubmitRedeem => {
                AccessibilityContent::new_without_help("提交兑换", WarpA11yRole::ButtonRole)
            }
        };
        ActionAccessibilityContent::Custom(content)
    }
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
