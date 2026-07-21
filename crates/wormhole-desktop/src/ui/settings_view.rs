use warpui::elements::{
    Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex, MainAxisSize,
    ParentElement, Radius, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::agent_panel::sidebar::{load_archived_snapshots, ArchivedSessionSnapshot};
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    section_hint, section_title, status_line, tab_content_fill, StatusTone,
};
use crate::ui::text_field_input::{
    render_field_with_caret, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status_fast;
use wormhole_desktop_core::email_connector_commands::{
    email_connector_disconnect, email_connector_list, email_connector_start_oauth,
    email_connector_test_read, EmailConnectorDto,
};
use wormhole_desktop_core::settings_cache_commands::{
    clear_settings_cache, settings_cache_status, ClearSettingsCacheParams, SettingsCacheStatusDto,
};
use wormhole_desktop_core::sync_commands::{
    migrate_shared_storage, shared_storage_info, SharedStorageInfoDto,
};
use wormhole_desktop_core::{
    clear_cloud_auth_token, cloud_auth_status, get_network_relay_status, save_network_relay_config,
    NetworkRelayStatusDto, SaveNetworkRelayParams,
};

#[derive(Debug, Clone)]
pub enum SettingsEvent {
    AccountChanged { authenticated: bool },
    OpenLogin,
    OpenClusterManagement,
    RestoreArchivedSession(String),
    DeleteArchivedSession(String),
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    BrowseMigrate,
    SaveMigration,
    FocusPath,
    TextFieldEdit(TextFieldEditAction),
    Refresh,
    Login,
    Logout,
    RefreshAccount,
    RefreshEmailConnectors,
    ConnectEmail(String),
    DisconnectEmail(String),
    TestEmail(String),
    ToggleClusterSection,
    OpenClusterManagement,
    ToggleRelaySection,
    ToggleCacheSection,
    ToggleArchiveSection,
    RestoreArchivedSession(String),
    DeleteArchivedSession(String),
    RefreshRelay,
    SelectRelay(String),
    ApplyRelay,
    RefreshCache,
    ClearCache(String),
}

pub struct SettingsView {
    core: CoreHandle,
    font: FamilyId,
    storage: Option<SharedStorageInfoDto>,
    storage_draft: String,
    storage_field: TextFieldState,
    storage_focused: bool,
    status: String,
    status_tone: StatusTone,
    busy: bool,
    auth_user_id: Option<String>,
    auth_status: String,
    auth_status_tone: StatusTone,
    auth_busy: bool,
    auth_device_id: Option<String>,
    email_connectors: Vec<EmailConnectorDto>,
    email_busy: bool,
    email_message: String,
    email_tone: StatusTone,
    cluster_expanded: bool,
    cluster_id: Option<String>,
    cluster_name: Option<String>,
    cluster_message: String,
    archive_expanded: bool,
    relay_expanded: bool,
    relay_status: Option<NetworkRelayStatusDto>,
    relay_mode: String,
    relay_message: String,
    relay_tone: StatusTone,
    relay_busy: bool,
    cache_expanded: bool,
    cache_status: Option<SettingsCacheStatusDto>,
    cache_message: String,
    cache_tone: StatusTone,
    cache_busy: bool,
    scroll: ClippedScrollStateHandle,
}

impl SettingsView {
    fn archive_expanded_path(core: &CoreHandle) -> std::path::PathBuf {
        core.data_dir().join("settings-archive-expanded")
    }

    fn load_archive_expanded(core: &CoreHandle) -> bool {
        std::fs::read_to_string(Self::archive_expanded_path(core))
            .map(|value| value.trim() == "1")
            .unwrap_or(false)
    }

    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let archive_expanded = Self::load_archive_expanded(&core);
        let mut view = Self {
            core,
            font,
            storage: None,
            storage_draft: String::new(),
            storage_field: TextFieldState::new(),
            storage_focused: false,
            status: String::new(),
            status_tone: StatusTone::Placeholder,
            busy: false,
            auth_user_id: None,
            auth_status: String::new(),
            auth_status_tone: StatusTone::Placeholder,
            auth_busy: false,
            auth_device_id: None,
            email_connectors: Vec::new(),
            email_busy: false,
            email_message: String::new(),
            email_tone: StatusTone::Placeholder,
            cluster_expanded: false,
            cluster_id: None,
            cluster_name: None,
            cluster_message: String::new(),
            archive_expanded,
            relay_expanded: false,
            relay_status: None,
            relay_mode: "auto".into(),
            relay_message: String::new(),
            relay_tone: StatusTone::Placeholder,
            relay_busy: false,
            cache_expanded: false,
            cache_status: None,
            cache_message: String::new(),
            cache_tone: StatusTone::Placeholder,
            cache_busy: false,
            scroll: ClippedScrollStateHandle::new(),
        };
        view.refresh(ctx);
        view.refresh_account(ctx);
        view.refresh_email_connectors(ctx);
        view.refresh_cluster(ctx);
        view.refresh_relay(ctx);
        view.refresh_cache(ctx);
        view
    }

    fn refresh_email_connectors(&mut self, ctx: &mut ViewContext<Self>) {
        if self.auth_user_id.is_none() {
            self.email_connectors.clear();
            self.email_message = "登录 Wormhole 后可连接邮箱。".into();
            self.email_tone = StatusTone::Placeholder;
            return;
        }
        self.email_busy = true;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                email_connector_list(&state).await
            },
            |view, output, ctx| {
                view.email_busy = false;
                match output {
                    Ok(result) => {
                        view.email_connectors = result.items;
                        view.email_message.clear();
                        view.email_tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.email_message = format!("读取邮箱连接器失败: {error}");
                        view.email_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    pub fn refresh_account(&mut self, ctx: &mut ViewContext<Self>) {
        self.auth_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cloud_auth_status(&state).await
            },
            |view, output, ctx| {
                view.auth_busy = false;
                match output {
                    Ok(status) => {
                        view.auth_user_id = status.user_id;
                        view.auth_device_id = status.device_id;
                        if status.authenticated && view.auth_device_id.is_some() {
                            view.auth_status = "已登录".into();
                            view.auth_status_tone = StatusTone::Success;
                        } else if status.authenticated {
                            view.auth_status = "已登录，正在恢复设备身份…".into();
                            view.auth_status_tone = StatusTone::Placeholder;
                        } else {
                            view.auth_status = "未登录 — P2P / 集群 / 聊天需先登录。".into();
                            view.auth_status_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(err) => {
                        view.auth_status = format!("读取登录状态失败: {err}");
                        view.auth_status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_cluster(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status_fast(&state).await
            },
            |view, output, ctx| {
                match output {
                    Ok(status) => {
                        let active = status.clusters.into_iter().find(|cluster| cluster.active);
                        view.cluster_id = active
                            .as_ref()
                            .map(|cluster| cluster.cluster_id.clone())
                            .or(status.cluster_id);
                        view.cluster_name =
                            active.and_then(|cluster| cluster.name.or(Some(cluster.folder_name)));
                        view.cluster_message.clear();
                    }
                    Err(error) => {
                        view.cluster_message = format!("读取集群状态失败: {error}");
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_cache(&mut self, ctx: &mut ViewContext<Self>) {
        self.cache_busy = true;
        if self.cache_message.is_empty() {
            self.cache_message = "正在统计可安全重建的本地缓存…".into();
            self.cache_tone = StatusTone::Placeholder;
        }
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                settings_cache_status(&state).await
            },
            |view, output, ctx| {
                view.cache_busy = false;
                match output {
                    Ok(status) => {
                        view.cache_status = Some(status);
                        view.cache_message =
                            "仅统计聊天派生缓存、预览缓存与工作区传输缓存。".into();
                        view.cache_tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.cache_message = format!("缓存统计失败: {error}");
                        view.cache_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_relay(&mut self, ctx: &mut ViewContext<Self>) {
        self.relay_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                get_network_relay_status(&state).await
            },
            |view, output, ctx| {
                view.relay_busy = false;
                match output {
                    Ok(status) => {
                        view.relay_mode = status.relay_mode.clone();
                        view.relay_status = Some(status);
                        if view.relay_message.is_empty() {
                            view.relay_message =
                                "切换 Relay 后会重启 P2P 节点；集群成员需使用相同 Relay。".into();
                            view.relay_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(err) => {
                        view.relay_message = format!("读取 Relay 配置失败: {err}");
                        view.relay_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                shared_storage_info(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(s) => {
                        let was_dirty = view.storage_dirty();
                        if !was_dirty {
                            view.storage_draft = s.sync_entry_path.clone();
                        }
                        view.storage = Some(s);
                        if view.status.is_empty() {
                            view.status = "点击「浏览…」可选择新位置并迁移共享文件。".into();
                            view.status_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(e) => {
                        view.status = format!("无法读取同步路径: {e}");
                        view.status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn path_row(&self, label: &str, value: &str) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::mono(value.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(theme::bg())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish(),
        );
        col.finish()
    }

    fn storage_dirty(&self) -> bool {
        self.storage
            .as_ref()
            .map(|storage| storage_paths_differ(&storage.sync_entry_path, &self.storage_draft))
            .unwrap_or(false)
    }

    fn flat_section(&self, body: Box<dyn Element>) -> Box<dyn Element> {
        Container::new(body)
            .with_padding_top(16.0)
            .with_padding_bottom(16.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish()
    }

    /// Inline action buttons (`.settings-auth-actions` / `.settings-action-row` in HTML).
    fn inline_action_row(&self, buttons: Vec<Box<dyn Element>>) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        for (index, button) in buttons.into_iter().enumerate() {
            if index > 0 {
                row.add_child(Container::new(button).with_margin_left(8.0).finish());
            } else {
                row.add_child(button);
            }
        }
        row.finish()
    }

    fn action_button(&self, label: &str, action: SettingsAction) -> Box<dyn Element> {
        self.stateful_action_button(label, action, false, false)
    }

    fn stateful_action_button(
        &self,
        label: &str,
        action: SettingsAction,
        disabled: bool,
        primary: bool,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                if disabled {
                    return DispatchEventResult::StopPropagation;
                }
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(7.0)
        .with_padding_bottom(7.0)
        .with_background(if primary {
            theme::accent_cool_bg(if disabled { 16 } else { 40 })
        } else {
            theme::accent_bg(if disabled { 8 } else { 24 })
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn relay_option_button(&self, mode: &str, label: &str) -> Box<dyn Element> {
        let selected = self.relay_mode == mode;
        let disabled = self.relay_busy;
        let mode = mode.to_string();
        let label = if selected {
            format!("● {label}")
        } else {
            format!("○ {label}")
        };
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(if selected {
                        theme::accent_cool()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .on_left_mouse_down({
                let mode = mode.clone();
                move |ctx, _, _| {
                    if disabled {
                        return DispatchEventResult::StopPropagation;
                    }
                    ctx.dispatch_typed_action(SettingsAction::SelectRelay(mode.clone()));
                    DispatchEventResult::StopPropagation
                }
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(if selected {
            theme::accent_cool_bg(24)
        } else {
            theme::accent_bg(12)
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn fold_summary_badge(&self, summary: &str) -> Box<dyn Element> {
        Container::new(
            ui_text::mono(summary.to_string(), self.font)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_padding_left(8.0)
        .with_padding_right(8.0)
        .with_padding_top(2.0)
        .with_padding_bottom(2.0)
        .with_background(theme::accent_cool_bg(36))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
        .finish()
    }

    fn collapsible_row(
        &self,
        label: &str,
        summary: &str,
        expanded: bool,
        action: SettingsAction,
    ) -> Box<dyn Element> {
        let chevron_path = settings_fold_chevron_path(expanded);
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(icons::icon(
            chevron_path,
            SETTINGS_FOLD_CHEVRON_SIZE,
            theme::muted(),
        ));
        row.add_child(
            Container::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_left(8.0)
            .finish(),
        );
        row.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        row.add_child(self.fold_summary_badge(summary));
        let row = EventHandler::new(
            Container::new(row.finish())
                .with_padding_left(12.0)
                .with_padding_right(12.0)
                .with_padding_top(9.0)
                .with_padding_bottom(9.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish();
        ConstrainedBox::new(row)
            .with_max_width(SETTINGS_FOLD_TOGGLE_MAX_WIDTH)
            .finish()
    }

    fn relay_details(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            "集群 P2P 穿透依赖 Relay。自动模式会并行探测国内/海外节点并选择最快可达者。",
            self.font,
        ));

        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(
            Container::new(self.relay_option_button("auto", "自动"))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        row.add_child(
            Container::new(self.relay_option_button("domestic", "国内"))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        row.add_child(
            Container::new(self.relay_option_button("overseas", "海外"))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        col.add_child(row.finish());

        if let Some(status) = &self.relay_status {
            if status.env_override {
                col.add_child(
                    ui_text::body("环境变量 WORMHOLE_IROH_RELAYS 已接管 Relay", self.font)
                        .with_color(theme::danger())
                        .finish(),
                );
            }
            if let Some(url) = status.resolved_relay.as_deref() {
                col.add_child(self.path_row("当前生效", url));
            } else if !status.effective_urls.is_empty() {
                col.add_child(self.path_row("当前生效", &status.effective_urls.join(", ")));
            }
            for probe in &status.probe_results {
                let line = if probe.reachable {
                    format!(
                        "{} {} — {}ms",
                        probe.mode,
                        probe.url,
                        probe.latency_ms.unwrap_or(0)
                    )
                } else {
                    format!(
                        "{} {} — 不可达{}",
                        probe.mode,
                        probe.url,
                        probe
                            .error
                            .as_deref()
                            .map(|err| format!(" ({err})"))
                            .unwrap_or_default()
                    )
                };
                col.add_child(
                    ui_text::mono(line, self.font)
                        .with_color(if probe.reachable {
                            theme::muted()
                        } else {
                            theme::danger()
                        })
                        .finish(),
                );
            }
        }

        let mut actions = Flex::row();
        actions.add_child(
            Container::new(self.stateful_action_button(
                if self.relay_busy {
                    "正在应用…"
                } else {
                    "应用并重启 P2P"
                },
                SettingsAction::ApplyRelay,
                self.relay_busy,
                true,
            ))
            .with_horizontal_margin(4.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.stateful_action_button(
                "刷新",
                SettingsAction::RefreshRelay,
                self.relay_busy,
                false,
            ))
            .with_horizontal_margin(4.0)
            .finish(),
        );
        col.add_child(actions.finish());

        if !self.relay_message.is_empty() {
            col.add_child(status_line(
                self.relay_message.clone(),
                self.font,
                self.relay_tone,
            ));
        }
        ConstrainedBox::new(col.finish())
            .with_max_width(720.0)
            .finish()
    }

    fn relay_block(&self) -> Box<dyn Element> {
        let summary = relay_mode_label(&self.relay_mode);
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("P2P · Relay", self.font));
        col.add_child(self.collapsible_row(
            "网络穿透",
            summary,
            self.relay_expanded,
            SettingsAction::ToggleRelaySection,
        ));
        if self.relay_expanded {
            col.add_child(
                Container::new(self.relay_details())
                    .with_margin_top(8.0)
                    .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn shared_path_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("DATA · 共享文件存放位置", self.font));
        col.add_child(section_hint(
            "终端共享文件夹的本地副本写入此目录。点击「浏览…」选择新位置并迁移数据。",
            self.font,
        ));

        if let Some(info) = &self.storage {
            col.add_child(
                ui_text::body("当前同步目录", self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
            let draft = self.storage_draft.clone();
            let marked = self.storage_field.marked_text.clone();
            let field = TextFieldInput::builder(
                EventHandler::new(
                    Container::new(render_field_with_caret(
                        &draft,
                        &marked,
                        "选择共享文件存放位置",
                        self.font,
                        self.storage_focused,
                        self.busy,
                        true,
                        self.storage_field.cursor,
                    ))
                    .with_uniform_padding(10.0)
                    .with_background(theme::bg())
                    .with_border(Border::all(1.0).with_border_fill(if self.storage_dirty() {
                        theme::accent_cool()
                    } else {
                        theme::border()
                    }))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                    .finish(),
                )
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(SettingsAction::FocusPath);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
                |ctx, action| {
                    ctx.dispatch_typed_action(SettingsAction::TextFieldEdit(action));
                },
            )
            .focused(self.storage_focused)
            .disabled(self.busy)
            .ime_preedit(!marked.is_empty())
            .finish();
            let mut path_row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max);
            path_row.add_child(Expanded::new(1.0, field).finish());
            path_row.add_child(
                Container::new(self.stateful_action_button(
                    "浏览…",
                    SettingsAction::BrowseMigrate,
                    self.busy,
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            path_row.add_child(
                Container::new(self.stateful_action_button(
                    if self.busy { "保存中…" } else { "保存" },
                    SettingsAction::SaveMigration,
                    self.busy || !self.storage_dirty(),
                    true,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            path_row.add_child(
                Container::new(self.action_button("刷新", SettingsAction::Refresh))
                    .with_margin_left(8.0)
                    .finish(),
            );
            col.add_child(
                ConstrainedBox::new(
                    Container::new(path_row.finish())
                        .with_vertical_margin(4.0)
                        .finish(),
                )
                .with_max_width(SETTINGS_FORM_MAX_WIDTH)
                .finish(),
            );
            if info.physical_path != info.sync_entry_path {
                col.add_child(
                    Container::new(self.path_row("物理存放", &info.physical_path))
                        .with_vertical_margin(6.0)
                        .finish(),
                );
            }
            if let Some(cluster) = &info.active_cluster_path {
                col.add_child(
                    Container::new(
                        ui_text::body(format!("当前集群目录: {cluster}"), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_vertical_margin(6.0)
                    .finish(),
                );
            }
        } else {
            col.add_child(
                ui_text::body("加载中…", self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        if !self.status.is_empty() {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }
        self.flat_section(col.finish())
    }

    fn account_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("ACCOUNT · 账号", self.font));
        col.add_child(section_hint(
            "登录后本机会绑定硬件码并在云端保存设备身份；重装系统后可自动恢复同一设备。",
            self.font,
        ));
        if let Some(user_id) = &self.auth_user_id {
            col.add_child(
                ui_text::mono(format!("用户 ID: {user_id}"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }
        if let Some(device_id) = &self.auth_device_id {
            col.add_child(
                ui_text::mono(format!("设备 ID: {device_id}"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }
        if !self.auth_status.is_empty() {
            col.add_child(status_line(
                self.auth_status.clone(),
                self.font,
                self.auth_status_tone,
            ));
        }
        let mut auth_actions = Vec::new();
        if self.auth_user_id.is_none() {
            auth_actions.push(self.stateful_action_button(
                if self.auth_busy {
                    "正在读取账号…"
                } else {
                    "登录 Wormhole"
                },
                SettingsAction::Login,
                self.auth_busy,
                true,
            ));
        } else {
            auth_actions.push(self.stateful_action_button(
                if self.auth_busy {
                    "正在退出…"
                } else {
                    "退出登录"
                },
                SettingsAction::Logout,
                self.auth_busy,
                false,
            ));
        }
        auth_actions.push(self.stateful_action_button(
            "刷新账号状态",
            SettingsAction::RefreshAccount,
            self.auth_busy,
            false,
        ));
        col.add_child(
            Container::new(self.inline_action_row(auth_actions))
                .with_vertical_margin(8.0)
                .finish(),
        );
        self.flat_section(col.finish())
    }

    fn email_connector_row(&self, provider: &str, label: &str) -> Box<dyn Element> {
        let connection = self
            .email_connectors
            .iter()
            .find(|item| item.provider == provider && item.connected);
        let detail = connection
            .and_then(|item| item.email.as_deref())
            .unwrap_or("未连接")
            .to_string();
        let mut labels = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        labels.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        labels.add_child(
            ui_text::mono(detail, self.font)
                .with_color(if connection.is_some() {
                    theme::accent_cool()
                } else {
                    theme::muted()
                })
                .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, labels.finish()).finish());
        if connection.is_some() {
            row.add_child(
                Container::new(self.stateful_action_button(
                    "测试读取",
                    SettingsAction::TestEmail(provider.to_string()),
                    self.email_busy,
                    false,
                ))
                .with_margin_right(8.0)
                .finish(),
            );
            row.add_child(self.stateful_action_button(
                "断开",
                SettingsAction::DisconnectEmail(provider.to_string()),
                self.email_busy,
                false,
            ));
        } else {
            row.add_child(self.stateful_action_button(
                "连接",
                SettingsAction::ConnectEmail(provider.to_string()),
                self.email_busy || self.auth_user_id.is_none(),
                true,
            ));
        }
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn email_connectors_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("CONNECTORS · 邮箱", self.font));
        col.add_child(section_hint(
            "通过官方 OAuth 连接 Gmail 或 Outlook。Wormhole 不保存邮箱密码；发送和修改邮件需要明确确认。",
            self.font,
        ));
        col.add_child(self.email_connector_row("gmail", "Gmail"));
        col.add_child(
            Container::new(self.email_connector_row("outlook", "Microsoft Outlook"))
                .with_margin_top(8.0)
                .finish(),
        );
        col.add_child(
            Container::new(self.stateful_action_button(
                if self.email_busy {
                    "正在刷新…"
                } else {
                    "刷新连接状态"
                },
                SettingsAction::RefreshEmailConnectors,
                self.email_busy || self.auth_user_id.is_none(),
                false,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        if !self.email_message.is_empty() {
            col.add_child(status_line(
                self.email_message.clone(),
                self.font,
                self.email_tone,
            ));
        }
        self.flat_section(col.finish())
    }

    fn cluster_block(&self) -> Box<dyn Element> {
        let summary = self
            .cluster_name
            .as_deref()
            .or(self.cluster_id.as_deref())
            .unwrap_or("未加入集群");
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("CLUSTER · 集群", self.font));
        col.add_child(self.collapsible_row(
            "活动集群",
            summary,
            self.cluster_expanded,
            SettingsAction::ToggleClusterSection,
        ));
        if self.cluster_expanded {
            let mut details = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            if let Some(cluster_id) = self.cluster_id.as_deref() {
                details.add_child(self.path_row("Cluster ID", cluster_id));
            }
            if let Some(name) = self.cluster_name.as_deref() {
                details.add_child(
                    Container::new(self.path_row("当前名称", name))
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
            if self.cluster_id.is_none() {
                details.add_child(section_hint("当前没有活动集群。", self.font));
            }
            details.add_child(
                Container::new(
                    self.action_button("前往终端管理", SettingsAction::OpenClusterManagement),
                )
                .with_margin_top(8.0)
                .finish(),
            );
            if !self.cluster_message.is_empty() {
                details.add_child(status_line(
                    self.cluster_message.clone(),
                    self.font,
                    StatusTone::Danger,
                ));
            }
            col.add_child(
                Container::new(
                    ConstrainedBox::new(details.finish())
                        .with_max_width(720.0)
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn cache_row(
        &self,
        label: &str,
        description: &str,
        bytes: Option<u64>,
        kind: &str,
    ) -> Box<dyn Element> {
        let mut labels = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        labels.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        labels.add_child(
            ui_text::body(description.to_string(), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, labels.finish()).finish());
        row.add_child(
            Container::new(
                ui_text::mono(
                    bytes
                        .map(format_cache_size)
                        .unwrap_or_else(|| "统计中…".into()),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_horizontal_margin(10.0)
            .finish(),
        );
        row.add_child(self.stateful_action_button(
            if self.cache_busy {
                "处理中…"
            } else {
                "清理"
            },
            SettingsAction::ClearCache(kind.to_string()),
            self.cache_busy || bytes.unwrap_or(0) == 0,
            false,
        ));
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn cache_block(&self) -> Box<dyn Element> {
        let summary = cache_summary_label(self.cache_status.as_ref());
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("CACHE · 本地缓存", self.font));
        col.add_child(self.collapsible_row(
            "缓存详情",
            &summary,
            self.cache_expanded,
            SettingsAction::ToggleCacheSection,
        ));
        if self.cache_expanded {
            let chat_bytes = self.cache_status.as_ref().map(|status| status.chat_bytes);
            let sync_bytes = self.cache_status.as_ref().map(|status| status.sync_bytes);
            let total = cache_total_bytes(self.cache_status.as_ref());
            let mut details = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            details.add_child(section_hint(
                "只清理可从聊天文档或远端文件重新生成的数据，不会删除共享文件、聊天索引或集群配置。",
                self.font,
            ));
            details.add_child(self.cache_row(
                "聊天记录缓存",
                "本地消息查询库与媒体派生文件",
                chat_bytes,
                "chat",
            ));
            details.add_child(
                Container::new(self.cache_row(
                    "终端同步文件缓存",
                    "预览文件与工作区传输临时文件",
                    sync_bytes,
                    "sync",
                ))
                .with_margin_top(6.0)
                .finish(),
            );
            let mut actions = Flex::row();
            actions.add_child(self.stateful_action_button(
                if self.cache_busy {
                    "正在清理…"
                } else {
                    "清理全部缓存"
                },
                SettingsAction::ClearCache("all".into()),
                self.cache_busy || total.unwrap_or(0) == 0,
                true,
            ));
            actions.add_child(
                Container::new(self.stateful_action_button(
                    "重新统计",
                    SettingsAction::RefreshCache,
                    self.cache_busy,
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            details.add_child(
                Container::new(actions.finish())
                    .with_margin_top(8.0)
                    .finish(),
            );
            if !self.cache_message.is_empty() {
                details.add_child(status_line(
                    self.cache_message.clone(),
                    self.font,
                    self.cache_tone,
                ));
            }
            col.add_child(
                Container::new(
                    ConstrainedBox::new(details.finish())
                        .with_max_width(720.0)
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn archive_block(&self) -> Box<dyn Element> {
        let data_dir = self.core.data_dir();
        let snapshots = load_archived_snapshots(&data_dir);
        let count = snapshots.len();
        let count_label = count.to_string();

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("ARCHIVE · 历史对话归档", self.font));
        col.add_child(section_hint(
            "从智能体归档的对话会显示在这里，可恢复至项目或独立对话，或永久删除。",
            self.font,
        ));
        col.add_child(self.collapsible_row(
            "已归档会话",
            &count_label,
            self.archive_expanded,
            SettingsAction::ToggleArchiveSection,
        ));

        if self.archive_expanded {
            let mut list_col =
                Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            if snapshots.is_empty() {
                list_col.add_child(
                    Container::new(section_hint("暂无归档会话", self.font))
                        .with_padding_top(4.0)
                        .with_padding_bottom(4.0)
                        .finish(),
                );
            } else {
                for snapshot in snapshots {
                    list_col.add_child(self.archived_session_row(snapshot));
                }
            }
            col.add_child(
                ConstrainedBox::new(
                    Container::new(list_col.finish())
                        .with_margin_top(4.0)
                        .with_uniform_padding(4.0)
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                        .finish(),
                )
                .with_max_width(520.0)
                .finish(),
            );
        }

        self.flat_section(col.finish())
    }

    fn archived_session_row(&self, snapshot: ArchivedSessionSnapshot) -> Box<dyn Element> {
        let session_id = snapshot.id.clone();
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        let mut label_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        label_col.add_child(
            ui_text::body(snapshot.label, self.font)
                .with_color(theme::text())
                .finish(),
        );
        label_col.add_child(
            ui_text::mono(snapshot.time, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        row.add_child(Expanded::new(1.0, label_col.finish()).finish());
        row.add_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("恢复", self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .on_left_mouse_down({
                    let id = session_id.clone();
                    move |ctx, _, _| {
                        ctx.dispatch_typed_action(SettingsAction::RestoreArchivedSession(
                            id.clone(),
                        ));
                        DispatchEventResult::StopPropagation
                    }
                })
                .finish(),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        row.add_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("永久删除", self.font)
                        .with_color(theme::danger())
                        .finish(),
                )
                .on_left_mouse_down({
                    let id = session_id.clone();
                    move |ctx, _, _| {
                        ctx.dispatch_typed_action(SettingsAction::DeleteArchivedSession(
                            id.clone(),
                        ));
                        DispatchEventResult::StopPropagation
                    }
                })
                .finish(),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        Container::new(row.finish())
            .with_uniform_padding(8.0)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish()
    }
}

impl Entity for SettingsView {
    type Event = SettingsEvent;
}

impl View for SettingsView {
    fn ui_name() -> &'static str {
        "SettingsView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("设置", self.font));
        col.add_child(self.account_block());
        col.add_child(self.email_connectors_block());
        col.add_child(self.shared_path_block());
        col.add_child(self.cluster_block());
        col.add_child(self.relay_block());
        col.add_child(self.cache_block());
        col.add_child(self.archive_block());
        col.add_child(
            ui_text::body(crate::ui::fonts::UI_FONT_ATTRIBUTION, self.font)
                .with_color(theme::placeholder())
                .finish(),
        );

        let body = Container::new(col.finish())
            .with_uniform_padding(20.0)
            .finish();
        tab_content_fill(
            Container::new(
                ClippedScrollable::vertical(
                    self.scroll.clone(),
                    body,
                    ScrollbarWidth::Auto,
                    Fill::None,
                    Fill::None,
                    Fill::None,
                )
                .finish(),
            )
            .with_background(theme::panel())
            .finish(),
        )
    }
}

impl TypedActionView for SettingsView {
    type Action = SettingsAction;

    fn handle_action(&mut self, action: &SettingsAction, ctx: &mut ViewContext<Self>) {
        match action {
            SettingsAction::Refresh => self.refresh(ctx),
            SettingsAction::RefreshAccount => self.refresh_account(ctx),
            SettingsAction::RefreshEmailConnectors => self.refresh_email_connectors(ctx),
            SettingsAction::ConnectEmail(provider) => {
                if self.email_busy {
                    return;
                }
                self.email_busy = true;
                self.email_message =
                    format!("正在创建 {} 授权链接…", email_provider_label(provider));
                self.email_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                let provider = provider.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        let result = email_connector_start_oauth(&state, &provider).await?;
                        tokio::task::spawn_blocking(move || {
                            open_external_url(&result.authorization_url)
                        })
                        .await
                        .map_err(|error| format!("启动浏览器失败: {error}"))??;
                        Ok::<(), String>(())
                    },
                    |view, output, ctx| {
                        view.email_busy = false;
                        match output {
                            Ok(()) => {
                                view.email_message =
                                    "授权页已在系统浏览器打开；完成后点击“刷新连接状态”。".into();
                                view.email_tone = StatusTone::Placeholder;
                            }
                            Err(error) => {
                                view.email_message = format!("连接邮箱失败: {error}");
                                view.email_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::DisconnectEmail(provider) => {
                if self.email_busy {
                    return;
                }
                self.email_busy = true;
                let core = self.core.clone();
                let provider = provider.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        email_connector_disconnect(&state, &provider).await
                    },
                    |view, output, ctx| {
                        view.email_busy = false;
                        match output {
                            Ok(()) => {
                                view.email_message = "邮箱已断开。".into();
                                view.email_tone = StatusTone::Success;
                                view.refresh_email_connectors(ctx);
                            }
                            Err(error) => {
                                view.email_message = format!("断开邮箱失败: {error}");
                                view.email_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::TestEmail(provider) => {
                if self.email_busy {
                    return;
                }
                self.email_busy = true;
                let core = self.core.clone();
                let provider = provider.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        email_connector_test_read(&state, &provider).await
                    },
                    |view, output, ctx| {
                        view.email_busy = false;
                        match output {
                            Ok(result) => {
                                view.email_message =
                                    format!("邮箱读取成功，返回 {} 封邮件。", result.count);
                                view.email_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.email_message = format!("邮箱读取失败: {error}");
                                view.email_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::RefreshRelay => self.refresh_relay(ctx),
            SettingsAction::RefreshCache => self.refresh_cache(ctx),
            SettingsAction::ToggleClusterSection => {
                self.cluster_expanded = !self.cluster_expanded;
                ctx.notify();
            }
            SettingsAction::OpenClusterManagement => {
                ctx.emit(SettingsEvent::OpenClusterManagement);
            }
            SettingsAction::ToggleRelaySection => {
                self.relay_expanded = !self.relay_expanded;
                ctx.notify();
            }
            SettingsAction::ToggleCacheSection => {
                self.cache_expanded = !self.cache_expanded;
                ctx.notify();
            }
            SettingsAction::SelectRelay(mode) => {
                if is_relay_mode(mode) && !self.relay_busy {
                    self.relay_mode = mode.clone();
                    self.relay_message = "Relay 模式已修改，点击「应用并重启 P2P」生效。".into();
                    self.relay_tone = StatusTone::Placeholder;
                    ctx.notify();
                }
            }
            SettingsAction::ApplyRelay => {
                if self.relay_busy {
                    return;
                }
                self.relay_busy = true;
                self.relay_message = "正在应用 Relay 并重启 P2P 节点…".into();
                self.relay_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                let relay_mode = self.relay_mode.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        save_network_relay_config(&state, SaveNetworkRelayParams { relay_mode })
                            .await
                    },
                    |view, output, ctx| {
                        view.relay_busy = false;
                        match output {
                            Ok(status) => {
                                view.relay_mode = status.relay_mode.clone();
                                view.relay_status = Some(status);
                                view.relay_message = "Relay 已应用，P2P 节点已重启。".into();
                                view.relay_tone = StatusTone::Success;
                            }
                            Err(err) => {
                                view.relay_message = format!("应用 Relay 失败: {err}");
                                view.relay_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::Login => {
                ctx.emit(SettingsEvent::OpenLogin);
            }
            SettingsAction::Logout => {
                self.auth_busy = true;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        clear_cloud_auth_token(&state).await
                    },
                    |view, output, ctx| {
                        view.auth_busy = false;
                        match output {
                            Ok(()) => {
                                view.auth_user_id = None;
                                view.auth_device_id = None;
                                view.auth_status = "已退出登录。".into();
                                view.auth_status_tone = StatusTone::Placeholder;
                                ctx.emit(SettingsEvent::AccountChanged {
                                    authenticated: false,
                                });
                            }
                            Err(err) => {
                                view.auth_status = format!("退出失败: {err}");
                                view.auth_status_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::BrowseMigrate => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.status = "正在选择文件夹…".into();
                self.status_tone = StatusTone::Placeholder;
                ctx.notify();
                ctx.spawn(
                    async move {
                        let picked = tokio::task::spawn_blocking(|| {
                            #[cfg(windows)]
                            {
                                wormhole_desktop_platform_windows::pick_folder(
                                    "选择共享文件存放位置",
                                )
                            }
                            #[cfg(not(windows))]
                            {
                                rfd::FileDialog::new()
                                    .set_title("选择共享文件存放位置")
                                    .pick_folder()
                            }
                        })
                        .await
                        .ok()
                        .flatten();

                        let Some(path) = picked else {
                            return None;
                        };
                        Some(path.display().to_string())
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Some(path) => {
                                view.storage_draft = path;
                                view.storage_field.clear_marked();
                                view.storage_focused = true;
                                view.status = "已选择文件夹，点击「保存」开始迁移。".into();
                                view.status_tone = StatusTone::Placeholder;
                            }
                            None => {
                                view.status = "已取消选择。".into();
                                view.status_tone = StatusTone::Placeholder;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::SaveMigration => {
                if self.busy || !self.storage_dirty() {
                    return;
                }
                let target = self.storage_draft.trim().to_string();
                if target.is_empty() {
                    self.status = "共享文件路径不能为空。".into();
                    self.status_tone = StatusTone::Danger;
                    ctx.notify();
                    return;
                }
                self.busy = true;
                self.storage_focused = false;
                self.status = "正在迁移共享文件…".into();
                self.status_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.notify();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        migrate_shared_storage(&state, target).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(info) => {
                                view.storage_draft = info.sync_entry_path.clone();
                                view.status = format!("已迁移共享文件至 {}", info.sync_entry_path);
                                view.storage = Some(info);
                                view.status_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.status = format!("迁移失败: {error}");
                                view.status_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::FocusPath => {
                if !self.busy {
                    self.storage_focused = true;
                    ctx.notify();
                }
            }
            SettingsAction::TextFieldEdit(action) => {
                if !self.busy {
                    self.storage_field.apply(&mut self.storage_draft, action);
                    self.status.clear();
                    ctx.notify();
                }
            }
            SettingsAction::ToggleArchiveSection => {
                self.archive_expanded = !self.archive_expanded;
                let value = if self.archive_expanded { "1" } else { "0" };
                if let Err(error) = std::fs::write(Self::archive_expanded_path(&self.core), value) {
                    self.status = format!("保存归档折叠状态失败: {error}");
                    self.status_tone = StatusTone::Danger;
                }
                ctx.notify();
            }
            SettingsAction::RestoreArchivedSession(id) => {
                ctx.emit(SettingsEvent::RestoreArchivedSession(id.clone()));
            }
            SettingsAction::DeleteArchivedSession(id) => {
                ctx.emit(SettingsEvent::DeleteArchivedSession(id.clone()));
            }
            SettingsAction::ClearCache(kind) => {
                if self.cache_busy {
                    return;
                }
                self.cache_busy = true;
                self.cache_message = "正在安全清理缓存…".into();
                self.cache_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                let kind = kind.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        clear_settings_cache(&state, ClearSettingsCacheParams { kind }).await
                    },
                    |view, output, ctx| {
                        view.cache_busy = false;
                        match output {
                            Ok(result) => {
                                view.cache_status = Some(result.status);
                                view.cache_message = format!(
                                    "缓存清理完成，已释放 {}。",
                                    format_cache_size(result.removed_bytes)
                                );
                                view.cache_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.cache_message = format!("缓存清理失败: {error}");
                                view.cache_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
        }
    }
}

fn storage_paths_differ(current: &str, draft: &str) -> bool {
    current.trim() != draft.trim()
}

fn email_provider_label(provider: &str) -> &'static str {
    match provider {
        "gmail" => "Gmail",
        "outlook" => "Microsoft Outlook",
        _ => "邮箱",
    }
}

fn open_external_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1:")) {
        return Err("拒绝打开非 HTTPS 授权地址".into());
    }
    #[cfg(windows)]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .status();
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    status
        .map_err(|error| error.to_string())?
        .success()
        .then_some(())
        .ok_or_else(|| "系统浏览器启动失败".into())
}

const SETTINGS_FOLD_TOGGLE_MAX_WIDTH: f32 = 420.0;
const SETTINGS_FORM_MAX_WIDTH: f32 = 560.0;
const SETTINGS_FOLD_CHEVRON_SIZE: f32 = 14.0;

fn settings_fold_chevron_path(expanded: bool) -> &'static str {
    if expanded {
        "agent-chevron-down.svg"
    } else {
        "agent-chevron.svg"
    }
}

fn cache_total_bytes(status: Option<&SettingsCacheStatusDto>) -> Option<u64> {
    status.map(|entry| entry.chat_bytes.saturating_add(entry.sync_bytes))
}

fn cache_summary_label(status: Option<&SettingsCacheStatusDto>) -> String {
    cache_total_bytes(status)
        .map(format_cache_size)
        .unwrap_or_else(|| "统计中…".into())
}

fn is_relay_mode(mode: &str) -> bool {
    matches!(mode, "auto" | "domestic" | "overseas")
}

fn relay_mode_label(mode: &str) -> &'static str {
    match mode {
        "domestic" => "国内 Relay",
        "overseas" => "海外 Relay",
        _ => "自动选择",
    }
}

fn format_cache_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cache_summary_label, email_provider_label, format_cache_size, is_relay_mode,
        relay_mode_label, settings_fold_chevron_path, storage_paths_differ,
    };

    #[test]
    fn email_provider_labels_are_stable() {
        assert_eq!(email_provider_label("gmail"), "Gmail");
        assert_eq!(email_provider_label("outlook"), "Microsoft Outlook");
    }
    use wormhole_desktop_core::settings_cache_commands::SettingsCacheStatusDto;

    #[test]
    fn storage_dirty_state_ignores_outer_whitespace_only() {
        assert!(!storage_paths_differ("D:\\Wormhole", " D:\\Wormhole "));
        assert!(storage_paths_differ("D:\\Wormhole", "E:\\Wormhole"));
    }

    #[test]
    fn relay_mode_accepts_only_backend_modes() {
        assert!(is_relay_mode("auto"));
        assert!(is_relay_mode("domestic"));
        assert!(is_relay_mode("overseas"));
        assert!(!is_relay_mode("fastest"));
    }

    #[test]
    fn setting_summaries_are_compact_and_stable() {
        assert_eq!(relay_mode_label("auto"), "自动选择");
        assert_eq!(relay_mode_label("domestic"), "国内 Relay");
        assert_eq!(format_cache_size(0), "0 B");
        assert_eq!(format_cache_size(1536), "1.5 KB");
        assert_eq!(format_cache_size(2 * 1024 * 1024), "2.0 MB");
    }

    #[test]
    fn settings_fold_chevron_paths_match_expanded_state() {
        assert_eq!(settings_fold_chevron_path(false), "agent-chevron.svg");
        assert_eq!(settings_fold_chevron_path(true), "agent-chevron-down.svg");
    }

    #[test]
    fn cache_summary_reports_total_or_pending() {
        assert_eq!(cache_summary_label(None), "统计中…");
        let status = SettingsCacheStatusDto {
            chat_bytes: 1024,
            sync_bytes: 2048,
        };
        assert_eq!(cache_summary_label(Some(&status)), "3.0 KB");
    }
}
