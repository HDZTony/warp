use pathfinder_color::ColorU;
use std::sync::Arc;
use warpui::elements::{
    Border, ChildView, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, MainAxisSize, ParentElement, Radius, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{
    AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext, ViewHandle,
};

use crate::coordinator::{CoordinatorState, CoordinatorView};
use crate::ui::chat::ChatShellView;
use crate::ui::codex_provider_import_model::SharedCodexProviderImportModel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::devices_view::DevicesView;
use crate::ui::display_view::DisplayView;
use crate::ui::settings_view::SettingsView;
use crate::ui::sync_views::SyncView;
use crate::ui::theme;
use crate::ui::toolbox_view::ToolboxView;
use crate::ui::w_drive_view::WDriveView;
use crate::ui::warp_embed_view::{WarpEmbedAction, WarpEmbedView};
use crate::ui_text;
use wormhole_desktop_core::warp_embed_prefs::PreferredAgent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    WDrive,
    Devices,
    Display,
    Chat,
    Warp,
    Toolbox,
    Settings,
}

#[derive(Debug, Clone)]
pub enum AppShellAction {
    SelectTab(AppTab),
}

pub struct AppShellView {
    tab: AppTab,
    core: CoreHandle,
    coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
    #[allow(dead_code)]
    coordinator_view: ViewHandle<CoordinatorView>,
    w_drive: ViewHandle<WDriveView>,
    devices: ViewHandle<DevicesView>,
    display: ViewHandle<DisplayView>,
    chat: ViewHandle<ChatShellView>,
    warp: ViewHandle<WarpEmbedView>,
    toolbox: ViewHandle<ToolboxView>,
    settings: ViewHandle<SettingsView>,
    font: FamilyId,
}

impl AppShellView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
        import_model: SharedCodexProviderImportModel,
        pending_deeplink: Option<String>,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let coordinator_view = ctx.add_view(|ctx| CoordinatorView::new(ctx, coordinator.clone()));
        let w_drive = ctx.add_view(|ctx| WDriveView::new(ctx, core.clone()));
        let devices = ctx.add_view(|ctx| DevicesView::new(ctx, core.clone()));
        let display = ctx.add_view(|ctx| DisplayView::new(ctx, core.clone()));
        let chat = ctx.add_view(|ctx| ChatShellView::new(ctx, core.clone()));
        let warp = ctx.add_view(|ctx| WarpEmbedView::new(ctx, core.clone()));
        let toolbox = ctx.add_view(|ctx| ToolboxView::new(ctx, core.clone(), coordinator.clone()));
        let settings = ctx.add_view(|ctx| SettingsView::new(ctx, core.clone(), import_model));
        let mut tab = AppTab::Chat;
        if let Some(url) = pending_deeplink {
            tab = AppTab::Settings;
            let settings_handle = settings.clone();
            ctx.update_view(&settings_handle, |view, ctx| {
                view.open_deeplink_url(url, ctx)
            });
        }
        let mut view = Self {
            tab,
            core,
            coordinator,
            coordinator_view,
            w_drive,
            devices,
            display,
            chat,
            warp,
            toolbox,
            settings,
            font,
        };
        view.start_warp_focus_poll(ctx);
        view.start_deeplink_listener(ctx);
        view
    }

    fn start_deeplink_listener(&self, ctx: &mut ViewContext<Self>) {
        let mut rx = self.core.runtime().ctx.events.subscribe();
        let settings = self.settings.clone();
        Self::poll_deeplink_once(ctx, rx, settings);
    }

    fn poll_deeplink_once(
        ctx: &mut ViewContext<Self>,
        rx: tokio::sync::broadcast::Receiver<wormhole_desktop_core::DesktopEvent>,
        settings: ViewHandle<SettingsView>,
    ) {
        let mut waiter = rx.resubscribe();
        ctx.spawn(
            async move { waiter.recv().await },
            move |_view, output, ctx| {
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
                Self::poll_deeplink_once(ctx, rx, settings);
            },
        );
    }

    fn start_warp_focus_poll(&self, ctx: &mut ViewContext<Self>) {
        let coordinator = Arc::clone(&self.coordinator);
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
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
                        } else if let Some(signal) =
                            wormhole_desktop_core::warp_remote::take_focus_signal(guard.data_dir())
                        {
                            let agent = match signal.agent {
                                wormhole_desktop_core::warp_remote::WarpAgentKind::Codex => {
                                    PreferredAgent::Codex
                                }
                                wormhole_desktop_core::warp_remote::WarpAgentKind::Cursor => {
                                    PreferredAgent::Cursor
                                }
                            };
                            view.focus_warp_tab(agent, ctx);
                        }
                    }
                    Self::poll_warp_focus_once(ctx, tick_rx, coordinator);
                }
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
            ctx.notify();
        }
    }

    fn tab_label(tab: AppTab) -> &'static str {
        match tab {
            AppTab::WDrive => "W 盘",
            AppTab::Devices => "设备",
            AppTab::Display => "显示器",
            AppTab::Chat => "聊天",
            AppTab::Warp => "Warp",
            AppTab::Toolbox => "工具箱",
            AppTab::Settings => "设置",
        }
    }

    fn tabs() -> [AppTab; 7] {
        [
            AppTab::WDrive,
            AppTab::Devices,
            AppTab::Display,
            AppTab::Chat,
            AppTab::Warp,
            AppTab::Toolbox,
            AppTab::Settings,
        ]
    }

    fn tab_bar(&self) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        for tab in Self::tabs() {
            let selected = self.tab == tab;
            let bg = if selected {
                theme::accent_bg(40)
            } else {
                ColorU::new(0, 0, 0, 0)
            };
            let label = ui_text::body(Self::tab_label(tab), self.font)
                .with_color(if selected {
                    theme::accent()
                } else {
                    theme::text()
                })
                .finish();
            let tab_btn = Container::new(
                EventHandler::new(label)
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(AppShellAction::SelectTab(tab));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish();
            row.add_child(tab_btn);
        }
        Container::new(row.finish())
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_uniform_padding(8.0)
            .finish()
    }

    fn body(&self) -> Box<dyn Element> {
        let content: Box<dyn Element> = match self.tab {
            AppTab::WDrive => ChildView::new(&self.w_drive).finish(),
            AppTab::Devices => ChildView::new(&self.devices).finish(),
            AppTab::Display => ChildView::new(&self.display).finish(),
            AppTab::Chat => ChildView::new(&self.chat).finish(),
            AppTab::Warp => ChildView::new(&self.warp).finish(),
            AppTab::Toolbox => ChildView::new(&self.toolbox).finish(),
            AppTab::Settings => ChildView::new(&self.settings).finish(),
        };
        Flex::column()
            .with_child(self.tab_bar())
            .with_child(
                Shrinkable::new(
                    1.0,
                    Container::new(content).with_uniform_padding(12.0).finish(),
                )
                .finish(),
            )
            .finish()
    }
}

impl Entity for AppShellView {
    type Event = ();
}

impl View for AppShellView {
    fn ui_name() -> &'static str {
        "AppShellView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Container::new(self.body())
            .with_background(theme::canvas())
            .with_uniform_padding(0.0)
            .finish()
    }
}

impl TypedActionView for AppShellView {
    type Action = AppShellAction;

    fn handle_action(&mut self, action: &AppShellAction, ctx: &mut ViewContext<Self>) {
        match action {
            AppShellAction::SelectTab(tab) => {
                let warp_visible = *tab == AppTab::Warp;
                let warp_handle = self.warp.clone();
                ctx.update_view(&warp_handle, |view, ctx| {
                    view.set_tab_visible(warp_visible, ctx);
                });
                self.tab = *tab;
                ctx.notify();
            }
        }
    }
}
