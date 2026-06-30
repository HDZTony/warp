use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use std::sync::Arc;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, ChildAnchor, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded,
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
use crate::ui::desktop_prefs::{self};
use crate::ui::devices_view::DevicesView;
use crate::ui::display_view::DisplayView;
use crate::ui::hud_effects::HudBackdrop;
use crate::ui::icons;
use crate::ui::login_modal::{LoginModalAction, LoginModalEvent, LoginModalView};
use crate::ui::settings_view::{SettingsEvent, SettingsView};
use crate::ui::sync_views::SyncView;
use crate::ui::theme;
use crate::ui::toolbox_view::ToolboxView;
use crate::ui::w_drive_view::WDriveView;
use crate::ui::window_chrome::{self, CHROME_ROW_HEIGHT, TrafficLightActions, TrafficLightMouseStates};
use crate::ui_text;
use crate::ui::panel_primitives::{section_hint, tab_content_fill, HUD_RADIUS};
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
    auth_user_id: Option<String>,
    login_modal_open: bool,
    font: FamilyId,
    mono: FamilyId,
    hud_nodes: usize,
    device_ready: bool,
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
        let toolbox =
            ctx.add_typed_action_view(|ctx| ToolboxView::new(ctx, core.clone(), coordinator.clone()));
        let settings =
            ctx.add_typed_action_view(|ctx| SettingsView::new(ctx, core.clone(), import_model));
        let login_modal = ctx.add_typed_action_view(|ctx| LoginModalView::new(ctx, core.clone()));
        ctx.subscribe_to_view(&login_modal, |view, _, event, ctx| {
            match event {
                LoginModalEvent::AuthChanged {
                    authenticated,
                    user_id,
                } => {
                    view.auth_authenticated = *authenticated;
                    view.auth_user_id = user_id.clone();
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
            let SettingsEvent::AccountChanged { authenticated } = event;
            view.auth_authenticated = *authenticated;
            if !authenticated {
                view.auth_user_id = None;
                view.device_ready = false;
            }
            view.refresh_auth_gated_views(ctx);
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
            auth_user_id: None,
            login_modal_open: false,
            font,
            mono,
            hud_nodes: 1,
            device_ready: false,
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
                    view.auth_authenticated = !status.auth_required;
                    let device_ready =
                        !status.auth_required && !status.device_bootstrap_required;
                    if device_ready && !view.device_ready {
                        view.refresh_auth_gated_views(ctx);
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
        ctx.spawn(async move { rx.recv().await }, move |_view, output, ctx| {
            if let Ok(event) = output {
                if event.name == "deeplink-import" {
                    if let Some(url) = event.payload.get("url").and_then(|v| v.as_str()) {
                        let url = url.to_string();
                        ctx.update_view(&settings, |view, ctx| {
                            view.open_deeplink_url(url, ctx);
                        });
                    }
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
                    view.auth_authenticated = status.authenticated;
                    view.auth_user_id = status.user_id;
                    ctx.notify();
                }
            },
        );
    }

    fn hud_login_entry(&self) -> Box<dyn Element> {
        if self.auth_authenticated {
            let label = self
                .auth_user_id
                .as_deref()
                .map(|id| {
                    if id.len() > 10 {
                        format!("{}…", &id[..8])
                    } else {
                        id.to_string()
                    }
                })
                .unwrap_or_else(|| "ACCOUNT".to_string());
            return Container::new(
                ui_text::hud_title(label, self.mono)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_horizontal_margin(8.0)
            .finish();
        }

        Container::new(
            EventHandler::new(
                ui_text::hud_title("登录", self.mono)
                    .with_color(theme::accent())
                    .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(AppShellAction::OpenLogin);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(6.0)
        .with_horizontal_margin(4.0)
        .with_background(theme::accent_bg(20))
        .with_border(Border::all(1.0).with_border_fill(theme::accent()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .finish()
    }

    fn hud_status_bar(&self) -> Box<dyn Element> {
        let nodes = format!("{}", self.hud_nodes);
        let metrics = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                ui_text::hud_title("IROH · SYNC", self.mono)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_child(
                Container::new(Flex::row().finish())
                    .with_horizontal_margin(16.0)
                    .finish(),
            )
            .with_child(self.hud_login_entry())
            .with_child(
                Flex::row()
                    .with_child(
                        ui_text::hud_title("NODES ", self.mono)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_child(
                        ui_text::hud_title(nodes, self.mono)
                            .with_color(theme::accent())
                            .finish(),
                    )
                    .finish(),
            )
            .finish();
        Container::new(metrics)
            .with_uniform_padding(12.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish()
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
        let vertical_pad =
            ((CHROME_ROW_HEIGHT - content_height - bottom_border) / 2.0).max(0.0);

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
        tab_row.add_child(self.hud_status_bar());

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
        column.add_child(
            Expanded::new(
                1.0,
                HudBackdrop::live(tab_content_fill(content)),
            )
            .finish(),
        );
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
        stack.add_child(ChildView::new(&self.login_modal).finish());

        if window_chrome::traffic_light_data(app, self.window_id)
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
                self.login_modal_open = true;
                let login = self.login_modal.clone();
                ctx.update_view(&login, |modal, ctx| {
                    modal.open(ctx);
                });
                ctx.notify();
            }
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
