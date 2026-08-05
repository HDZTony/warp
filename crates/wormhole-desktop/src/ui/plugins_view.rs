//! Settings → 插件：Codex 市场 + bb-browser 托管 Chromium / MCP。

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::agent_bb_browser_commands::{
    self, AgentBbBrowserStatusDto, ConfigureAgentBbBrowserParams,
};
use wormhole_desktop_core::agent_plugins_catalog::{
    self, AgentPluginInstallParams, AgentPluginListItem, AgentPluginsListDto,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_hint, section_title, status_line, truncate_middle, StatusTone,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum PluginsAction {
    Refresh,
    InstallMarketplacePlugin(String),
    ToggleBbBrowser,
    ToggleBbBrowserAutoStart,
    EnsureBbBrowser,
    CreateBbBrowserShortcut,
    SetBbBrowserDefault,
}

pub struct PluginsView {
    core: CoreHandle,
    font: FamilyId,
    marketplace: Option<AgentPluginsListDto>,
    marketplace_error: Option<String>,
    installing_id: Option<String>,
    bb_browser: Option<AgentBbBrowserStatusDto>,
    status: String,
    busy: bool,
}

impl PluginsView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            marketplace: None,
            marketplace_error: None,
            installing_id: None,
            bb_browser: None,
            status: String::new(),
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在刷新插件状态…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let bb = agent_bb_browser_commands::agent_bb_browser_status(&state).await;
                let market = agent_plugins_catalog::agent_plugins_list(&state).await;
                (bb, market)
            },
            |view, output, ctx| {
                match output.0 {
                    Ok(status) => {
                        view.bb_browser = Some(status);
                    }
                    Err(error) => {
                        view.bb_browser = None;
                        view.status = format!("刷新 bb-browser 失败: {error}");
                    }
                }
                match output.1 {
                    Ok(dto) => {
                        view.marketplace = Some(dto);
                        view.marketplace_error = None;
                        if view.status.starts_with("正在刷新") || view.status.is_empty() {
                            view.status = "插件状态已刷新".into();
                        }
                    }
                    Err(error) => {
                        view.marketplace = None;
                        view.marketplace_error = Some(error.clone());
                        view.status = format!("刷新 Codex 市场失败: {error}");
                    }
                }
                view.busy = false;
                view.installing_id = None;
                ctx.notify();
            },
        );
    }

    fn install_marketplace_plugin(&mut self, plugin_id: String, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.installing_id = Some(plugin_id.clone());
        self.status = format!("正在安装 {plugin_id}…");
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let (name, marketplace) = if let Some((n, m)) = plugin_id.split_once('@') {
                    (n.to_string(), Some(m.to_string()))
                } else {
                    (plugin_id.clone(), None)
                };
                agent_plugins_catalog::agent_plugins_install(
                    &state,
                    AgentPluginInstallParams { name, marketplace },
                )
                .await?;
                agent_plugins_catalog::agent_plugins_list(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                view.installing_id = None;
                match output {
                    Ok(dto) => {
                        view.marketplace = Some(dto);
                        view.marketplace_error = None;
                        view.status = "插件已安装".into();
                    }
                    Err(err) => {
                        view.status = format!("安装失败: {err}");
                    }
                }
                ctx.notify();
            },
        );
    }

    fn configure_bb_browser(
        &mut self,
        enabled: Option<bool>,
        auto_start_browser: Option<bool>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.busy = true;
        self.status = if enabled == Some(true) {
            "正在检查并安装前置依赖（Node / bb-browser）…".into()
        } else {
            "正在更新 bb-browser…".into()
        };
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_bb_browser_commands::agent_bb_browser_configure(
                    ConfigureAgentBbBrowserParams {
                        enabled,
                        auto_start_browser,
                    },
                    &state,
                )
                .await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(status) => {
                        view.status = if status.config.enabled {
                            let base = if status.ready {
                                "bb-browser 已启用且就绪"
                            } else {
                                "bb-browser 已启用（见下方提示）"
                            };
                            match status.setup_message.as_deref() {
                                Some(setup) if !setup.is_empty() => {
                                    truncate_middle(&format!("{base}。{setup}"), 160)
                                }
                                _ => base.into(),
                            }
                        } else {
                            "bb-browser 已关闭".into()
                        };
                        view.bb_browser = Some(status);
                    }
                    Err(err) => {
                        view.status = err;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn ensure_bb_browser(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在启动托管浏览器…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_bb_browser_commands::agent_bb_browser_ensure_browser(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(action) => {
                        view.status = action.message;
                        view.bb_browser = Some(action.status);
                    }
                    Err(err) => {
                        view.status = err;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn create_bb_browser_shortcut(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在创建桌面快捷方式…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_bb_browser_commands::agent_bb_browser_create_shortcut(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(action) => {
                        view.status = action.message;
                        view.bb_browser = Some(action.status);
                    }
                    Err(err) => {
                        view.status = err;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn set_bb_browser_default(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在注册系统默认浏览器…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_bb_browser_commands::agent_bb_browser_set_default_browser(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(action) => {
                        view.status = action.message;
                        view.bb_browser = Some(action.status);
                    }
                    Err(err) => {
                        view.status = err;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn action_button(&self, label: &str, action: PluginsAction) -> Box<dyn Element> {
        let label = label.to_string();
        let automation_id = format!("plugins:btn:{label}");
        let disabled = self.busy;
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(theme::accent())
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                if disabled {
                    return DispatchEventResult::StopPropagation;
                }
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_bg(28))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn marketplace_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("CODEX · 插件市场", self.font));
        col.add_child(section_hint(
            "浏览 Wormhole 托管的 Codex 插件（官方镜像按需下载单个 zip）。需登录；不 clone openai/plugins，不启用 remote_plugin。",
            self.font,
        ));

        if let Some(error) = &self.marketplace_error {
            col.add_child(status_line(
                truncate_middle(error, 140),
                self.font,
                StatusTone::Danger,
            ));
            return col.finish();
        }

        let Some(dto) = &self.marketplace else {
            col.add_child(status_line(
                "正在加载 Codex 插件市场…",
                self.font,
                StatusTone::Muted,
            ));
            return col.finish();
        };

        let installed_count = dto.plugins.len();
        let available_count = dto.available.len();
        col.add_child(status_line(
            format!("已安装 {installed_count} · 可安装 {available_count}"),
            self.font,
            StatusTone::Neutral,
        ));

        if !dto.plugins.is_empty() {
            col.add_child(
                Container::new(section_hint("已安装", self.font))
                    .with_margin_top(8.0)
                    .finish(),
            );
            for plugin in &dto.plugins {
                col.add_child(self.plugin_row(plugin, false));
            }
        }

        if !dto.available.is_empty() {
            col.add_child(
                Container::new(section_hint("可安装", self.font))
                    .with_margin_top(10.0)
                    .finish(),
            );
            for plugin in &dto.available {
                col.add_child(self.plugin_row(plugin, true));
            }
        }

        if dto.plugins.is_empty() && dto.available.is_empty() {
            col.add_child(status_line(
                "市场为空。请确认已登录，且 R2 catalog 已同步。",
                self.font,
                StatusTone::Placeholder,
            ));
        }

        col.finish()
    }

    fn plugin_row(&self, plugin: &AgentPluginListItem, installable: bool) -> Box<dyn Element> {
        let category = plugin
            .category
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("—");
        let desc = plugin
            .description
            .as_deref()
            .unwrap_or("")
            .trim();
        let line = if desc.is_empty() {
            format!("{} · {}", plugin.name, category)
        } else {
            format!("{} · {} · {}", plugin.name, category, desc)
        };
        let mut row_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        row_col.add_child(
            ui_text::body(truncate_middle(&line, 110), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if installable {
            let id = plugin.id.clone();
            let installing = self.installing_id.as_deref() == Some(id.as_str());
            let label = if installing { "安装中…" } else { "安装" };
            row_col.add_child(
                Container::new(self.action_button(
                    label,
                    PluginsAction::InstallMarketplacePlugin(id),
                ))
                .with_margin_top(6.0)
                .finish(),
            );
        } else {
            row_col.add_child(
                Container::new(status_line("已安装", self.font, StatusTone::Success))
                    .with_margin_top(4.0)
                    .finish(),
            );
        }
        Container::new(row_col.finish())
            .with_uniform_padding(10.0)
            .with_margin_top(6.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
            .finish()
    }

    fn bb_browser_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("PLUGIN · bb-browser", self.font));
        col.add_child(section_hint(
            "托管 Chromium（免扩展）。启用时自动检查浏览器并安装本地 Node / bb-browser。用桌面「Wormhole浏览器」登录；也可设为系统默认浏览器。公开网页用 bb-browser，桌面 GUI 用 Computer Use。",
            self.font,
        ));

        let Some(status) = &self.bb_browser else {
            col.add_child(status_line(
                "正在加载 bb-browser 状态…",
                self.font,
                StatusTone::Muted,
            ));
            return col.finish();
        };

        let enabled_label = if status.config.enabled {
            "已启用"
        } else {
            "已关闭"
        };
        let launch = status
            .launch_command
            .as_deref()
            .unwrap_or("未检测到 bb-browser / npx");
        let cdp = if status.cdp_healthy {
            format!("CDP 就绪 {}:{}", status.cdp_host, status.cdp_port)
        } else {
            format!("CDP 未就绪 {}:{}", status.cdp_host, status.cdp_port)
        };
        let browser = if status.browser_available {
            "浏览器已找到"
        } else {
            "未找到 Chrome/Edge"
        };
        let summary = format!(
            "{enabled_label} · {} · MCP={} · {browser} · {cdp}",
            status.launch_kind.as_deref().unwrap_or("无启动器"),
            if status.mcp_section_written {
                "是"
            } else {
                "否"
            },
        );
        col.add_child(status_line(
            summary,
            self.font,
            if status.ready {
                StatusTone::Success
            } else if status.config.enabled {
                StatusTone::Neutral
            } else {
                StatusTone::Muted
            },
        ));
        col.add_child(status_line(
            truncate_middle(launch, 96),
            self.font,
            StatusTone::Muted,
        ));
        col.add_child(status_line(
            truncate_middle(&format!("profile: {}", status.profile_dir), 110),
            self.font,
            StatusTone::Muted,
        ));

        let mut toolbar = Flex::row();
        toolbar.add_child(self.action_button(
            if status.config.enabled {
                "关闭 bb-browser"
            } else {
                "启用 bb-browser"
            },
            PluginsAction::ToggleBbBrowser,
        ));
        if status.config.enabled {
            toolbar.add_child(
                Container::new(self.action_button(
                    if status.config.auto_start_browser {
                        "自动启浏览器：开"
                    } else {
                        "自动启浏览器：关"
                    },
                    PluginsAction::ToggleBbBrowserAutoStart,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            toolbar.add_child(
                Container::new(self.action_button(
                    "启动浏览器",
                    PluginsAction::EnsureBbBrowser,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            toolbar.add_child(
                Container::new(self.action_button(
                    "Wormhole 设为系统默认浏览器",
                    PluginsAction::SetBbBrowserDefault,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            toolbar.add_child(
                Container::new(self.action_button(
                    "创建桌面快捷方式",
                    PluginsAction::CreateBbBrowserShortcut,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
        }
        col.add_child(Container::new(toolbar.finish()).with_margin_top(8.0).finish());

        for note in &status.notes {
            col.add_child(
                Container::new(status_line(note.clone(), self.font, StatusTone::Muted))
                    .with_margin_top(4.0)
                    .finish(),
            );
        }

        col.finish()
    }
}

fn plugin_status_tone(status: &str) -> StatusTone {
    if status.contains("错误") || status.contains("失败") || status.contains("未找到") {
        StatusTone::Danger
    } else if status.contains("已") || status.contains("成功") || status.contains("就绪") {
        StatusTone::Success
    } else {
        StatusTone::Neutral
    }
}

impl Entity for PluginsView {
    type Event = ();
}

impl View for PluginsView {
    fn ui_name() -> &'static str {
        "PluginsView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let mut toolbar = Flex::row();
        toolbar.add_child(self.action_button("刷新", PluginsAction::Refresh));
        col.add_child(toolbar.finish());

        if !self.status.is_empty() {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                plugin_status_tone(&self.status),
            ));
        }

        col.add_child(
            Container::new(self.marketplace_block())
                .with_margin_top(12.0)
                .finish(),
        );
        col.add_child(
            Container::new(self.bb_browser_block())
                .with_margin_top(16.0)
                .finish(),
        );

        col.finish()
    }
}

impl TypedActionView for PluginsView {
    type Action = PluginsAction;

    fn handle_action(&mut self, action: &PluginsAction, ctx: &mut ViewContext<Self>) {
        match action {
            PluginsAction::Refresh => self.refresh(ctx),
            PluginsAction::InstallMarketplacePlugin(id) => {
                self.install_marketplace_plugin(id.clone(), ctx)
            }
            PluginsAction::ToggleBbBrowser => {
                let enabled = self
                    .bb_browser
                    .as_ref()
                    .map(|s| !s.config.enabled)
                    .unwrap_or(true);
                self.configure_bb_browser(Some(enabled), None, ctx);
            }
            PluginsAction::ToggleBbBrowserAutoStart => {
                let auto = self
                    .bb_browser
                    .as_ref()
                    .map(|s| !s.config.auto_start_browser)
                    .unwrap_or(true);
                self.configure_bb_browser(None, Some(auto), ctx);
            }
            PluginsAction::EnsureBbBrowser => self.ensure_bb_browser(ctx),
            PluginsAction::CreateBbBrowserShortcut => self.create_bb_browser_shortcut(ctx),
            PluginsAction::SetBbBrowserDefault => self.set_bb_browser_default(ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{plugin_status_tone, StatusTone};

    #[test]
    fn plugin_operation_status_has_explicit_tone() {
        assert_eq!(
            plugin_status_tone("刷新插件失败: timeout"),
            StatusTone::Danger
        );
        assert_eq!(plugin_status_tone("插件状态已刷新"), StatusTone::Success);
        assert_eq!(
            plugin_status_tone("正在启动托管浏览器…"),
            StatusTone::Neutral
        );
    }
}
