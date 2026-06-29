use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext};

use crate::ui::agent_providers_view::AgentProvidersView;
use crate::ui::codex_provider_import_model::SharedCodexProviderImportModel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::sync_commands::{list_local_drives, set_sync_root, sync_status};

#[derive(Debug, Clone)]
pub enum SettingsAction {
    SavePath(String),
    Refresh,
}

pub struct SettingsView {
    core: CoreHandle,
    font: FamilyId,
    root_path: String,
    drive_options: Vec<String>,
    status: String,
    status_tone: StatusTone,
    busy: bool,
    agent_providers: warpui::ViewHandle<AgentProvidersView>,
}

impl SettingsView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        import_model: SharedCodexProviderImportModel,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let agent_providers = ctx.add_typed_action_view(|ctx| {
            AgentProvidersView::new(ctx, core.clone(), import_model)
        });
        let mut view = Self {
            core,
            font,
            root_path: String::new(),
            drive_options: Vec::new(),
            status: String::new(),
            status_tone: StatusTone::Placeholder,
            busy: false,
            agent_providers,
        };
        view.refresh(ctx);
        view
    }

    pub fn agent_providers_view(&self) -> &warpui::ViewHandle<AgentProvidersView> {
        &self.agent_providers
    }

    pub fn open_deeplink_url(&mut self, url: String, ctx: &mut ViewContext<Self>) {
        let agent = self.agent_providers.clone();
        ctx.update_view(&agent, |view, ctx| view.open_deeplink_url(url, ctx));
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let status = sync_status(&state).await;
                let drives = list_local_drives().await;
                (status, drives)
            },
            |view, output, ctx| {
                view.busy = false;
                let (status, drives) = output;
                match status {
                    Ok(s) => {
                        view.root_path = s.root_path;
                        if view.status.is_empty() {
                            view.status =
                                "终端共享文件夹的本地副本将写入此目录。修改后建议重启同步服务。"
                                    .into();
                            view.status_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(e) => {
                        view.status = format!("无法读取同步路径: {e}");
                        view.status_tone = StatusTone::Danger;
                    }
                }
                view.drive_options = drives
                    .unwrap_or_default()
                    .into_iter()
                    .map(|d| format!("{}:\\Wormhole", d.letter.trim_end_matches(':')))
                    .collect();
                ctx.notify();
            },
        );
    }

    fn action_button(&self, label: &str, path: String) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(SettingsAction::SavePath(path.clone()));
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

    fn shared_path_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("DATA · 共享文件存放位置", self.font));
        col.add_child(section_hint(
            "终端共享文件夹的本地副本将写入此目录。选择新位置后自动保存。",
            self.font,
        ));
        col.add_child(
            Container::new(
                ui_text::mono(self.root_path.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(theme::bg())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish(),
        );

        if !self.drive_options.is_empty() {
            col.add_child(
                ui_text::body("选择存放磁盘:", self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
            let mut picks = Flex::row();
            for path in &self.drive_options {
                let label = format!("使用 {path}");
                picks.add_child(
                    Container::new(self.action_button(&label, path.clone()))
                        .with_horizontal_margin(4.0)
                        .finish(),
                );
            }
            col.add_child(picks.finish());
        }

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
}

impl Entity for SettingsView {
    type Event = ();
}

impl View for SettingsView {
    fn ui_name() -> &'static str {
        "SettingsView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::title("设置", self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(self.shared_path_block());
        col.add_child(
            ui_text::body(crate::ui::fonts::UI_FONT_ATTRIBUTION, self.font)
                .with_color(theme::placeholder())
                .finish(),
        );

        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(16.0)
            .finish()
    }
}

impl TypedActionView for SettingsView {
    type Action = SettingsAction;

    fn handle_action(&mut self, action: &SettingsAction, ctx: &mut ViewContext<Self>) {
        match action {
            SettingsAction::Refresh => self.refresh(ctx),
            SettingsAction::SavePath(path) => {
                if path.trim().is_empty() {
                    return;
                }
                if path == &self.root_path {
                    self.refresh(ctx);
                    return;
                }
                self.busy = true;
                self.status = "正在保存…".into();
                self.status_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                let path_arg = Some(path.clone());
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        set_sync_root(&state, path_arg).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(saved) => {
                                view.root_path = saved;
                                view.status = "已保存共享文件夹路径。".into();
                                view.status_tone = StatusTone::Success;
                            }
                            Err(e) => {
                                view.status = format!("保存失败: {e}");
                                view.status_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
        }
    }
}
