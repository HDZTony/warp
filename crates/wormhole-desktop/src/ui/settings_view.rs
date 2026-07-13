use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext};

use crate::ui::agent_panel::sidebar::{load_archived_snapshots, ArchivedSessionSnapshot};
use crate::ui::agent_providers_view::AgentProvidersView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, view_panel, StatusTone,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::sync_commands::{
    migrate_shared_storage, shared_storage_info, SharedStorageInfoDto,
};
use wormhole_desktop_core::{
    clear_cloud_auth_token, clear_desktop_error_events, cloud_auth_status,
    configure_desktop_error_settings, desktop_error_status, sync_desktop_errors_now,
    DesktopErrorSettingsParams, DesktopErrorStatusDto,
};

#[derive(Debug, Clone)]
pub enum SettingsEvent {
    AccountChanged { authenticated: bool },
    OpenLogin,
    RestoreArchivedSession(String),
    DeleteArchivedSession(String),
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    BrowseMigrate,
    Refresh,
    Login,
    Logout,
    RefreshAccount,
    RefreshDiagnostics,
    ToggleDiagnosticUpload,
    UploadDiagnosticsNow,
    ClearDiagnosticQueue,
    ToggleArchiveSection,
    RestoreArchivedSession(String),
    DeleteArchivedSession(String),
}

pub struct SettingsView {
    core: CoreHandle,
    font: FamilyId,
    storage: Option<SharedStorageInfoDto>,
    status: String,
    status_tone: StatusTone,
    busy: bool,
    auth_user_id: Option<String>,
    auth_status: String,
    auth_status_tone: StatusTone,
    auth_busy: bool,
    auth_device_id: Option<String>,
    diagnostic_status: Option<DesktopErrorStatusDto>,
    diagnostic_message: String,
    diagnostic_tone: StatusTone,
    diagnostic_busy: bool,
    archive_expanded: bool,
    agent_providers: warpui::ViewHandle<AgentProvidersView>,
}

impl SettingsView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let agent_providers =
            ctx.add_typed_action_view(|ctx| AgentProvidersView::new(ctx, core.clone()));
        let mut view = Self {
            core,
            font,
            storage: None,
            status: String::new(),
            status_tone: StatusTone::Placeholder,
            busy: false,
            auth_user_id: None,
            auth_status: String::new(),
            auth_status_tone: StatusTone::Placeholder,
            auth_busy: false,
            auth_device_id: None,
            diagnostic_status: None,
            diagnostic_message: String::new(),
            diagnostic_tone: StatusTone::Placeholder,
            diagnostic_busy: false,
            archive_expanded: false,
            agent_providers,
        };
        view.refresh(ctx);
        view.refresh_account(ctx);
        view.refresh_diagnostics(ctx);
        view
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

    pub fn agent_providers_view(&self) -> &warpui::ViewHandle<AgentProvidersView> {
        &self.agent_providers
    }

    fn refresh_diagnostics(&mut self, ctx: &mut ViewContext<Self>) {
        self.diagnostic_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                desktop_error_status(&state).await
            },
            |view, output, ctx| {
                view.diagnostic_busy = false;
                match output {
                    Ok(status) => {
                        view.diagnostic_status = Some(status);
                        if view.diagnostic_message.is_empty() {
                            view.diagnostic_message =
                                "错误会先保存在本机；开启后会在登录且设备就绪时上传诊断事件。"
                                    .into();
                            view.diagnostic_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(err) => {
                        view.diagnostic_message = format!("读取诊断状态失败: {err}");
                        view.diagnostic_tone = StatusTone::Danger;
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

    fn action_button(&self, label: &str, action: SettingsAction) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_bg(24))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn diagnostic_button(&self, label: &str, action: SettingsAction) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_bg(18))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn shared_path_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("DATA · 共享文件存放位置", self.font));
        col.add_child(section_hint(
            "终端共享文件夹的本地副本写入此目录。点击「浏览…」选择新位置并迁移数据。",
            self.font,
        ));

        if let Some(info) = &self.storage {
            col.add_child(self.path_row("当前同步目录", &info.sync_entry_path));
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

        col.add_child(
            Container::new(self.action_button("浏览…", SettingsAction::BrowseMigrate))
                .with_vertical_margin(8.0)
                .finish(),
        );

        col.add_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("刷新", self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(SettingsAction::Refresh);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );

        if !self.status.is_empty() {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }
        section_card(col.finish())
    }

    fn account_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("ACCOUNT · 账号", self.font));
        col.add_child(section_hint(
            "登录后本机会绑定硬件码并在云端保存设备身份；重装系统后可自动恢复同一设备。",
            self.font,
        ));
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
        if self.auth_user_id.is_none() {
            col.add_child(
                Container::new(
                    EventHandler::new(
                        ui_text::body("登录 Wormhole", self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(SettingsAction::Login);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_uniform_padding(10.0)
                .with_vertical_margin(8.0)
                .with_background(theme::accent_bg(24))
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
            );
        } else {
            col.add_child(
                Container::new(
                    EventHandler::new(
                        ui_text::body("退出登录", self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(SettingsAction::Logout);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_vertical_margin(8.0)
                .finish(),
            );
        }
        section_card(col.finish())
    }

    fn diagnostics_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("DIAGNOSTICS · 错误捕获", self.font));
        col.add_child(section_hint(
            "本机自动记录 panic、warn/error 日志和崩溃文件；诊断上传默认关闭。",
            self.font,
        ));

        if let Some(status) = &self.diagnostic_status {
            let upload = if status.upload_enabled {
                "已开启"
            } else {
                "已关闭"
            };
            col.add_child(
                ui_text::body(
                    format!("诊断上传: {upload} · 待上传事件: {}", status.pending_events),
                    self.font,
                )
                .with_color(theme::text())
                .finish(),
            );
            col.add_child(
                ui_text::mono(format!("日志: {}", status.log_path), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
            col.add_child(
                ui_text::mono(format!("队列: {}", status.error_events_path), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        let mut row = Flex::row();
        let toggle_label = if self
            .diagnostic_status
            .as_ref()
            .map(|status| status.upload_enabled)
            .unwrap_or(false)
        {
            "关闭上传"
        } else {
            "开启上传"
        };
        row.add_child(
            Container::new(
                self.diagnostic_button(toggle_label, SettingsAction::ToggleDiagnosticUpload),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        row.add_child(
            Container::new(
                self.diagnostic_button("立即上传", SettingsAction::UploadDiagnosticsNow),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        row.add_child(
            Container::new(
                self.diagnostic_button("清空本地队列", SettingsAction::ClearDiagnosticQueue),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        row.add_child(
            Container::new(self.diagnostic_button("刷新", SettingsAction::RefreshDiagnostics))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        col.add_child(row.finish());

        if !self.diagnostic_message.is_empty() {
            col.add_child(status_line(
                self.diagnostic_message.clone(),
                self.font,
                self.diagnostic_tone,
            ));
        }
        section_card(col.finish())
    }

    fn archive_block(&self) -> Box<dyn Element> {
        let data_dir = self.core.data_dir();
        let snapshots = load_archived_snapshots(&data_dir);
        let count = snapshots.len();
        let expanded = self.archive_expanded;
        let chevron = if expanded { "▼" } else { "▶" };

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("ARCHIVE · 历史对话归档", self.font));
        col.add_child(section_hint(
            "从智能体归档的对话会显示在这里，可恢复至项目或独立对话，或永久删除。",
            self.font,
        ));

        let toggle_label = format!("{chevron} 已归档会话");
        let toggle_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::body(toggle_label, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                Container::new(
                    ui_text::mono(count.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(4.0)
                .with_background(theme::accent_cool_bg(24))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .with_horizontal_margin(8.0)
                .finish(),
            );
        let toggle = EventHandler::new(
            Container::new(toggle_row.finish())
                .with_uniform_padding(10.0)
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(SettingsAction::ToggleArchiveSection);
            DispatchEventResult::StopPropagation
        })
        .finish();
        col.add_child(toggle);

        if expanded {
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
                Container::new(list_col.finish())
                    .with_margin_top(4.0)
                    .with_uniform_padding(4.0)
                    .with_border(Border::all(1.0).with_border_fill(theme::border()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                    .finish(),
            );
        }

        section_card(col.finish())
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
        col.add_child(self.diagnostics_block());
        col.add_child(self.shared_path_block());
        col.add_child(self.archive_block());
        col.add_child(
            ui_text::body(crate::ui::fonts::UI_FONT_ATTRIBUTION, self.font)
                .with_color(theme::placeholder())
                .finish(),
        );

        view_panel(col.finish())
    }
}

impl TypedActionView for SettingsView {
    type Action = SettingsAction;

    fn handle_action(&mut self, action: &SettingsAction, ctx: &mut ViewContext<Self>) {
        match action {
            SettingsAction::Refresh => self.refresh(ctx),
            SettingsAction::RefreshAccount => self.refresh_account(ctx),
            SettingsAction::RefreshDiagnostics => self.refresh_diagnostics(ctx),
            SettingsAction::ToggleDiagnosticUpload => {
                self.diagnostic_busy = true;
                self.diagnostic_message = "正在保存诊断上传设置…".into();
                self.diagnostic_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                let upload_enabled = !self
                    .diagnostic_status
                    .as_ref()
                    .map(|status| status.upload_enabled)
                    .unwrap_or(false);
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        configure_desktop_error_settings(
                            &state,
                            DesktopErrorSettingsParams { upload_enabled },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.diagnostic_busy = false;
                        match output {
                            Ok(status) => {
                                let enabled = status.upload_enabled;
                                view.diagnostic_status = Some(status);
                                view.diagnostic_message = if enabled {
                                    "已开启诊断上传。".into()
                                } else {
                                    "已关闭诊断上传。".into()
                                };
                                view.diagnostic_tone = StatusTone::Success;
                            }
                            Err(err) => {
                                view.diagnostic_message = format!("保存诊断设置失败: {err}");
                                view.diagnostic_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::UploadDiagnosticsNow => {
                self.diagnostic_busy = true;
                self.diagnostic_message = "正在上传诊断事件…".into();
                self.diagnostic_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        let upload = sync_desktop_errors_now(&state).await?;
                        let status = desktop_error_status(&state).await?;
                        Ok::<_, String>((upload, status))
                    },
                    |view, output, ctx| {
                        view.diagnostic_busy = false;
                        match output {
                            Ok((upload, status)) => {
                                view.diagnostic_status = Some(status);
                                view.diagnostic_message = format!(
                                    "上传完成: accepted={} duplicated={} rejected={}",
                                    upload.accepted, upload.duplicated, upload.rejected
                                );
                                view.diagnostic_tone = StatusTone::Success;
                            }
                            Err(err) => {
                                view.diagnostic_message = format!("上传诊断事件失败: {err}");
                                view.diagnostic_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::ClearDiagnosticQueue => {
                self.diagnostic_busy = true;
                self.diagnostic_message = "正在清空本地错误队列…".into();
                self.diagnostic_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        clear_desktop_error_events(&state).await
                    },
                    |view, output, ctx| {
                        view.diagnostic_busy = false;
                        match output {
                            Ok(status) => {
                                view.diagnostic_status = Some(status);
                                view.diagnostic_message = "已清空本地错误队列。".into();
                                view.diagnostic_tone = StatusTone::Success;
                            }
                            Err(err) => {
                                view.diagnostic_message = format!("清空本地错误队列失败: {err}");
                                view.diagnostic_tone = StatusTone::Danger;
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
                let core = self.core.clone();
                self.busy = true;
                self.status = "正在迁移…".into();
                self.status_tone = StatusTone::Placeholder;
                ctx.notify();
                ctx.spawn(
                    async move {
                        #[cfg(windows)]
                        let picked = tokio::task::spawn_blocking(|| {
                            wormhole_desktop_platform_windows::pick_folder("选择共享文件存放位置")
                        })
                        .await
                        .ok()
                        .flatten();
                        #[cfg(not(windows))]
                        let picked: Option<std::path::PathBuf> = None;

                        let Some(path) = picked else {
                            return Ok::<Option<SharedStorageInfoDto>, String>(None);
                        };

                        let target = path.display().to_string();
                        let state = core.runtime().state.clone();
                        let info = migrate_shared_storage(&state, target).await?;
                        Ok(Some(info))
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(Some(info)) => {
                                view.status = format!("已迁移共享文件至 {}", info.sync_entry_path);
                                view.storage = Some(info);
                                view.status_tone = StatusTone::Success;
                            }
                            Ok(None) => {
                                view.status = "已取消选择。".into();
                                view.status_tone = StatusTone::Placeholder;
                            }
                            Err(e) => {
                                view.status = format!("迁移失败: {e}");
                                view.status_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::ToggleArchiveSection => {
                self.archive_expanded = !self.archive_expanded;
                ctx.notify();
            }
            SettingsAction::RestoreArchivedSession(id) => {
                ctx.emit(SettingsEvent::RestoreArchivedSession(id.clone()));
            }
            SettingsAction::DeleteArchivedSession(id) => {
                ctx.emit(SettingsEvent::DeleteArchivedSession(id.clone()));
            }
        }
    }
}
