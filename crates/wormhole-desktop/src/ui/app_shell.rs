use pathfinder_color::ColorU;
use std::sync::Arc;
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Fill, Flex, MainAxisSize,
    ParentElement, Radius, Rect, ScrollbarWidth, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{
    AccessibilityData, AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext,
    ViewHandle,
};
use warpui_core::keymap::Keystroke;

use crate::coordinator::{CoordinatorState, CoordinatorView};
use crate::ui::agent_panel::AgentPanelView;
use crate::ui::window_chrome::{self, CHROME_ROW_HEIGHT};
use crate::ui::chat::ChatShellView;
use crate::ui::codex_provider_import_model::SharedCodexProviderImportModel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::desktop_prefs::{self};
use crate::ui::devices_view::DevicesView;
use crate::ui::display_view::DisplayView;
use crate::ui::panel_primitives::section_hint;
use crate::ui::settings_view::SettingsView;
use crate::ui::sync_views::SyncView;
use crate::ui::theme;
use crate::ui::toolbox_view::ToolboxView;
use crate::ui::w_drive_view::WDriveView;
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

impl AppTab {
    pub fn persist_id(self) -> &'static str {
        match self {
            AppTab::WDrive => "w_drive",
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
            "w_drive" => Some(AppTab::WDrive),
            "devices" => Some(AppTab::Devices),
            "display" => Some(AppTab::Display),
            "chat" => Some(AppTab::Chat),
            "warp" => Some(AppTab::Warp),
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
    DismissOnboarding,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
}

pub struct AppShellView {
    tab: AppTab,
    tab_focus: AppTab,
    tab_bar_keyboard_focus: bool,
    core: CoreHandle,
    coordinator: std::sync::Arc<std::sync::Mutex<CoordinatorState>>,
    #[allow(dead_code)]
    coordinator_view: ViewHandle<CoordinatorView>,
    w_drive: ViewHandle<WDriveView>,
    devices: ViewHandle<DevicesView>,
    display: ViewHandle<DisplayView>,
    chat: ViewHandle<ChatShellView>,
    warp: ViewHandle<AgentPanelView>,
    toolbox: ViewHandle<ToolboxView>,
    settings: ViewHandle<SettingsView>,
    font: FamilyId,
    tab_scroll: ClippedScrollStateHandle,
    show_onboarding: bool,
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
        let warp = ctx.add_view(|ctx| AgentPanelView::new(ctx, core.clone()));
        let toolbox = ctx.add_view(|ctx| ToolboxView::new(ctx, core.clone(), coordinator.clone()));
        let settings = ctx.add_view(|ctx| SettingsView::new(ctx, core.clone(), import_model));
        let prefs = desktop_prefs::load(&core.data_dir());
        let mut tab = prefs
            .last_tab
            .as_deref()
            .and_then(AppTab::from_persist_id)
            .unwrap_or(AppTab::Chat);
        if let Some(ref url) = pending_deeplink {
            tab = AppTab::Settings;
            let settings_handle = settings.clone();
            ctx.update_view(&settings_handle, |view, ctx| {
                view.open_deeplink_url(url.clone(), ctx)
            });
        }
        let show_onboarding = pending_deeplink.is_none() && !prefs.onboarding_dismissed;
        let view = Self {
            tab,
            tab_focus: tab,
            tab_bar_keyboard_focus: false,
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
            tab_scroll: ClippedScrollStateHandle::new(),
            show_onboarding,
        };
        view.start_warp_focus_poll(ctx);
        view.start_deeplink_listener(ctx);
        Self::sync_titlebar_height(ctx);
        view
    }

    fn sync_titlebar_height(ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        if let Some(window) = ctx.windows().platform_window(window_id) {
            window.set_titlebar_height(CHROME_ROW_HEIGHT as f64);
        }
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

    fn adjacent_tab(current: AppTab, delta: i32) -> AppTab {
        let tabs = Self::tabs();
        let idx = tabs.iter().position(|&t| t == current).unwrap_or(0);
        let len = tabs.len() as i32;
        let next = (idx as i32 + delta).rem_euclid(len) as usize;
        tabs[next]
    }

    fn tab_from_arrow(keystroke: &Keystroke, current: AppTab) -> Option<AppTab> {
        if keystroke.ctrl || keystroke.meta || keystroke.alt {
            return None;
        }
        match keystroke.key.as_str() {
            "left" => Some(Self::adjacent_tab(current, -1)),
            "right" => Some(Self::adjacent_tab(current, 1)),
            "home" => Some(Self::tabs()[0]),
            "end" => Some(Self::tabs()[6]),
            _ => None,
        }
    }

    fn tab_from_keystroke(keystroke: &Keystroke) -> Option<AppTab> {
        if !keystroke.ctrl {
            return None;
        }
        match keystroke.key.as_str() {
            "1" => Some(AppTab::WDrive),
            "2" => Some(AppTab::Devices),
            "3" => Some(AppTab::Display),
            "4" => Some(AppTab::Chat),
            "5" => Some(AppTab::Warp),
            "6" => Some(AppTab::Toolbox),
            "7" => Some(AppTab::Settings),
            _ => None,
        }
    }

    fn tab_groups() -> [&'static [AppTab]; 4] {
        [
            &[AppTab::WDrive, AppTab::Devices],
            &[AppTab::Display, AppTab::Chat],
            &[AppTab::Warp, AppTab::Toolbox],
            &[AppTab::Settings],
        ]
    }

    fn persist_last_tab(&self) {
        let data_dir = self.core.data_dir();
        let tab_id = self.tab.persist_id().to_string();
        let _ = desktop_prefs::update(&data_dir, |prefs| {
            prefs.last_tab = Some(tab_id);
        });
    }

    fn tab_divider(&self) -> Box<dyn Element> {
        Container::new(
            ConstrainedBox::new(
                Rect::new()
                    .with_background_color(theme::border())
                    .finish(),
            )
            .with_width(1.0)
            .with_height(22.0)
            .finish(),
        )
        .with_horizontal_margin(4.0)
        .finish()
    }

    fn tab_button(&self, tab: AppTab) -> Box<dyn Element> {
        let selected = self.tab == tab;
        let keyboard_focused = self.tab_bar_keyboard_focus && self.tab_focus == tab;
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
        let mut btn = Container::new(
            EventHandler::new(label)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(AppShellAction::SelectTab(
                        tab,
                        TabSelectSource::Mouse,
                    ));
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)));
        if keyboard_focused {
            btn = btn.with_border(Border::all(2.0).with_border_color(theme::accent_cool()));
        }
        btn.finish()
    }

    fn tab_bar(&self) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        let groups = Self::tab_groups();
        for (group_idx, group) in groups.iter().enumerate() {
            if group_idx > 0 {
                row.add_child(self.tab_divider());
            }
            for &tab in *group {
                row.add_child(self.tab_button(tab));
            }
        }
        let scrollable_tabs = ClippedScrollable::horizontal(
            self.tab_scroll.clone(),
            row.finish(),
            ScrollbarWidth::None,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(Shrinkable::new(1.0, scrollable_tabs).finish())
                .with_child(window_chrome::caption_buttons(
                    self.font,
                    [
                        ("最小化", AppShellAction::MinimizeWindow),
                        ("最大化", AppShellAction::ToggleMaximizeWindow),
                        ("关闭", AppShellAction::CloseWindow),
                    ],
                ))
                .finish(),
        )
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_uniform_padding(8.0)
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
                ctx.dispatch_typed_action(AppShellAction::SelectTab(
                    tab,
                    TabSelectSource::Mouse,
                ));
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(theme::accent_bg(24))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish()
    }

    fn onboarding_banner(&self) -> Option<Box<dyn Element>> {
        if !self.show_onboarding {
            return None;
        }
        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        col.add_child(
            ui_text::title("快速开始", self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(section_hint(
            "约 2 分钟了解核心能力：虚拟盘、本机节点、P2P 聊天。可随时跳过。",
            self.font,
        ));
        let mut steps = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        steps.add_child(self.onboarding_chip("1 · W 盘", AppTab::WDrive));
        steps.add_child(self.onboarding_chip("2 · 设备", AppTab::Devices));
        steps.add_child(self.onboarding_chip("3 · 聊天", AppTab::Chat));
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
        let mut column = Flex::column().with_child(self.tab_bar());
        if let Some(banner) = self.onboarding_banner() {
            column.add_child(
                Container::new(banner)
                    .with_uniform_padding(8.0)
                    .finish(),
            );
        }
        column.add_child(
            Shrinkable::new(
                1.0,
                Container::new(content).with_uniform_padding(12.0).finish(),
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

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let current_tab = self.tab;
        let shell = Container::new(self.body())
            .with_background(theme::canvas())
            .with_uniform_padding(0.0)
            .finish();
        EventHandler::new(shell)
            .with_always_handle()
            .on_keydown(move |ctx, _, keystroke| {
                if let Some(tab) = Self::tab_from_keystroke(keystroke) {
                    ctx.dispatch_typed_action(AppShellAction::SelectTab(
                        tab,
                        TabSelectSource::Keyboard,
                    ));
                    return DispatchEventResult::StopPropagation;
                }
                if let Some(tab) = Self::tab_from_arrow(keystroke, current_tab) {
                    ctx.dispatch_typed_action(AppShellAction::SelectTab(
                        tab,
                        TabSelectSource::Keyboard,
                    ));
                    return DispatchEventResult::StopPropagation;
                }
                DispatchEventResult::PropagateToParent
            })
            .finish()
    }

    fn accessibility_contents(&self, _app: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
            format!("Wormhole，当前标签：{}", Self::tab_label(self.tab)),
            "Ctrl 加数字 1 到 7 切换标签。左右方向键切换相邻标签并显示键盘焦点环。Home 与 End 跳到首尾标签。",
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
                ctx.close_window();
            }
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
            AppShellAction::DismissOnboarding => AccessibilityContent::new_without_help(
                "关闭快速开始引导",
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::MinimizeWindow => AccessibilityContent::new_without_help(
                "最小化窗口",
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::ToggleMaximizeWindow => AccessibilityContent::new_without_help(
                "切换最大化",
                WarpA11yRole::ButtonRole,
            ),
            AppShellAction::CloseWindow => {
                AccessibilityContent::new_without_help("关闭窗口", WarpA11yRole::ButtonRole)
            }
        };
        ActionAccessibilityContent::Custom(content)
    }
}

#[cfg(test)]
mod tests {
    use super::AppTab;

    #[test]
    fn tab_persist_id_roundtrip() {
        let tabs = [
            AppTab::WDrive,
            AppTab::Devices,
            AppTab::Display,
            AppTab::Chat,
            AppTab::Warp,
            AppTab::Toolbox,
            AppTab::Settings,
        ];
        for tab in tabs {
            let id = tab.persist_id();
            assert_eq!(AppTab::from_persist_id(id), Some(tab));
        }
    }
}
