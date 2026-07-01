use pathfinder_color::ColorU;
use warpui::elements::Fill;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::clipboard::{read_clipboard_text, write_clipboard_text};
use crate::ui::cluster_topology_panel::ClusterTopologyPanel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::fetch_cluster_for_ui;
use crate::ui::devices_actions::DevicesAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    section_hint, section_title, status_line, tab_content_fill, truncate_middle, StatusTone,
    HUD_RADIUS, SECTION_PADDING,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::{
    add_storage_volume, create_cluster_invite, delete_share_entry, join_cluster,
    list_share_directory, open_share_entry, remote_open_share_entry, switch_active_cluster,
    sync_share_entry, AddStorageVolumeParams, ClusterStatusDto, JoinClusterOutcome,
    JoinClusterParams, JoinedClusterDto, ListShareDirectoryParams, ShareEntryDto,
    SwitchActiveClusterParams,
};

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Grid,
    Files,
}

pub struct DevicesView {
    core: CoreHandle,
    font: FamilyId,
    mono: FamilyId,
    mode: ViewMode,
    cluster: Option<ClusterStatusDto>,
    cluster_syncing: bool,
    cluster_error: Option<String>,
    browsing_node_id: Option<String>,
    browsing_label: String,
    share_entries: Vec<ShareEntryDto>,
    share_error: Option<String>,
    share_path: Vec<String>,
    share_history_back: Vec<Vec<String>>,
    share_history_forward: Vec<Vec<String>>,
    share_status: Option<String>,
    share_add_modal_open: bool,
    share_add_path: String,
    share_add_feedback: Option<(StatusTone, String)>,
    share_add_busy: bool,
    join_modal_open: bool,
    join_invite_draft: String,
    join_feedback: Option<(StatusTone, String)>,
    local_invite: Option<String>,
    invite_busy: bool,
    status_flash: Option<String>,
    cluster_picker_open: bool,
    copy_invite_ack: bool,
    copy_invite_busy: bool,
    share_scroll: ClippedScrollStateHandle,
    share_context_entry: Option<String>,
    share_context_pos: Option<(f32, f32)>,
    share_file_busy: bool,
    last_file_click: Option<(String, std::time::Instant)>,
    bootstrap_busy: bool,
    bootstrap_pending_since: Option<Instant>,
}

const TOOLBAR_BTN_HEIGHT: f32 = 32.0;
const TOOLBAR_BTN_PAD_X: f32 = 18.0;
const CLUSTER_SELECT_MIN_WIDTH: f32 = 240.0;
const BOOTSTRAP_PENDING_HINT_AFTER: Duration = Duration::from_secs(35);

impl DevicesView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let view = Self {
            core,
            font,
            mono,
            mode: ViewMode::Grid,
            cluster: None,
            cluster_syncing: false,
            cluster_error: None,
            browsing_node_id: None,
            browsing_label: String::new(),
            share_entries: Vec::new(),
            share_error: None,
            share_path: Vec::new(),
            share_history_back: Vec::new(),
            share_history_forward: Vec::new(),
            share_status: None,
            share_add_modal_open: false,
            share_add_path: String::new(),
            share_add_feedback: None,
            share_add_busy: false,
            join_modal_open: false,
            join_invite_draft: String::new(),
            join_feedback: None,
            local_invite: None,
            invite_busy: false,
            status_flash: None,
            cluster_picker_open: false,
            copy_invite_ack: false,
            copy_invite_busy: false,
            share_scroll: ClippedScrollStateHandle::new(),
            share_context_entry: None,
            share_context_pos: None,
            share_file_busy: false,
            last_file_click: None,
            bootstrap_busy: false,
            bootstrap_pending_since: None,
        };
        view.refresh_cluster(ctx);
        view
    }

    fn note_bootstrap_pending(&mut self, status: &ClusterStatusDto) {
        if status.device_bootstrap_required && status.device_bootstrap_error.is_none() {
            if self.bootstrap_pending_since.is_none() {
                self.bootstrap_pending_since = Some(Instant::now());
            }
        } else {
            self.bootstrap_pending_since = None;
        }
    }

    fn bootstrap_pending_slow(&self, cluster: &ClusterStatusDto) -> bool {
        cluster.device_bootstrap_required
            && cluster.device_bootstrap_error.is_none()
            && self
                .bootstrap_pending_since
                .is_some_and(|started| started.elapsed() >= BOOTSTRAP_PENDING_HINT_AFTER)
    }

    fn apply_cluster_status(&mut self, status: ClusterStatusDto, ctx: &mut ViewContext<Self>) {
        self.note_bootstrap_pending(&status);
        self.cluster_syncing = status.syncing;
        self.cluster = Some(status.clone());
        self.cluster_error = None;
        if self.local_invite.is_none() && !self.invite_busy {
            self.load_local_invite(ctx);
        }
        if status.syncing {
            self.schedule_cluster_poll(ctx);
        } else if status.auth_required || status.device_bootstrap_required {
            self.schedule_bootstrap_poll(ctx);
        }
    }

    pub fn refresh_cluster(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                fetch_cluster_for_ui(&state).await
            },
            |view, output, ctx| {
                match output {
                    Ok(status) => view.apply_cluster_status(status, ctx),
                    Err(e) => {
                        view.cluster = None;
                        view.cluster_syncing = false;
                        view.cluster_error = Some(e);
                        view.bootstrap_pending_since = None;
                    }
                }
                ctx.notify();
            },
        );
    }

    pub fn retry_device_bootstrap(&mut self, ctx: &mut ViewContext<Self>) {
        if self.bootstrap_busy {
            return;
        }
        self.bootstrap_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let _bootstrap =
                    wormhole_desktop_core::device_identity::device_bootstrap(&state).await;
                fetch_cluster_for_ui(&state).await
            },
            |view, output, ctx| {
                view.bootstrap_busy = false;
                match output {
                    Ok(status) => view.apply_cluster_status(status, ctx),
                    Err(e) => {
                        view.cluster = None;
                        view.cluster_syncing = false;
                        view.cluster_error = Some(e);
                        view.bootstrap_pending_since = None;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn schedule_cluster_poll(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let state = core.runtime().state.clone();
                fetch_cluster_for_ui(&state).await
            },
            |view, output, ctx| {
                if let Ok(status) = output {
                    view.apply_cluster_status(status, ctx);
                }
                ctx.notify();
            },
        );
    }

    fn schedule_bootstrap_poll(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let state = core.runtime().state.clone();
                fetch_cluster_for_ui(&state).await
            },
            |view, output, ctx| {
                if let Ok(status) = output {
                    view.note_bootstrap_pending(&status);
                    view.cluster_syncing = status.syncing;
                    view.cluster = Some(status.clone());
                    view.cluster_error = None;
                    if status.auth_required || status.device_bootstrap_required {
                        view.schedule_bootstrap_poll(ctx);
                    } else if status.syncing {
                        view.schedule_cluster_poll(ctx);
                    } else if view.local_invite.is_none() && !view.invite_busy {
                        view.load_local_invite(ctx);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn share_path_string(&self) -> String {
        if self.share_path.is_empty() {
            "/".into()
        } else {
            format!("/{}", self.share_path.join("/"))
        }
    }

    fn load_share_directory(&self, ctx: &mut ViewContext<Self>) {
        let Some(node_id) = self.browsing_node_id.clone() else {
            return;
        };
        let path = self.share_path_string();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                list_share_directory(&state, ListShareDirectoryParams { node_id, path }).await
            },
            |view, output, ctx| {
                match output {
                    Ok(entries) => {
                        view.share_entries = entries;
                        view.share_error = None;
                    }
                    Err(e) => {
                        view.share_entries.clear();
                        view.share_error = Some(e);
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
        let label = self
            .cluster
            .as_ref()
            .and_then(|c| c.nodes.iter().find(|n| n.node_id == node_id))
            .map(|n| {
                if node_id == local_id {
                    format!("{} · 本机", n.os)
                } else {
                    format!("{} · {}", n.os, n.hostname)
                }
            })
            .unwrap_or_else(|| node_id.clone());

        self.mode = ViewMode::Files;
        self.join_modal_open = false;
        self.join_invite_draft.clear();
        self.join_feedback = None;
        self.cluster_picker_open = false;
        self.browsing_node_id = Some(node_id);
        self.browsing_label = label;
        self.share_entries.clear();
        self.share_error = None;
        self.share_path.clear();
        self.share_history_back.clear();
        self.share_history_forward.clear();
        self.share_status = None;
        self.share_add_modal_open = false;
        self.reset_share_scroll();
        self.load_share_directory(ctx);
        ctx.notify();
    }

    fn back_to_grid(&mut self, ctx: &mut ViewContext<Self>) {
        self.mode = ViewMode::Grid;
        self.browsing_node_id = None;
        self.browsing_label.clear();
        self.share_entries.clear();
        self.share_error = None;
        self.share_path.clear();
        self.share_history_back.clear();
        self.share_history_forward.clear();
        self.share_status = None;
        self.share_add_modal_open = false;
        ctx.notify();
    }

    fn navigate_share_to(
        &mut self,
        volume_id: Option<String>,
        name: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let mut next = self.share_path.clone();
        if let Some(id) = volume_id {
            next = vec![id];
        } else if next.is_empty() {
            return;
        } else {
            next.push(name);
        }
        if next == self.share_path {
            return;
        }
        self.share_history_back.push(self.share_path.clone());
        self.share_history_forward.clear();
        self.share_path = next;
        self.share_status = None;
        self.reset_share_scroll();
        self.load_share_directory(ctx);
        ctx.notify();
    }

    fn share_back(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(prev) = self.share_history_back.pop() else {
            return;
        };
        self.share_history_forward.push(self.share_path.clone());
        self.share_path = prev;
        self.share_status = None;
        self.reset_share_scroll();
        self.load_share_directory(ctx);
        ctx.notify();
    }

    fn share_forward(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(next) = self.share_history_forward.pop() else {
            return;
        };
        self.share_history_back.push(self.share_path.clone());
        self.share_path = next;
        self.share_status = None;
        self.reset_share_scroll();
        self.load_share_directory(ctx);
        ctx.notify();
    }

    fn browsing_local(&self) -> bool {
        self.cluster.as_ref().is_some_and(|cluster| {
            self.browsing_node_id.as_deref() == Some(cluster.local_node_id.as_str())
        })
    }

    fn open_share_add_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.browsing_local() || !self.share_path.is_empty() {
            return;
        }
        self.share_add_feedback = None;
        if self.share_add_path.is_empty() {
            self.share_add_path = default_share_browse_path();
        }
        self.share_add_modal_open = true;
        ctx.notify();
    }

    fn close_share_add_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.share_add_modal_open = false;
        self.share_add_feedback = None;
        ctx.notify();
    }

    fn browse_share_add_path(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                #[cfg(windows)]
                {
                    tokio::task::spawn_blocking(|| {
                        wormhole_desktop_platform_windows::pick_folder("选择共享文件夹")
                    })
                    .await
                    .ok()
                    .flatten()
                }
                #[cfg(not(windows))]
                {
                    let _ = ();
                    None::<std::path::PathBuf>
                }
            },
            |view, picked, ctx| {
                if let Some(path) = picked {
                    view.share_add_path = path.display().to_string();
                    view.share_add_feedback = None;
                }
                ctx.notify();
            },
        );
    }

    fn submit_share_add(&mut self, ctx: &mut ViewContext<Self>) {
        let path = self.share_add_path.trim().to_string();
        if path.is_empty() {
            self.share_add_feedback = Some((StatusTone::Warn, "请选择本机路径".into()));
            ctx.notify();
            return;
        }
        self.share_add_busy = true;
        self.share_add_feedback = Some((StatusTone::Neutral, "正在添加共享文件夹…".into()));
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                add_storage_volume(&state, AddStorageVolumeParams { path }).await
            },
            |view, output, ctx| {
                view.share_add_busy = false;
                match output {
                    Ok(_) => {
                        view.share_add_modal_open = false;
                        view.share_add_feedback = None;
                        view.share_status = Some("已添加共享文件夹".into());
                        view.load_share_directory(ctx);
                        view.refresh_cluster(ctx);
                    }
                    Err(e) => {
                        view.share_add_feedback = Some((StatusTone::Danger, e));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn close_share_context_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.share_context_entry = None;
        self.share_context_pos = None;
        ctx.notify();
    }

    fn share_file_context(&self) -> (String, String, String) {
        let node_id = self.browsing_node_id.clone().unwrap_or_default();
        let path = self.share_path_string();
        (node_id, path, String::new())
    }

    fn run_share_file_action<F>(
        &mut self,
        entry_name: String,
        busy_label: &str,
        success_label: &str,
        op: F,
        ctx: &mut ViewContext<Self>,
    ) where
        F: FnOnce(
                wormhole_desktop_core::state::AppState,
                String,
                String,
                String,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), String>> + Send>,
            > + Send
            + 'static,
    {
        if self.share_file_busy {
            return;
        }
        let (node_id, share_path, _) = self.share_file_context();
        let name = entry_name.clone();
        self.share_file_busy = true;
        self.share_status = Some(busy_label.to_string());
        self.close_share_context_menu(ctx);
        ctx.notify();
        let core = self.core.clone();
        let success = success_label.to_string();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                op(state, node_id, share_path, name).await
            },
            move |view, output, ctx| {
                view.share_file_busy = false;
                match output {
                    Ok(()) => {
                        view.share_status = Some(format!("{success} · {entry_name}"));
                        view.load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_status = Some(format!("失败 · {e}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            "正在打开…",
            "已打开",
            |state, node_id, share_path, name| {
                Box::pin(
                    async move { open_share_entry(&state, &node_id, &share_path, &name).await },
                )
            },
            ctx,
        );
    }

    fn sync_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            "正在同步…",
            "已同步",
            |state, node_id, share_path, name| {
                Box::pin(
                    async move { sync_share_entry(&state, &node_id, &share_path, &name).await },
                )
            },
            ctx,
        );
    }

    fn remote_open_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            "正在远程打开…",
            "已请求远程打开",
            |state, node_id, share_path, name| {
                Box::pin(async move {
                    remote_open_share_entry(&state, &node_id, &share_path, &name).await
                })
            },
            ctx,
        );
    }

    fn delete_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            "正在删除…",
            "已删除",
            |state, node_id, share_path, name| {
                Box::pin(
                    async move { delete_share_entry(&state, &node_id, &share_path, &name).await },
                )
            },
            ctx,
        );
    }

    fn handle_share_file_click(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        let now = std::time::Instant::now();
        if let Some((last_name, last_at)) = &self.last_file_click {
            if last_name == &entry_name && last_at.elapsed() < Duration::from_millis(450) {
                self.last_file_click = None;
                self.open_share_file(entry_name, ctx);
                return;
            }
        }
        self.last_file_click = Some((entry_name, now));
        ctx.notify();
    }

    fn joined_cluster_label(entry: &JoinedClusterDto) -> String {
        format!(
            "{} · {}",
            entry.folder_name,
            short_cluster_id(&entry.cluster_id)
        )
    }

    fn cluster_label(cluster: &ClusterStatusDto) -> String {
        cluster
            .clusters
            .iter()
            .find(|c| c.active)
            .map(Self::joined_cluster_label)
            .unwrap_or_else(|| {
                cluster
                    .cluster_id
                    .as_deref()
                    .map(short_cluster_id)
                    .unwrap_or_else(|| "集群".to_string())
            })
    }

    fn cluster_status_text(&self, cluster: &ClusterStatusDto) -> String {
        if self.cluster_syncing {
            return "CLUSTER · SYNCING · 后台同步集群…".to_string();
        }
        if let Some(flash) = &self.status_flash {
            return flash.clone();
        }
        let n = cluster.nodes.len();
        format!(
            "CLUSTER · {n} NODE{} · E2E ENCRYPTED · 点击终端「共享文件」浏览共享文件夹",
            if n == 1 { "" } else { "S" }
        )
    }

    fn toolbar_button(
        &self,
        label: &str,
        action: DevicesAction,
        warm: bool,
        min_width: f32,
        enabled: bool,
    ) -> Box<dyn Element> {
        let (border, bg, color) = if !enabled {
            (
                dim_color(theme::border_bright(), 0.35),
                theme::panel(),
                dim_color(theme::text(), 0.35),
            )
        } else if warm {
            (
                theme::accent_cool(),
                theme::accent_cool_bg(40),
                theme::accent_cool(),
            )
        } else {
            (theme::border_bright(), theme::panel(), theme::text())
        };
        let inner = Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_child(
                        ui_text::cluster_ctrl(label.to_string(), self.mono)
                            .with_color(color)
                            .finish(),
                    )
                    .finish(),
            )
            .with_height(TOOLBAR_BTN_HEIGHT)
            .with_min_width(min_width)
            .finish(),
        )
        .with_horizontal_padding(TOOLBAR_BTN_PAD_X)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();
        if enabled {
            EventHandler::new(inner)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            inner
        }
    }

    fn reset_share_scroll(&self) {
        use warpui_core::units::Pixels;
        self.share_scroll.scroll_to(Pixels::zero());
    }

    fn share_nav_button(
        &self,
        action: DevicesAction,
        enabled: bool,
        back: bool,
    ) -> Box<dyn Element> {
        let icon_color = if enabled {
            theme::muted()
        } else {
            dim_color(theme::muted(), 0.3)
        };
        let border = if enabled {
            theme::border_bright()
        } else {
            dim_color(theme::border_bright(), 0.3)
        };
        let inner = Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(icons::share_nav_icon(back, icon_color))
                    .finish(),
            )
            .with_width(icons::SHARE_NAV_BTN_SIZE)
            .with_height(icons::SHARE_NAV_BTN_SIZE)
            .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();

        if enabled {
            EventHandler::new(inner)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            inner
        }
    }

    fn cluster_select(&self, cluster: &ClusterStatusDto) -> Box<dyn Element> {
        let label = Self::cluster_label(cluster);
        let mut trigger_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        trigger_row.add_child(
            Shrinkable::new(
                1.0,
                ui_text::cluster_label(label, self.mono)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
        );
        trigger_row.add_child(
            Container::new(
                ui_text::cluster_ctrl("▾", self.mono)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_horizontal_margin(8.0)
            .finish(),
        );
        let trigger = EventHandler::new(
            Container::new(
                ConstrainedBox::new(trigger_row.finish())
                    .with_height(TOOLBAR_BTN_HEIGHT)
                    .with_width(CLUSTER_SELECT_MIN_WIDTH)
                    .finish(),
            )
            .with_horizontal_padding(12.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::ToggleClusterPicker);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(trigger);
        if self.cluster_picker_open && cluster.clusters.len() > 1 {
            let mut menu = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            for entry in &cluster.clusters {
                let cluster_id = entry.cluster_id.clone();
                let item_label = Self::joined_cluster_label(entry);
                let (bg, color) = if entry.active {
                    (theme::accent_cool_bg(40), theme::accent_cool())
                } else {
                    (theme::canvas(), theme::text())
                };
                menu.add_child(
                    EventHandler::new(
                        Container::new(
                            ui_text::cluster_label(item_label, self.mono)
                                .with_color(color)
                                .finish(),
                        )
                        .with_horizontal_padding(12.0)
                        .with_vertical_padding(10.0)
                        .with_background(bg)
                        .finish(),
                    )
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(DevicesAction::SelectCluster(cluster_id.clone()));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                );
            }
            col.add_child(
                Container::new(menu.finish())
                    .with_vertical_margin(4.0)
                    .with_background(theme::canvas())
                    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                    .finish(),
            );
        }

        ConstrainedBox::new(col.finish())
            .with_width(CLUSTER_SELECT_MIN_WIDTH)
            .finish()
    }

    fn cluster_toolbar(&self, cluster: &ClusterStatusDto) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        row.add_child(self.cluster_select(cluster));
        row.add_child(
            Container::new(self.toolbar_button(
                self.copy_invite_label(),
                DevicesAction::CopyInvite,
                self.copy_invite_ack,
                136.0,
                !self.copy_invite_busy,
            ))
            .with_horizontal_margin(10.0)
            .finish(),
        );
        row.add_child(self.toolbar_button(
            "+ 加入集群",
            DevicesAction::OpenJoinModal,
            true,
            128.0,
            true,
        ));
        row.add_child(
            Shrinkable::new(
                1.0,
                Container::new(
                    ConstrainedBox::new(
                        ui_text::cluster_status(self.cluster_status_text(cluster), self.mono)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_height(TOOLBAR_BTN_HEIGHT)
                    .finish(),
                )
                .with_horizontal_margin(10.0)
                .finish(),
            )
            .finish(),
        );
        row.finish()
    }

    fn load_local_invite(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                create_cluster_invite(&state).await
            },
            |view, output, ctx| {
                view.invite_busy = false;
                match output {
                    Ok(invite) => view.local_invite = Some(invite),
                    Err(e) => {
                        view.join_feedback = Some((StatusTone::Danger, e));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn copy_invite_label(&self) -> &'static str {
        if self.copy_invite_ack {
            "已复制"
        } else if self.copy_invite_busy {
            "复制中…"
        } else {
            "复制邀请码"
        }
    }

    fn apply_copy_invite_success(&mut self, invite: String, ctx: &mut ViewContext<Self>) {
        self.local_invite = Some(invite);
        let name = self
            .cluster
            .as_ref()
            .map(DevicesView::cluster_label)
            .unwrap_or_else(|| "集群".to_string());
        self.status_flash = Some(format!("INVITE COPIED · {name} · 已复制本机邀请码"));
        self.copy_invite_ack = true;
        self.copy_invite_busy = false;
        ctx.notify();
        ctx.spawn(
            async move {
                tokio::time::sleep(Duration::from_millis(2600)).await;
            },
            |view, _, ctx| {
                view.copy_invite_ack = false;
                view.status_flash = None;
                ctx.notify();
            },
        );
    }

    fn copy_invite(&mut self, ctx: &mut ViewContext<Self>) {
        if self.copy_invite_busy {
            return;
        }
        self.copy_invite_ack = false;
        self.copy_invite_busy = true;
        self.status_flash = Some("正在生成邀请码…".into());
        ctx.notify();

        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                create_cluster_invite(&state).await
            },
            |view, output, ctx| {
                view.copy_invite_busy = false;
                match output {
                    Ok(invite) => match write_clipboard_text(&invite) {
                        Ok(()) => view.apply_copy_invite_success(invite, ctx),
                        Err(e) => {
                            view.status_flash = Some(format!("复制失败：{e}"));
                            view.join_feedback = Some((StatusTone::Danger, e));
                            ctx.notify();
                        }
                    },
                    Err(e) => {
                        view.status_flash = Some(format!("生成邀请码失败：{e}"));
                        view.join_feedback = Some((StatusTone::Danger, e));
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn open_join_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.cluster_picker_open = false;
        self.join_modal_open = true;
        self.join_feedback = None;
        if self.join_invite_draft.is_empty() {
            if let Some(text) = read_clipboard_text() {
                self.join_invite_draft = text;
            }
        }
        if self.local_invite.is_none() {
            self.invite_busy = true;
            self.load_local_invite(ctx);
        }
        ctx.notify();
    }

    fn close_join_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.join_modal_open = false;
        self.join_invite_draft.clear();
        self.join_feedback = None;
        ctx.notify();
    }

    fn toggle_cluster_picker(&mut self, ctx: &mut ViewContext<Self>) {
        if self.cluster.as_ref().is_some_and(|c| c.clusters.len() > 1) {
            self.cluster_picker_open = !self.cluster_picker_open;
            ctx.notify();
        }
    }

    fn close_cluster_picker(&mut self, ctx: &mut ViewContext<Self>) {
        if self.cluster_picker_open {
            self.cluster_picker_open = false;
            ctx.notify();
        }
    }

    fn select_cluster(&mut self, cluster_id: String, ctx: &mut ViewContext<Self>) {
        self.cluster_picker_open = false;
        let already_active = self.cluster.as_ref().and_then(|c| c.cluster_id.as_deref())
            == Some(cluster_id.as_str());
        if already_active {
            ctx.notify();
            return;
        }
        self.status_flash = None;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                switch_active_cluster(
                    &state,
                    SwitchActiveClusterParams {
                        cluster_id: cluster_id.clone(),
                    },
                )
                .await
            },
            |view, output, ctx| {
                match output {
                    Ok(status) => {
                        view.cluster = Some(status);
                        view.cluster_error = None;
                        view.local_invite = None;
                    }
                    Err(e) => {
                        view.cluster_error = Some(e);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn paste_join_invite(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(text) = read_clipboard_text() {
            self.join_invite_draft = text.trim().to_string();
            self.join_feedback = None;
            ctx.notify();
        }
    }

    fn submit_join(&mut self, ctx: &mut ViewContext<Self>) {
        let invite = self.join_invite_draft.trim().to_string();
        if invite.is_empty() {
            self.join_feedback = Some((StatusTone::Warn, "请粘贴邀请码".into()));
            ctx.notify();
            return;
        }
        self.invite_busy = true;
        self.join_feedback = Some((StatusTone::Neutral, "正在加入集群…".into()));
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                join_cluster(&state, JoinClusterParams { invite }).await
            },
            |view, output, ctx| {
                view.invite_busy = false;
                match output {
                    Ok(result) => {
                        view.cluster = Some(result.status);
                        view.cluster_error = None;
                        view.join_modal_open = false;
                        view.join_invite_draft.clear();
                        view.join_feedback = None;
                        view.local_invite = None;
                        view.status_flash = Some(match result.outcome {
                            JoinClusterOutcome::Joined => format!(
                                "已加入集群 · {}",
                                DevicesView::cluster_label(&view.cluster.as_ref().unwrap())
                            ),
                            JoinClusterOutcome::AlreadyActive => "您已在该集群中".to_string(),
                            JoinClusterOutcome::SwitchedActive => format!(
                                "已切换到集群 · {}",
                                DevicesView::cluster_label(&view.cluster.as_ref().unwrap())
                            ),
                        });
                        ctx.spawn(
                            async move {
                                tokio::time::sleep(Duration::from_millis(2600)).await;
                            },
                            |view, _, ctx| {
                                view.status_flash = None;
                                ctx.notify();
                            },
                        );
                    }
                    Err(e) => {
                        view.join_feedback = Some((StatusTone::Danger, e));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn join_modal(&self) -> Box<dyn Element> {
        let join_preview = if self.join_invite_draft.is_empty() {
            "（从剪贴板粘贴邀请 JSON）".to_string()
        } else {
            truncate_middle(&self.join_invite_draft, 240)
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("集群", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body("粘贴对方邀请码加入新集群。", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            ui_text::hud_title("加入集群 · 粘贴邀请码", self.font)
                .with_color(theme::muted())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::mono(join_preview, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        );
        dialog.add_child(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(
                        Expanded::new(
                            1.0,
                            self.toolbar_button(
                                "从剪贴板粘贴",
                                DevicesAction::PasteJoinInvite,
                                false,
                                0.0,
                                true,
                            ),
                        )
                        .finish(),
                    )
                    .finish(),
            )
            .with_vertical_margin(10.0)
            .finish(),
        );

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.toolbar_button(
            "取消",
            DevicesAction::CloseJoinModal,
            false,
            72.0,
            true,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        actions.add_child(self.toolbar_button(
            if self.invite_busy {
                "加入中…"
            } else {
                "加入"
            },
            DevicesAction::SubmitJoin,
            true,
            80.0,
            !self.invite_busy,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(12.0)
                .finish(),
        );
        if let Some((tone, msg)) = &self.join_feedback {
            dialog.add_child(status_line(msg.clone(), self.font, *tone));
        }

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(440.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        )
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseJoinModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn grid_view(&self) -> Box<dyn Element> {
        let mut header = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        header.add_child(section_title("集群节点", self.mono));

        if let Some(err) = &self.cluster_error {
            header.add_child(section_hint("CLUSTER · OFFLINE · 无法读取集群", self.font));
            header.add_child(status_line(err.clone(), self.font, StatusTone::Danger));
        } else if let Some(cluster) = &self.cluster {
            if cluster.auth_required {
                header.add_child(section_hint("ACCOUNT · 需要登录", self.font));
                header.add_child(status_line(
                    "请登录 Wormhole 账号以使用集群与 P2P 功能。",
                    self.font,
                    StatusTone::Placeholder,
                ));
            } else if cluster.device_bootstrap_required {
                header.add_child(section_hint("DEVICE · 正在恢复设备身份", self.font));
                if let Some(err) = &cluster.device_bootstrap_error {
                    header.add_child(status_line(err.clone(), self.font, StatusTone::Danger));
                } else {
                    header.add_child(status_line(
                        "登录成功，正在从云端恢复本机设备身份…",
                        self.font,
                        StatusTone::Placeholder,
                    ));
                    if self.bootstrap_pending_slow(cluster) {
                        header.add_child(status_line(
                            "仍在等待控制面响应。请点击「重试」；若持续失败请查看终端日志。",
                            self.font,
                            StatusTone::Placeholder,
                        ));
                    }
                }
                header.add_child(
                    Container::new(self.toolbar_button(
                        if self.bootstrap_busy {
                            "重试中…"
                        } else {
                            "重试"
                        },
                        DevicesAction::RetryDeviceBootstrap,
                        false,
                        88.0,
                        !self.bootstrap_busy,
                    ))
                    .with_vertical_margin(8.0)
                    .finish(),
                );
            } else {
                header.add_child(
                    Container::new(self.cluster_toolbar(cluster))
                        .with_vertical_margin(10.0)
                        .finish(),
                );
                if self.cluster_syncing {
                    header.add_child(section_hint("CLUSTER · SYNCING", self.font));
                }
            }
        } else {
            header.add_child(section_hint("CLUSTER · LOADING", self.font));
            header.add_child(status_line("加载集群…", self.font, StatusTone::Placeholder));
        }

        let header_block = Container::new(header.finish())
            .with_uniform_padding(SECTION_PADDING)
            .with_padding_bottom(12.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish();

        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        col.add_child(header_block);

        if let Some(cluster) = &self.cluster {
            if cluster.auth_required || cluster.device_bootstrap_required {
                col.add_child(
                    Expanded::new(
                        1.0,
                        Container::new(
                            ui_text::body(
                                if cluster.auth_required {
                                    "登录后可查看集群拓扑并加入家庭网络。"
                                } else if cluster.device_bootstrap_error.is_some() {
                                    "设备身份恢复失败。请检查网络与控制面配置后点击「重试」，或在设置中退出并重新登录。"
                                } else if self.bootstrap_pending_slow(cluster) {
                                    "控制面响应较慢。点击「重试」将重新发起设备身份恢复；也可在终端查看 bootstrap 日志。"
                                } else {
                                    "设备身份恢复完成后将自动同步集群。"
                                },
                                self.font,
                            )
                            .with_color(theme::muted())
                            .finish(),
                        )
                        .with_uniform_padding(SECTION_PADDING)
                        .finish(),
                    )
                    .finish(),
                );
            } else {
                let local_id = cluster.local_node_id.clone();
                let mut nodes = cluster.nodes.clone();
                nodes.sort_by(|a, b| {
                    let a_local = a.node_id == local_id;
                    let b_local = b.node_id == local_id;
                    b_local
                        .cmp(&a_local)
                        .then_with(|| a.hostname.cmp(&b.hostname))
                });
                let hub_index = nodes
                    .iter()
                    .position(|n| n.node_id == local_id)
                    .unwrap_or(0);
                col.add_child(
                    Expanded::new(
                        1.0,
                        Container::new(
                            ConstrainedBox::new(ClusterTopologyPanel::element(
                                nodes, local_id, hub_index, self.mono,
                            ))
                            .with_min_height(280.0)
                            .finish(),
                        )
                        .with_background(theme::panel())
                        .finish(),
                    )
                    .finish(),
                );
            }
        } else {
            col.add_child(
                Expanded::new(
                    1.0,
                    Container::new(Flex::column().finish())
                        .with_background(theme::panel())
                        .finish(),
                )
                .finish(),
            );
        }

        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(col.finish())
                        .with_background(theme::panel())
                        .finish(),
                )
                .finish(),
            )
            .finish()
    }

    fn grid_shell(&self) -> Box<dyn Element> {
        let grid = self.grid_view();
        if !self.join_modal_open {
            return grid;
        }
        let mut stack = Stack::new();
        stack.add_child(grid);
        stack.add_child(self.join_modal());
        stack.finish()
    }

    fn share_status_line(&self) -> String {
        if let Some(msg) = &self.share_status {
            return msg.clone();
        }
        let count = self.share_entries.len();
        if let Some(err) = &self.share_error {
            return format!("VAULT · {err}");
        }
        format!("VAULT · {count} 项")
    }

    fn share_address_label(&self) -> String {
        let device = truncate_middle(&self.browsing_label, 24);
        if self.share_path.is_empty() {
            device
        } else if self.share_path.len() == 1 {
            let root_name = self
                .share_entries
                .iter()
                .find(|entry| entry.volume_id.as_deref() == Some(self.share_path[0].as_str()))
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| self.share_path[0].clone());
            format!("{device} · {root_name}")
        } else {
            format!("{}\\{}", device, self.share_path[1..].join("\\"))
        }
    }

    fn share_row(&self, entry: &ShareEntryDto) -> Box<dyn Element> {
        let is_folder = entry.kind.eq_ignore_ascii_case("folder");
        let volume_id = entry.volume_id.clone();
        let name = entry.name.clone();
        let modified = format_share_modified(entry.modified_at);
        let kind = share_type_label(entry);
        let size = if is_folder {
            "—".to_string()
        } else {
            format_size(entry.size)
        };

        let mut name_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        if !is_folder && !entry.local {
            let sync_name = name.clone();
            name_row.add_child(
                EventHandler::new(
                    Container::new(icons::share_sync_icon(self.share_file_busy))
                        .with_horizontal_margin(4.0)
                        .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::ShareSyncFile(sync_name.clone()));
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            );
        }
        name_row.add_child(
            Container::new(icons::share_file_icon(&entry.name, is_folder))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        name_row.add_child(
            Shrinkable::new(
                1.0,
                ui_text::body(entry.name.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
        );

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        row.add_child(
            Shrinkable::new(
                0.48,
                Container::new(name_row.finish())
                    .with_uniform_padding(8.0)
                    .finish(),
            )
            .finish(),
        );
        row.add_child(
            Shrinkable::new(
                0.22,
                Container::new(
                    ui_text::mono(modified, self.mono)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(8.0)
                .finish(),
            )
            .finish(),
        );
        row.add_child(
            Shrinkable::new(
                0.18,
                Container::new(
                    ui_text::body(kind, self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(8.0)
                .finish(),
            )
            .finish(),
        );
        row.add_child(
            Shrinkable::new(
                0.12,
                Container::new(
                    ui_text::mono(size, self.mono)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(8.0)
                .finish(),
            )
            .finish(),
        );

        let inner = Container::new(row.finish())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish();

        if is_folder {
            EventHandler::new(inner)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::ShareNavigate {
                        volume_id: volume_id.clone(),
                        name: name.clone(),
                    });
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            let click_name = name.clone();
            let menu_name = name.clone();
            EventHandler::new(inner)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::ShareFileClick(click_name.clone()));
                    DispatchEventResult::StopPropagation
                })
                .on_right_mouse_down(move |ctx, _, position| {
                    ctx.dispatch_typed_action(DevicesAction::OpenShareContextMenu {
                        name: menu_name.clone(),
                        x: position.x(),
                        y: position.y(),
                    });
                    DispatchEventResult::StopPropagation
                })
                .finish()
        }
    }

    fn share_table_header(&self) -> Box<dyn Element> {
        let mut row = Flex::row().with_main_axis_size(MainAxisSize::Min);
        for (label, weight) in [
            ("名称", 0.48),
            ("修改日期", 0.22),
            ("类型", 0.18),
            ("大小", 0.12),
        ] {
            row.add_child(
                Shrinkable::new(
                    weight,
                    Container::new(
                        ui_text::hud_title(label, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_uniform_padding(8.0)
                    .finish(),
                )
                .finish(),
            );
        }
        row.finish()
    }

    fn files_view(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);

        let mut toolbar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        toolbar.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::cluster_ctrl("← 终端", self.mono)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_horizontal_padding(4.0)
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::BackToGrid);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        toolbar.add_child(
            Container::new(self.share_nav_button(
                DevicesAction::ShareBack,
                !self.share_history_back.is_empty(),
                true,
            ))
            .with_horizontal_margin(6.0)
            .finish(),
        );
        toolbar.add_child(self.share_nav_button(
            DevicesAction::ShareForward,
            !self.share_history_forward.is_empty(),
            false,
        ));
        toolbar.add_child(
            Expanded::new(
                1.0,
                Container::new(
                    ConstrainedBox::new(
                        ui_text::cluster_label(self.share_address_label(), self.mono)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_height(TOOLBAR_BTN_HEIGHT)
                    .finish(),
                )
                .with_horizontal_margin(10.0)
                .with_background(theme::panel())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .finish(),
            )
            .finish(),
        );
        if self.browsing_local() && self.share_path.is_empty() {
            toolbar.add_child(
                Container::new(self.toolbar_button(
                    "+ 共享文件夹",
                    DevicesAction::OpenShareAddModal,
                    true,
                    128.0,
                    true,
                ))
                .with_horizontal_margin(8.0)
                .finish(),
            );
        }
        col.add_child(toolbar.finish());
        col.add_child(
            Container::new(
                ui_text::cluster_status(self.share_status_line(), self.mono)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        col.add_child(self.share_table_header());
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(4.0)
                .finish(),
        );

        if let Some(err) = &self.share_error {
            col.add_child(status_line(err.clone(), self.font, StatusTone::Warn));
        }

        let mut list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if self.share_entries.is_empty() && self.share_error.is_none() {
            list.add_child(
                Container::new(
                    ui_text::body("此文件夹为空", self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(16.0)
                .finish(),
            );
        } else {
            for entry in &self.share_entries {
                list.add_child(self.share_row(entry));
                list.add_child(
                    Container::new(Flex::column().finish())
                        .with_vertical_margin(2.0)
                        .finish(),
                );
            }
        }
        col.add_child(
            Expanded::new(
                1.0,
                ClippedScrollable::vertical(
                    self.share_scroll.clone(),
                    list.finish(),
                    ScrollbarWidth::Auto,
                    Fill::None,
                    Fill::None,
                    Fill::None,
                )
                .finish(),
            )
            .finish(),
        );

        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }

    fn share_add_modal(&self) -> Box<dyn Element> {
        let path_preview = if self.share_add_path.is_empty() {
            "例如：D:\\Projects\\Share".to_string()
        } else {
            self.share_add_path.clone()
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("增加共享文件夹", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    "选择本机目录并发布到当前终端的集群共享空间。其他终端可浏览并同步其中的文件。",
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            ui_text::hud_title("本机路径", self.font)
                .with_color(theme::muted())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::mono(path_preview, self.mono)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        );
        dialog.add_child(
            Container::new(self.toolbar_button(
                "浏览…",
                DevicesAction::BrowseShareAddPath,
                false,
                88.0,
                true,
            ))
            .with_vertical_margin(6.0)
            .finish(),
        );

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        actions.add_child(Expanded::new(1.0, Flex::column().finish()).finish());
        actions.add_child(self.toolbar_button(
            "取消",
            DevicesAction::CloseShareAddModal,
            false,
            88.0,
            true,
        ));
        actions.add_child(
            Container::new(self.toolbar_button(
                if self.share_add_busy {
                    "添加中…"
                } else {
                    "添加"
                },
                DevicesAction::SubmitShareAdd,
                true,
                88.0,
                !self.share_add_busy,
            ))
            .with_horizontal_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(12.0)
                .finish(),
        );
        if let Some((tone, msg)) = &self.share_add_feedback {
            dialog.add_child(status_line(msg.clone(), self.font, *tone));
        }

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(440.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        )
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseShareAddModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn share_context_menu(&self) -> Box<dyn Element> {
        let Some(name) = self.share_context_entry.clone() else {
            return Flex::column().finish();
        };
        let Some((x, y)) = self.share_context_pos else {
            return Flex::column().finish();
        };
        let entry = self.share_entries.iter().find(|entry| entry.name == name);
        let is_local = entry.map(|entry| entry.local).unwrap_or(true);

        let mut menu = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        menu.add_child(
            Container::new(
                ui_text::body(name.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish(),
        );
        menu.add_child(self.share_context_item(
            "打开",
            DevicesAction::ShareOpenFile(name.clone()),
            false,
        ));
        if !is_local {
            menu.add_child(self.share_context_item(
                "同步",
                DevicesAction::ShareSyncFile(name.clone()),
                false,
            ));
            menu.add_child(self.share_context_item(
                "远程打开",
                DevicesAction::ShareRemoteOpenFile(name.clone()),
                false,
            ));
        }
        menu.add_child(self.share_context_item("删除", DevicesAction::ShareDeleteFile(name), true));

        let panel = Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(180.0)
                .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();

        EventHandler::new(
            Container::new(panel)
                .with_margin_left(x.max(8.0))
                .with_margin_top(y.max(8.0))
                .finish(),
        )
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
    }

    fn share_context_item(
        &self,
        label: &str,
        action: DevicesAction,
        danger: bool,
    ) -> Box<dyn Element> {
        let color = if danger {
            theme::danger()
        } else {
            theme::text()
        };
        EventHandler::new(
            Container::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(color)
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn files_shell(&self) -> Box<dyn Element> {
        let has_overlay = self.share_add_modal_open || self.share_context_entry.is_some();
        if !has_overlay {
            return self.files_view();
        }
        let mut stack = Stack::new();
        stack.add_child(self.files_view());
        if self.share_add_modal_open {
            stack.add_child(self.share_add_modal());
        }
        if self.share_context_entry.is_some() {
            let scrim = EventHandler::new(
                Container::new(Flex::column().finish())
                    .with_background(ColorU::new(8, 7, 11, 40))
                    .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseShareContextMenu);
                DispatchEventResult::StopPropagation
            })
            .finish();
            stack.add_child(scrim);
            stack.add_child(self.share_context_menu());
        }
        stack.finish()
    }
}

fn dim_color(color: ColorU, factor: f32) -> ColorU {
    let scale = |component: u8| ((component as f32) * factor).round() as u8;
    ColorU::new(
        scale(color.r),
        scale(color.g),
        scale(color.b),
        scale(color.a),
    )
}

fn format_share_modified(timestamp: u64) -> String {
    if timestamp == 0 {
        return "—".to_string();
    }
    let secs = timestamp as i64;
    let days = secs / 86_400;
    let hour = (secs / 3600) % 24;
    let minute = (secs / 60) % 60;
    let year = 1970 + days / 365;
    let month = ((days % 365) / 30).clamp(1, 12);
    let day = ((days % 365) % 30).clamp(1, 28);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn share_type_label(entry: &ShareEntryDto) -> String {
    if entry.kind.eq_ignore_ascii_case("folder") {
        return "文件夹".into();
    }
    let ext = entry
        .name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" => "文本文档".into(),
        "md" => "Markdown".into(),
        "json" => "JSON".into(),
        "pdf" => "PDF".into(),
        "toml" => "TOML".into(),
        "rs" => "Rust 源文件".into(),
        "swift" => "Swift 源文件".into(),
        _ => "文件".into(),
    }
}

fn default_share_browse_path() -> String {
    #[cfg(windows)]
    {
        "D:\\Projects\\Share".to_string()
    }
    #[cfg(not(windows))]
    {
        "/Users/Shared/Wormhole".to_string()
    }
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
        let body = match self.mode {
            ViewMode::Grid => self.grid_shell(),
            ViewMode::Files => self.files_shell(),
        };
        tab_content_fill(body)
    }
}

impl TypedActionView for DevicesView {
    type Action = DevicesAction;

    fn handle_action(&mut self, action: &DevicesAction, ctx: &mut ViewContext<Self>) {
        match action {
            DevicesAction::Refresh => self.refresh_cluster(ctx),
            DevicesAction::RetryDeviceBootstrap => self.retry_device_bootstrap(ctx),
            DevicesAction::OpenNode(node_id) => self.open_node(node_id.clone(), ctx),
            DevicesAction::BackToGrid => self.back_to_grid(ctx),
            DevicesAction::OpenJoinModal => self.open_join_modal(ctx),
            DevicesAction::CloseJoinModal => self.close_join_modal(ctx),
            DevicesAction::PasteJoinInvite => self.paste_join_invite(ctx),
            DevicesAction::SubmitJoin => {
                if !self.invite_busy {
                    self.submit_join(ctx);
                }
            }
            DevicesAction::CopyInvite => self.copy_invite(ctx),
            DevicesAction::ToggleClusterPicker => self.toggle_cluster_picker(ctx),
            DevicesAction::CloseClusterPicker => self.close_cluster_picker(ctx),
            DevicesAction::SelectCluster(cluster_id) => {
                self.select_cluster(cluster_id.clone(), ctx);
            }
            DevicesAction::ShareBack => self.share_back(ctx),
            DevicesAction::ShareForward => self.share_forward(ctx),
            DevicesAction::ShareNavigate { volume_id, name } => {
                self.navigate_share_to(volume_id.clone(), name.clone(), ctx);
            }
            DevicesAction::OpenShareAddModal => self.open_share_add_modal(ctx),
            DevicesAction::CloseShareAddModal => self.close_share_add_modal(ctx),
            DevicesAction::BrowseShareAddPath => self.browse_share_add_path(ctx),
            DevicesAction::SubmitShareAdd => {
                if !self.share_add_busy {
                    self.submit_share_add(ctx);
                }
            }
            DevicesAction::ShareOpenFile(name) => self.open_share_file(name.clone(), ctx),
            DevicesAction::ShareSyncFile(name) => self.sync_share_file(name.clone(), ctx),
            DevicesAction::ShareRemoteOpenFile(name) => {
                self.remote_open_share_file(name.clone(), ctx);
            }
            DevicesAction::ShareDeleteFile(name) => self.delete_share_file(name.clone(), ctx),
            DevicesAction::OpenShareContextMenu { name, x, y } => {
                self.share_context_entry = Some(name.clone());
                self.share_context_pos = Some((*x, *y));
                ctx.notify();
            }
            DevicesAction::CloseShareContextMenu => self.close_share_context_menu(ctx),
            DevicesAction::ShareFileClick(name) => self.handle_share_file_click(name.clone(), ctx),
        }
    }
}

fn short_cluster_id(id: &str) -> String {
    if id.len() > 12 {
        id.chars().take(12).collect()
    } else {
        id.to_string()
    }
}
