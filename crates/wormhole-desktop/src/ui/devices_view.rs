use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::hud_effects::ClusterTopology;
use crate::ui::panel_primitives::{section_hint, section_title, status_line, StatusTone, SECTION_PADDING, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::{cluster_status, ClusterNodeDto, ClusterStatusDto};
use wormhole_desktop_core::commands::{list_files, pin_file, FileDto};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Grid,
    Files,
}

#[derive(Debug, Clone)]
pub enum DevicesAction {
    Refresh,
    OpenNode(String),
    BackToGrid,
    SyncFile(String),
}

pub struct DevicesView {
    core: CoreHandle,
    font: FamilyId,
    mode: ViewMode,
    cluster: Option<ClusterStatusDto>,
    cluster_error: Option<String>,
    browsing_node_id: Option<String>,
    browsing_label: String,
    browsing_local: bool,
    files: Vec<FileDto>,
    files_error: Option<String>,
    syncing_file: Option<String>,
}

impl DevicesView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let view = Self {
            core,
            font,
            mode: ViewMode::Grid,
            cluster: None,
            cluster_error: None,
            browsing_node_id: None,
            browsing_label: String::new(),
            browsing_local: false,
            files: Vec::new(),
            files_error: None,
            syncing_file: None,
        };
        view.refresh_cluster(ctx);
        view
    }

    fn refresh_cluster(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status(&state).await
            },
            |view, output, ctx| {
                match output {
                    Ok(status) => {
                        view.cluster = Some(status);
                        view.cluster_error = None;
                    }
                    Err(e) => {
                        view.cluster = None;
                        view.cluster_error = Some(e);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn load_files(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                list_files(&state).await
            },
            |view, output, ctx| {
                match output {
                    Ok(files) => {
                        view.files = files;
                        view.files_error = None;
                    }
                    Err(e) => {
                        view.files = Vec::new();
                        view.files_error = Some(e);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_node(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        let local_id = self
            .cluster
            .as_ref()
            .map(|c| c.local_node_id.clone())
            .unwrap_or_default();
        let is_local = node_id == local_id;
        let label = self
            .cluster
            .as_ref()
            .and_then(|c| c.nodes.iter().find(|n| n.node_id == node_id))
            .map(|n| format!("{} · {}", n.os, n.hostname))
            .unwrap_or_else(|| node_id.clone());

        self.mode = ViewMode::Files;
        self.browsing_node_id = Some(node_id);
        self.browsing_label = label;
        self.browsing_local = is_local;
        self.files = Vec::new();
        self.files_error = None;

        if is_local {
            self.load_files(ctx);
        } else {
            self.files_error = Some("远程终端共享浏览即将支持；当前仅本机可浏览保险库文件。".into());
        }
        ctx.notify();
    }

    fn back_to_grid(&mut self, ctx: &mut ViewContext<Self>) {
        self.mode = ViewMode::Grid;
        self.browsing_node_id = None;
        self.browsing_label.clear();
        self.browsing_local = false;
        self.files.clear();
        self.files_error = None;
        ctx.notify();
    }

    fn sync_file(&mut self, file_id: String, ctx: &mut ViewContext<Self>) {
        self.syncing_file = Some(file_id.clone());
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                pin_file(&state, file_id).await
            },
            |view, output, ctx| {
                view.syncing_file = None;
                if output.is_ok() {
                    view.load_files(ctx);
                } else if let Err(e) = output {
                    view.files_error = Some(format!("同步失败: {e}"));
                }
                ctx.notify();
            },
        );
    }

    fn node_card(&self, node: &ClusterNodeDto, is_local: bool) -> Box<dyn Element> {
        let node_id = node.node_id.clone();
        let os_label = node.os.clone();
        let status_text = if node.online {
            "在线"
        } else {
            "离线"
        };
        let status_tone = if node.online {
            StatusTone::Success
        } else {
            StatusTone::Placeholder
        };

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            Container::new(
                ui_text::body(os_label, self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(theme::accent_bg(16))
            .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::mono(
                    truncate_node_id(&node.node_id),
                    self.font,
                )
                .with_color(theme::text())
                .finish(),
            )
            .with_uniform_padding(12.0)
            .finish(),
        );
        let mut status_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        status_row.add_child(status_line(status_text, self.font, status_tone));
        if is_local {
            status_row.add_child(
                Container::new(
                    ui_text::body("本机", self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_horizontal_margin(8.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(status_row.finish())
                .with_uniform_padding(12.0)
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::body("单击浏览共享", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_uniform_padding(8.0)
            .finish(),
        );

        let card = Container::new(col.finish())
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish();

        EventHandler::new(card)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::OpenNode(node_id.clone()));
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn grid_view(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("集群节点", self.font));

        if let Some(err) = &self.cluster_error {
            col.add_child(section_hint(
                "CLUSTER · OFFLINE · 无法读取集群",
                self.font,
            ));
            col.add_child(status_line(err.clone(), self.font, StatusTone::Danger));
        } else if let Some(cluster) = &self.cluster {
            let member_count = cluster.nodes.len();
            col.add_child(section_hint(
                format!(
                    "CLUSTER · {member_count} NODES · E2E ENCRYPTED · 单击终端浏览共享文件夹"
                ),
                self.font,
            ));
            let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
            let local_id = cluster.local_node_id.clone();
            let hub_index = cluster
                .nodes
                .iter()
                .position(|n| n.node_id == local_id)
                .unwrap_or(0);
            for node in &cluster.nodes {
                let card = Container::new(self.node_card(node, node.node_id == local_id))
                    .with_uniform_margin(6.0)
                    .finish();
                row.add_child(Shrinkable::new(1.0, card).finish());
            }
            let node_count = cluster.nodes.len().max(1);
            let mut stack = Stack::new();
            stack.add_child(
                ClusterTopology::new(node_count, hub_index).live(),
            );
            stack.add_child(row.finish());
            col.add_child(stack.finish());
        } else {
            col.add_child(section_hint("CLUSTER · LOADING", self.font));
            col.add_child(status_line("加载集群…", self.font, StatusTone::Placeholder));
        }

        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(SECTION_PADDING)
            .finish()
    }

    fn file_needs_sync(file: &FileDto) -> bool {
        let status = file.status.to_ascii_lowercase();
        !file.pinned && (status.contains("remote") || status.contains("pending"))
    }

    fn file_row(&self, file: &FileDto) -> Box<dyn Element> {
        let needs_sync = Self::file_needs_sync(file);
        let syncing = self.syncing_file.as_deref() == Some(file.id.as_str());
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);

        if needs_sync {
            let file_id = file.id.clone();
            let sync_label = if syncing { "同步中…" } else { "⟳" };
            row.add_child(
                Container::new(
                    EventHandler::new(
                        ui_text::body(sync_label, self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .on_left_mouse_down(move |ctx, _, _| {
                        if !syncing {
                            ctx.dispatch_typed_action(DevicesAction::SyncFile(file_id.clone()));
                        }
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_horizontal_margin(8.0)
                .finish(),
            );
        }

        row.add_child(
            Shrinkable::new(
                1.0,
                ui_text::body(file.name.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
        );
        row.add_child(
            ui_text::mono(format_size(file.size), self.font)
                .with_color(theme::muted())
                .finish(),
        );

        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn files_view(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let back = EventHandler::new(
            ui_text::body("← 终端", self.font)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::BackToGrid);
            DispatchEventResult::StopPropagation
        })
        .finish();
        col.add_child(back);
        col.add_child(
            ui_text::title(self.browsing_label.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if let Some(id) = &self.browsing_node_id {
            col.add_child(
                ui_text::mono(id.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        if let Some(err) = &self.files_error {
            col.add_child(status_line(err.clone(), self.font, StatusTone::Warn));
        }

        let mut list = Flex::column();
        for file in &self.files {
            list.add_child(self.file_row(file));
            list.add_child(
                Container::new(Flex::column().finish())
                    .with_vertical_margin(4.0)
                    .finish(),
            );
        }
        if self.browsing_local && self.files.is_empty() && self.files_error.is_none() {
            list.add_child(ui_text::body("（暂无共享文件）", self.font).finish());
        }

        col.add_child(list.finish());

        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}

fn truncate_node_id(id: &str) -> String {
    if id.len() <= 28 {
        return id.to_string();
    }
    let head = id.chars().take(12).collect::<String>();
    let tail = id.chars().rev().take(12).collect::<String>();
    format!("{}…{}", head, tail.chars().rev().collect::<String>())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

impl Entity for DevicesView {
    type Event = ();
}

impl View for DevicesView {
    fn ui_name() -> &'static str {
        "DevicesView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        match self.mode {
            ViewMode::Grid => self.grid_view(),
            ViewMode::Files => self.files_view(),
        }
    }
}

impl TypedActionView for DevicesView {
    type Action = DevicesAction;

    fn handle_action(&mut self, action: &DevicesAction, ctx: &mut ViewContext<Self>) {
        match action {
            DevicesAction::Refresh => self.refresh_cluster(ctx),
            DevicesAction::OpenNode(node_id) => self.open_node(node_id.clone(), ctx),
            DevicesAction::BackToGrid => self.back_to_grid(ctx),
            DevicesAction::SyncFile(file_id) => self.sync_file(file_id.clone(), ctx),
        }
    }
}
