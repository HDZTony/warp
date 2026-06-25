#[cfg(windows)]
mod imp {
    use warpui::elements::{
        Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
        Flex, ParentElement, Radius,
    };
    use warpui::fonts::FamilyId;
    use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
    use wormhole_desktop_core::sync_commands::{
        get_w_drive_config, list_local_drives, update_w_drive_storage, virtual_drive_status,
        w_drive_install_logon_task, w_drive_logon_task_status, LocalDriveDto, WDriveConfigDto,
    };

    use crate::ui::core_handle::CoreHandle;
    use crate::ui::panel_primitives::{
        section_card, section_hint, section_title, status_line, StatusTone,
    };
    use crate::ui::theme;
    use crate::ui_text;

    #[derive(Debug, Clone)]
    pub enum WDriveSettingsAction {
        Refresh,
        MigrateTo(String),
        InstallLogonTask,
    }

    pub struct WDriveSettingsView {
        core: CoreHandle,
        font: FamilyId,
        config: Option<WDriveConfigDto>,
        drives: Vec<LocalDriveDto>,
        drive_status: String,
        logon_line: String,
        status: String,
        busy: bool,
    }

    impl WDriveSettingsView {
        pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
            let font = crate::ui::fonts::load_ui_font(ctx);
            let mut view = Self {
                core,
                font,
                config: None,
                drives: Vec::new(),
                drive_status: "加载中…".into(),
                logon_line: String::new(),
                status: String::new(),
                busy: false,
            };
            view.refresh(ctx);
            view
        }

        fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
            let core = self.core.clone();
            ctx.spawn(
                async move {
                    let state = core.runtime().state.clone();
                    let config = get_w_drive_config(&state).await;
                    let drives = list_local_drives().await;
                    let vdrive = virtual_drive_status(&state).await;
                    let logon = w_drive_logon_task_status().await;
                    (config, drives, vdrive, logon)
                },
                |view, output, ctx| {
                    let (config, drives, vdrive, logon) = output;
                    view.config = config.ok();
                    view.drives = drives.unwrap_or_default();
                    view.drive_status = match vdrive {
                        Ok(s) => format!(
                            "W: {} | mode={} | root={}",
                            if s.mounted { "已挂载" } else { "未挂载" },
                            s.drive_mode,
                            s.sync_root_path.as_deref().unwrap_or("—")
                        ),
                        Err(e) => format!("W: 状态错误: {e}"),
                    };
                    view.logon_line = match logon {
                        Ok(s) => format!(
                            "登录任务 {}: {}",
                            s.task_name,
                            if s.installed {
                                "已注册"
                            } else {
                                "未注册"
                            }
                        ),
                        Err(e) => format!("登录任务: {e}"),
                    };
                    view.busy = false;
                    ctx.notify();
                },
            );
        }

        fn action_button(
            &self,
            label: &str,
            action: WDriveSettingsAction,
        ) -> Box<dyn Element> {
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
            .with_background(theme::accent_bg(28))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish()
        }

        fn format_bytes(bytes: u64) -> String {
            const GB: u64 = 1024 * 1024 * 1024;
            if bytes >= GB {
                format!("{:.1} GB", bytes as f64 / GB as f64)
            } else {
                format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
            }
        }
    }

    impl Entity for WDriveSettingsView {
        type Event = ();
    }

    impl View for WDriveSettingsView {
        fn ui_name() -> &'static str {
            "WDriveSettingsView"
        }

        fn render(&self, _app: &AppContext) -> Box<dyn Element> {
            let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            col.add_child(section_title("同步数据 / W 盘", self.font));
            col.add_child(section_hint(
                "程序安装位置与同步数据分离。默认数据在 %LOCALAPPDATA%\\Wormhole\\data；可迁移到其他磁盘。",
                self.font,
            ));
            if self.busy {
                col.add_child(status_line("操作进行中…", self.font, StatusTone::Warn));
            }
            col.add_child(ui_text::body(self.drive_status.clone(), self.font).finish());
            col.add_child(ui_text::body(self.logon_line.clone(), self.font).finish());

            if let Some(config) = &self.config {
                if let Some(path) = config.primary_backing_path.as_deref() {
                    col.add_child(ui_text::mono(format!("VHD 目录: {path}"), self.font).finish());
                }
                if let Some(vhd) = config.vhd_path.as_deref() {
                    col.add_child(ui_text::mono(format!("VHD 文件: {vhd}"), self.font).finish());
                }
            }

            if !self.status.is_empty() {
                col.add_child(status_line(self.status.clone(), self.font, StatusTone::Danger));
            }

            let mut toolbar = Flex::row();
            toolbar.add_child(self.action_button("刷新", WDriveSettingsAction::Refresh));
            if !self.busy {
                toolbar.add_child(
                    self.action_button("注册登录挂载任务", WDriveSettingsAction::InstallLogonTask),
                );
            }
            col.add_child(toolbar.finish());

            if !self.drives.is_empty() {
                col.add_child(ui_text::body("迁移到其他磁盘:", self.font).finish());
                for drive in &self.drives {
                    let label = format!(
                        "迁移到 {}: ({}) 可用 {}",
                        drive.letter,
                        drive.label,
                        Self::format_bytes(drive.free_bytes)
                    );
                    col.add_child(self.action_button(
                        &label,
                        WDriveSettingsAction::MigrateTo(drive.letter.clone()),
                    ));
                }
            }

            section_card(col.finish())
        }
    }

    impl TypedActionView for WDriveSettingsView {
        type Action = WDriveSettingsAction;

        fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
            match action {
                WDriveSettingsAction::Refresh => self.refresh(ctx),
                WDriveSettingsAction::MigrateTo(letter) => {
                    if self.busy {
                        return;
                    }
                    self.busy = true;
                    self.status = format!("正在迁移到 {letter}: …");
                    ctx.notify();
                    let core = self.core.clone();
                    let letter = letter.clone();
                    ctx.spawn(
                        async move {
                            let state = core.runtime().state.clone();
                            update_w_drive_storage(&state, vec![letter]).await
                        },
                        |view, output, ctx| {
                            view.busy = false;
                            view.status = match output {
                                Ok(_) => "迁移完成".into(),
                                Err(e) => e,
                            };
                            view.refresh(ctx);
                        },
                    );
                }
                WDriveSettingsAction::InstallLogonTask => {
                    if self.busy {
                        return;
                    }
                    self.busy = true;
                    ctx.notify();
                    ctx.spawn(
                        async move { w_drive_install_logon_task().await },
                        |view, output, ctx| {
                            view.busy = false;
                            view.status = match output {
                                Ok(s) => s.message,
                                Err(e) => e,
                            };
                            view.refresh(ctx);
                        },
                    );
                }
            }
        }
    }
}

#[cfg(windows)]
pub use imp::WDriveSettingsView;

#[cfg(not(windows))]
mod stub {
    use warpui::elements::{Flex, ParentElement};
    use warpui::fonts::FamilyId;
    use warpui::{AppContext, Element, Entity, View, ViewContext};

    use crate::ui::core_handle::CoreHandle;
    use crate::ui::panel_primitives::{section_card, section_hint, section_title};

    pub struct WDriveSettingsView {
        font: FamilyId,
    }

    impl WDriveSettingsView {
        pub fn new(ctx: &mut ViewContext<Self>, _core: CoreHandle) -> Self {
            Self {
                font: crate::ui::fonts::load_ui_font(ctx),
            }
        }
    }

    impl Entity for WDriveSettingsView {
        type Event = ();
    }

    impl View for WDriveSettingsView {
        fn ui_name() -> &'static str {
            "WDriveSettingsView"
        }

        fn render(&self, _app: &AppContext) -> Box<dyn Element> {
            let col = Flex::column()
                .with_child(section_title("同步数据 / W 盘", self.font))
                .with_child(section_hint("W 盘迁移仅适用于 Windows。", self.font));
            section_card(col.finish())
        }
    }
}

#[cfg(not(windows))]
pub use stub::WDriveSettingsView;
