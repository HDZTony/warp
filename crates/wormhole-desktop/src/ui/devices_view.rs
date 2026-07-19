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
use crate::ui::cluster_topology_panel::{node_remote_desktop_available, ClusterTopologyPanel};
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::fetch_cluster_for_ui;
use crate::ui::devices_actions::DevicesAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    section_hint, section_title, status_line, tab_content_fill, truncate_middle, StatusTone,
    HUD_RADIUS, SECTION_PADDING,
};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::{
    add_storage_volume_to_node, cached_cluster_invite_from_json, cached_cluster_invite_if_fresh,
    create_cluster as create_cluster_command, create_share_entry, delete_cluster,
    delete_share_entry, join_cluster, leave_cluster, list_share_directory, open_share_entry,
    prepare_cluster_invite_for_copy, remote_open_share_entry, remote_open_share_entry_on_host,
    remove_cluster_device, remove_cluster_node, remove_storage_volume_from_node,
    rename_share_entry, switch_active_cluster, sync_share_entry, AddStorageVolumeParams,
    CachedClusterInvite, ClusterNodeDto, ClusterStatusDto, CreateClusterInviteParams,
    CreateClusterParams, CreateShareEntryKind, CreateShareEntryParams, DeleteClusterParams,
    JoinClusterOutcome, JoinClusterParams, JoinedClusterDto, LeaveClusterParams,
    ListShareDirectoryParams, RemoveClusterDeviceParams, RemoveClusterNodeParams,
    RenameShareEntryParams, ShareEntryActionParams, ShareEntryDto, SwitchActiveClusterParams,
    NODE_PRESENCE_HANDSHAKE_FAILED, NODE_PRESENCE_SIGNED_IN,
};
use wormhole_desktop_core::device_remarks::load_device_remarks;
use wormhole_desktop_core::workspace_ui::{
    workspace_app_preference, workspace_approve_provision_job,
    workspace_approve_provision_job_with_candidate, workspace_list_provision_jobs,
    workspace_list_workers, workspace_probe_vm_candidates, workspace_save_app_preference,
    WorkspaceProvisionJob, WorkspaceProvisionRequest, WorkspaceProvisionStage,
    WorkspaceVmCandidate, WorkspaceWorker,
};

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Grid,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextItemStyle {
    Normal,
    Accent,
    Danger,
}

#[derive(Debug, Clone)]
pub enum DevicesEvent {
    OpenChat { node_id: String },
    OpenRemoteDesktop { node_id: String },
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
    share_loading: bool,
    share_load_seq: u64,
    share_path: Vec<String>,
    share_history_back: Vec<Vec<String>>,
    share_history_forward: Vec<Vec<String>>,
    share_status: Option<String>,
    share_add_modal_open: bool,
    share_add_path: String,
    share_add_feedback: Option<(StatusTone, String)>,
    share_add_busy: bool,
    share_new_menu_open: bool,
    share_rename_modal_open: bool,
    share_rename_entry: Option<String>,
    share_rename_draft: String,
    share_rename_field: TextFieldState,
    share_rename_focused: bool,
    share_rename_feedback: Option<(StatusTone, String)>,
    share_rename_busy: bool,
    create_cluster_modal_open: bool,
    create_cluster_name: String,
    create_cluster_name_field: TextFieldState,
    create_cluster_name_focused: bool,
    create_cluster_feedback: Option<(StatusTone, String)>,
    join_modal_open: bool,
    join_invite_draft: String,
    join_feedback: Option<(StatusTone, String)>,
    cached_invite: Option<CachedClusterInvite>,
    invite_busy: bool,
    status_flash: Option<String>,
    cluster_picker_open: bool,
    copy_invite_ack: bool,
    copy_invite_busy: bool,
    create_cluster_busy: bool,
    share_scroll: ClippedScrollStateHandle,
    share_context_entry: Option<String>,
    share_context_pos: Option<(f32, f32)>,
    share_unshare_volume_id: Option<String>,
    share_unshare_name: Option<String>,
    share_unshare_busy: bool,
    share_file_busy: bool,
    last_file_click: Option<(String, std::time::Instant)>,
    selected_share_file: Option<String>,
    workspace_workers: Vec<WorkspaceWorker>,
    workspace_vm_candidates: Vec<WorkspaceVmCandidate>,
    workspace_jobs: Vec<WorkspaceProvisionJob>,
    workspace_remote_job: Option<WorkspaceProvisionJob>,
    workspace_status: String,
    workspace_loading: bool,
    workspace_selected_app: String,
    workspace_dismissed_approval: Option<String>,
    workspace_auto_opened_job: Option<String>,
    bootstrap_busy: bool,
    bootstrap_pending_since: Option<Instant>,
    /// First time we observed remote `signed_in` without Online (for relay tip).
    signed_in_peers_since: Option<Instant>,
    caret_blink: CaretBlink,
    selected_node_id: Option<String>,
    hovered_node_id: Option<String>,
    delete_modal_node_id: Option<String>,
    delete_modal_cluster_id: Option<String>,
    delete_cluster_busy: bool,
    device_context_menu: Option<(String, f32, f32)>,
    last_node_click: Option<(String, Instant)>,
    stable_cluster_poll_scheduled: bool,
    cluster_refresh_busy: bool,
    device_remarks: BTreeMap<String, String>,
}

const TOOLBAR_BTN_HEIGHT: f32 = 32.0;
const TOOLBAR_BTN_PAD_X: f32 = 18.0;
const CLUSTER_SELECT_MIN_WIDTH: f32 = 240.0;
const GRID_SECTION_TITLE_HEIGHT: f32 = 28.0;
const BOOTSTRAP_PENDING_HINT_AFTER: Duration = Duration::from_secs(35);
const SIGNED_IN_RELAY_HINT_AFTER: Duration = Duration::from_secs(30);
const STABLE_CLUSTER_POLL_INTERVAL: Duration = Duration::from_secs(10);

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
            share_loading: false,
            share_load_seq: 0,
            share_path: Vec::new(),
            share_history_back: Vec::new(),
            share_history_forward: Vec::new(),
            share_status: None,
            share_add_modal_open: false,
            share_add_path: String::new(),
            share_add_feedback: None,
            share_add_busy: false,
            share_new_menu_open: false,
            share_rename_modal_open: false,
            share_rename_entry: None,
            share_rename_draft: String::new(),
            share_rename_field: TextFieldState::new(),
            share_rename_focused: false,
            share_rename_feedback: None,
            share_rename_busy: false,
            create_cluster_modal_open: false,
            create_cluster_name: "Wormhole Cluster".to_string(),
            create_cluster_name_field: TextFieldState::new(),
            create_cluster_name_focused: false,
            create_cluster_feedback: None,
            join_modal_open: false,
            join_invite_draft: String::new(),
            join_feedback: None,
            cached_invite: None,
            invite_busy: false,
            status_flash: None,
            cluster_picker_open: false,
            copy_invite_ack: false,
            copy_invite_busy: false,
            create_cluster_busy: false,
            share_scroll: ClippedScrollStateHandle::new(),
            share_context_entry: None,
            share_context_pos: None,
            share_unshare_volume_id: None,
            share_unshare_name: None,
            share_unshare_busy: false,
            share_file_busy: false,
            last_file_click: None,
            selected_share_file: None,
            workspace_workers: Vec::new(),
            workspace_vm_candidates: Vec::new(),
            workspace_jobs: Vec::new(),
            workspace_remote_job: None,
            workspace_status: "选择远端文件以检测来源电脑运行器".into(),
            workspace_loading: false,
            workspace_selected_app: "default".into(),
            workspace_dismissed_approval: None,
            workspace_auto_opened_job: None,
            bootstrap_busy: false,
            bootstrap_pending_since: None,
            signed_in_peers_since: None,
            caret_blink: CaretBlink::new(),
            selected_node_id: None,
            hovered_node_id: None,
            delete_modal_node_id: None,
            delete_modal_cluster_id: None,
            delete_cluster_busy: false,
            device_context_menu: None,
            last_node_click: None,
            stable_cluster_poll_scheduled: false,
            cluster_refresh_busy: false,
            device_remarks: BTreeMap::new(),
        };
        view.refresh_cluster(ctx);
        view
    }

    pub fn set_remote_desktop_result(
        &mut self,
        result: Result<(), String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.status_flash = Some(match result {
            Ok(()) => "已打开远程桌面窗口".into(),
            Err(err) => format!("无法打开远程桌面：{err}"),
        });
        ctx.notify();
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

    fn note_signed_in_peers(&mut self, status: &ClusterStatusDto) {
        let has_signed_in = status.nodes.iter().any(|node| {
            node.node_id != status.local_node_id
                && !node.online
                && node.presence_status == NODE_PRESENCE_SIGNED_IN
        });
        if has_signed_in {
            if self.signed_in_peers_since.is_none() {
                self.signed_in_peers_since = Some(Instant::now());
            }
        } else {
            self.signed_in_peers_since = None;
        }
    }

    fn signed_in_peers_slow(&self) -> bool {
        self.signed_in_peers_since
            .is_some_and(|started| started.elapsed() >= SIGNED_IN_RELAY_HINT_AFTER)
    }

    fn apply_cluster_status(&mut self, status: ClusterStatusDto, ctx: &mut ViewContext<Self>) {
        let should_reload_share_root = should_reload_share_root_on_cluster_update(
            self.cluster.as_ref(),
            &status,
            self.browsing_node_id.as_deref(),
            self.mode == ViewMode::Files && self.share_path.is_empty(),
            self.share_entries.is_empty() && !self.share_loading,
        );
        self.invalidate_invite_cache_for_status(&status);
        self.note_bootstrap_pending(&status);
        self.note_signed_in_peers(&status);
        self.cluster_syncing = status.syncing;
        self.cluster = Some(status.clone());
        self.cluster_error = None;
        if status.syncing {
            self.stable_cluster_poll_scheduled = false;
            self.schedule_cluster_poll(ctx);
        } else if status.auth_required || status.device_bootstrap_required {
            self.stable_cluster_poll_scheduled = false;
            self.schedule_bootstrap_poll(ctx);
        } else if status.configured {
            self.schedule_stable_cluster_poll(ctx);
        } else {
            self.stable_cluster_poll_scheduled = false;
        }
        if should_reload_share_root {
            self.load_share_directory(ctx);
        }
    }

    fn invalidate_invite_cache_for_status(&mut self, status: &ClusterStatusDto) {
        let Some(cached) = self.cached_invite.clone() else {
            return;
        };
        let (Some(active_id), Some(entry)) = (
            Self::active_cluster_id(status),
            Self::active_cluster_entry(status),
        ) else {
            self.cached_invite = None;
            return;
        };
        if cached_cluster_invite_if_fresh(&cached, &active_id, entry.membership_epoch).is_none() {
            self.cached_invite = None;
        }
    }

    /// Instantly drop a cluster from the local HUD; control-plane delete continues in the background.
    fn apply_optimistic_remove_cluster(&mut self, cluster_id: &str, ctx: &mut ViewContext<Self>) {
        let Some(mut status) = self.cluster.take() else {
            return;
        };
        let was_active = status.cluster_id.as_deref() == Some(cluster_id)
            || status
                .clusters
                .iter()
                .any(|entry| entry.active && entry.cluster_id == cluster_id);
        status
            .clusters
            .retain(|entry| entry.cluster_id != cluster_id);
        if was_active {
            if self.mode == ViewMode::Files {
                self.mode = ViewMode::Grid;
                self.browsing_node_id = None;
                self.browsing_label.clear();
                self.share_entries.clear();
                self.share_error = None;
                self.cancel_share_directory_load();
                self.share_path.clear();
                self.share_history_back.clear();
                self.share_history_forward.clear();
                self.share_status = None;
                self.share_add_modal_open = false;
            }
            if let Some(next_id) = status
                .clusters
                .first()
                .map(|entry| entry.cluster_id.clone())
            {
                for entry in &mut status.clusters {
                    entry.active = entry.cluster_id == next_id;
                }
                let next = status
                    .clusters
                    .iter()
                    .find(|entry| entry.cluster_id == next_id)
                    .expect("next_id taken from clusters");
                status.configured = true;
                status.cluster_id = Some(next_id);
                status.joined_at = Some(next.joined_at);
                status.role_stale = next.role_stale;
                let local_id = status.local_node_id.clone();
                status.nodes.retain(|node| node.node_id == local_id);
                status
                    .storage_volumes
                    .retain(|volume| volume.node_id == local_id);
                status.syncing = true;
            } else {
                status.configured = false;
                status.cluster_id = None;
                status.joined_at = None;
                status.nodes.clear();
                status.storage_volumes.clear();
                status.syncing = false;
                status.role_stale = true;
            }
        }
        self.cached_invite = None;
        self.apply_cluster_status(status, ctx);
    }

    /// Instantly hide a remote terminal from the topology; removal continues in the background.
    fn apply_optimistic_remove_node(&mut self, node_id: &str, ctx: &mut ViewContext<Self>) {
        let Some(mut status) = self.cluster.take() else {
            return;
        };
        status.nodes.retain(|node| node.node_id != node_id);
        status
            .storage_volumes
            .retain(|volume| volume.node_id != node_id);
        if self.browsing_node_id.as_deref() == Some(node_id) {
            self.mode = ViewMode::Grid;
            self.browsing_node_id = None;
            self.browsing_label.clear();
            self.share_entries.clear();
            self.share_error = None;
            self.cancel_share_directory_load();
            self.share_path.clear();
            self.share_history_back.clear();
            self.share_history_forward.clear();
            self.share_status = None;
            self.share_add_modal_open = false;
        }
        self.apply_cluster_status(status, ctx);
    }

    pub fn refresh_cluster(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let status = fetch_cluster_for_ui(&state).await;
                let remarks = load_device_remarks(&state.data_dir)
                    .await
                    .unwrap_or_default();
                (status, remarks)
            },
            move |view, output, ctx| {
                let (status, remarks) = output;
                view.device_remarks = remarks;
                match status {
                    Ok(status) => view.apply_cluster_status(status, ctx),
                    Err(e) => {
                        view.cluster = None;
                        view.cluster_syncing = false;
                        view.cluster_error = Some(e);
                        view.bootstrap_pending_since = None;
                    }
                }
                view.refresh_workspace(ctx);
                ctx.notify();
            },
        );
    }

    fn manual_refresh_cluster(&mut self, ctx: &mut ViewContext<Self>) {
        if self.cluster_refresh_busy {
            return;
        }
        self.close_cluster_picker(ctx);
        self.cluster_refresh_busy = true;
        ctx.notify();

        let core = self.core.clone();
        let was_browsing = self.mode == ViewMode::Files;
        let browsing_node_id = self.browsing_node_id.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let gossip =
                    wormhole_desktop_core::cluster_commands::refresh_cluster_gossip_peers_now(
                        &state,
                    )
                    .await;
                let status = fetch_cluster_for_ui(&state).await;
                let remarks = load_device_remarks(&state.data_dir)
                    .await
                    .unwrap_or_default();
                (gossip, status, remarks)
            },
            move |view, output, ctx| {
                view.cluster_refresh_busy = false;
                let (gossip, status, remarks) = output;
                view.device_remarks = remarks;
                if let Err(err) = gossip {
                    view.status_flash = Some(format!("REFRESH FAILED · {err}"));
                }
                match status {
                    Ok(status) => {
                        let node_count = status.nodes.len();
                        view.apply_cluster_status(status, ctx);
                        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
                        view.status_flash =
                            Some(format!("REFRESHED · {node_count} NODES · {timestamp}"));
                        if was_browsing {
                            if let Some(node_id) = browsing_node_id {
                                let node_still_present =
                                    view.cluster.as_ref().is_some_and(|cluster| {
                                        cluster.nodes.iter().any(|node| node.node_id == node_id)
                                    });
                                if node_still_present {
                                    view.load_share_directory(ctx);
                                }
                            }
                        }
                        ctx.notify();
                        ctx.spawn(
                            async move {
                                tokio::time::sleep(Duration::from_millis(2800)).await;
                            },
                            |view, _, ctx| {
                                view.status_flash = None;
                                ctx.notify();
                            },
                        );
                    }
                    Err(e) => {
                        view.cluster = None;
                        view.cluster_syncing = false;
                        view.cluster_error = Some(e);
                        view.bootstrap_pending_since = None;
                        ctx.notify();
                    }
                }
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
            move |view, output, ctx| {
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
                match output {
                    Ok(status) => {
                        view.cluster_error = None;
                        view.apply_cluster_status(status, ctx);
                    }
                    Err(e) => {
                        view.cluster_error = Some(e);
                        if view.cluster_syncing {
                            view.schedule_cluster_poll(ctx);
                        }
                    }
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
                    }
                } else if let Err(e) = output {
                    view.cluster_error = Some(e);
                    view.schedule_bootstrap_poll(ctx);
                }
                ctx.notify();
            },
        );
    }

    fn schedule_stable_cluster_poll(&mut self, ctx: &mut ViewContext<Self>) {
        if self.stable_cluster_poll_scheduled {
            return;
        }
        self.stable_cluster_poll_scheduled = true;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                tokio::time::sleep(STABLE_CLUSTER_POLL_INTERVAL).await;
                let state = core.runtime().state.clone();
                fetch_cluster_for_ui(&state).await
            },
            |view, output, ctx| {
                view.stable_cluster_poll_scheduled = false;
                match output {
                    Ok(status) => {
                        view.cluster_error = None;
                        view.apply_cluster_status(status, ctx);
                    }
                    Err(e) => {
                        view.cluster_error = Some(e);
                        if view
                            .cluster
                            .as_ref()
                            .is_some_and(|cluster| cluster.configured)
                        {
                            view.schedule_stable_cluster_poll(ctx);
                        }
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

    fn cancel_share_directory_load(&mut self) {
        self.share_load_seq = self.share_load_seq.wrapping_add(1);
        self.share_loading = false;
    }

    fn load_share_directory(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(node_id) = self.browsing_node_id.clone() else {
            return;
        };
        let path = self.share_path_string();
        self.share_load_seq = self.share_load_seq.wrapping_add(1);
        let load_seq = self.share_load_seq;
        self.share_loading = true;
        self.share_entries.clear();
        self.share_error = None;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let result = list_share_directory(
                    &state,
                    ListShareDirectoryParams {
                        node_id: node_id.clone(),
                        path: path.clone(),
                    },
                )
                .await;
                (load_seq, node_id, path, result)
            },
            |view, output, ctx| {
                let (load_seq, node_id, path, result) = output;
                let still_current = view.share_load_seq == load_seq
                    && view.browsing_node_id.as_deref() == Some(node_id.as_str())
                    && view.share_path_string() == path;
                if !still_current {
                    return;
                }
                view.share_loading = false;
                match result {
                    Ok(entries) => {
                        view.share_entries = entries;
                        view.share_error = None;
                        if view
                            .selected_share_file
                            .as_ref()
                            .is_some_and(|name| !view.share_entries.iter().any(|e| &e.name == name))
                        {
                            view.selected_share_file = None;
                        }
                    }
                    Err(e) => {
                        view.share_entries.clear();
                        view.share_error = Some(e);
                    }
                }
                view.refresh_workspace(ctx);
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn selected_share_entry(&self) -> Option<&ShareEntryDto> {
        let selected = self.selected_share_file.as_deref()?;
        self.share_entries
            .iter()
            .find(|entry| entry.name == selected && entry.kind.eq_ignore_ascii_case("file"))
    }

    fn select_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        let Some(entry) = self
            .share_entries
            .iter()
            .find(|entry| entry.name == entry_name && entry.kind.eq_ignore_ascii_case("file"))
        else {
            return;
        };
        let preference_file = entry.name.clone();
        let entry_id = entry.entry_id.clone();
        self.selected_share_file = Some(entry_name);
        self.workspace_selected_app = recommended_workspace_app(&preference_file).to_string();
        if self
            .workspace_remote_job
            .as_ref()
            .is_some_and(|job| entry_id.as_deref() != Some(job.request.entry_id.as_str()))
        {
            self.workspace_remote_job = None;
        }
        self.refresh_workspace(ctx);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                workspace_app_preference(&state, &preference_file).await
            },
            |view, result, ctx| {
                if let Ok(Some(app)) = result {
                    if view.selected_share_entry().is_some_and(|entry| {
                        workspace_apps_for_file(&entry.name).contains(&app.as_str())
                    }) {
                        view.workspace_selected_app = app;
                        view.update_workspace_status_from_selection();
                    }
                }
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn refresh_workspace(&mut self, ctx: &mut ViewContext<Self>) {
        if self.workspace_loading {
            return;
        }
        self.workspace_loading = true;
        let core = self.core.clone();
        let remote_job = self
            .workspace_remote_job
            .as_ref()
            .map(|job| (job.request.source_node_id.clone(), job.job_id.clone()));
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let workers = workspace_list_workers(&state).await;
                let jobs = workspace_list_provision_jobs(&state).await;
                let candidates = workspace_probe_vm_candidates().await;
                let remote = if let Some((node_id, job_id)) = remote_job {
                    Some(
                        wormhole_desktop_core::cluster_commands::remote_workspace_provision_status(
                            &state, &node_id, &job_id,
                        )
                        .await,
                    )
                } else {
                    None
                };
                (workers, jobs, candidates, remote)
            },
            |view, output, ctx| {
                view.workspace_loading = false;
                let (workers, jobs, candidates, remote) = output;
                match workers {
                    Ok(workers) => view.workspace_workers = workers,
                    Err(error) => {
                        view.workspace_workers.clear();
                        view.workspace_status = format!("Workspace worker 检测失败: {error}");
                    }
                }
                if let Ok(jobs) = jobs {
                    if jobs.iter().any(|job| {
                        job.stage == WorkspaceProvisionStage::Failed
                            && job.import_candidate.is_some()
                            && view.workspace_dismissed_approval.as_deref()
                                == Some(job.job_id.as_str())
                    }) {
                        view.workspace_dismissed_approval = None;
                    }
                    view.workspace_jobs = jobs;
                }
                if let Ok(candidates) = candidates {
                    view.workspace_vm_candidates = candidates;
                }
                if let Some(result) = remote {
                    match result {
                        Ok(job) => {
                            view.workspace_status = job.detail.clone();
                            let should_open = job.stage == WorkspaceProvisionStage::Ready
                                && view.workspace_auto_opened_job.as_deref()
                                    != Some(job.job_id.as_str());
                            if should_open {
                                view.workspace_auto_opened_job = Some(job.job_id.clone());
                            }
                            view.workspace_remote_job = Some(job);
                            if should_open {
                                view.workspace_open_selected(ctx);
                            }
                        }
                        Err(error) => {
                            view.workspace_status = format!("读取远端安装进度失败: {error}");
                        }
                    }
                } else {
                    view.update_workspace_status_from_selection();
                }
                ctx.notify();
            },
        );
    }

    fn update_workspace_status_from_selection(&mut self) {
        let Some(entry) = self.selected_share_entry() else {
            self.workspace_status = "选择远端文件以检测来源电脑运行器".into();
            return;
        };
        let Some(author) = entry.version_author.as_deref() else {
            self.workspace_status =
                "文件缺少 SyncIndex 来源节点，请在来源电脑重新扫描后刷新".into();
            return;
        };
        let Some(worker) = self.workspace_worker_for_author(author) else {
            self.workspace_status = "来源电脑尚未准备 Ubuntu 工具运行器".into();
            return;
        };
        if workspace_worker_supports_app(worker, &self.workspace_selected_app) {
            self.workspace_status = format!("{} · 隔离工具运行器已就绪", worker.hostname);
        } else {
            self.workspace_status = format!(
                "{} 在线，但镜像未提供 {}",
                worker.hostname, self.workspace_selected_app
            );
        }
    }

    fn workspace_worker_for_author(&self, author: &str) -> Option<&WorkspaceWorker> {
        self.workspace_workers.iter().find(|worker| {
            worker.node_id == author || worker.dispatch_endpoint_id.as_deref() == Some(author)
        })
    }

    fn workspace_open_selected(&mut self, ctx: &mut ViewContext<Self>) {
        if self.share_file_busy {
            return;
        }
        let Some(entry) = self.selected_share_entry() else {
            return;
        };
        let mut params = self.share_action_params(&entry.name);
        params.requested_app = Some(self.workspace_selected_app.clone());
        let entry_name = entry.name.clone();
        self.share_file_busy = true;
        self.workspace_status = format!("正在来源电脑的隔离运行器中打开 {entry_name}…");
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                remote_open_share_entry(&state, params).await
            },
            move |view, result, ctx| {
                view.share_file_busy = false;
                view.workspace_status = match result {
                    Ok(session) => format!(
                        "会话 {} 已提交，正在等待 Ubuntu 运行器就绪",
                        session.session_id
                    ),
                    Err(error) => format!("来源电脑运行器打开失败: {error}"),
                };
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn workspace_request_provision(&mut self, ctx: &mut ViewContext<Self>) {
        if self.workspace_loading || self.workspace_remote_job.is_some() {
            return;
        }
        let Some(entry) = self.selected_share_entry() else {
            return;
        };
        let Some(cluster_id) = self
            .cluster
            .as_ref()
            .and_then(|cluster| cluster.cluster_id.clone())
        else {
            self.workspace_status = "当前共享浏览没有有效的 cluster_id".into();
            ctx.notify();
            return;
        };
        let Some(source_node_id) = entry.version_author.clone() else {
            self.workspace_status = "文件缺少 SyncIndex 来源节点，无法安装到正确电脑".into();
            ctx.notify();
            return;
        };
        let Some(entry_id) = entry.entry_id.clone() else {
            self.workspace_status = "文件缺少 SyncIndex entry_id，无法创建安装任务".into();
            ctx.notify();
            return;
        };
        let request = WorkspaceProvisionRequest {
            cluster_id,
            source_node_id: source_node_id.clone(),
            entry_id,
            requested_app: self.workspace_selected_app.clone(),
            image_id: None,
            version: None,
            licensing_acknowledged: false,
            requested_by_node_id: None,
        };
        self.workspace_loading = true;
        self.workspace_status = "正在向来源电脑请求准备 Ubuntu 工具运行器…".into();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                wormhole_desktop_core::cluster_commands::request_remote_workspace_provision(
                    &state,
                    &source_node_id,
                    request,
                )
                .await
            },
            |view, result, ctx| {
                view.workspace_loading = false;
                match result {
                    Ok(job) => {
                        view.workspace_status = job.detail.clone();
                        view.workspace_remote_job = Some(job);
                    }
                    Err(error) => {
                        view.workspace_status = format!("请求安装失败: {error}");
                    }
                }
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn workspace_retry_provision(&mut self, ctx: &mut ViewContext<Self>) {
        if self.workspace_loading {
            return;
        }
        self.workspace_remote_job = None;
        self.workspace_auto_opened_job = None;
        self.workspace_request_provision(ctx);
    }

    fn workspace_approve_job(
        &mut self,
        job_id: String,
        candidate_vm_name: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.workspace_loading = true;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                if let Some(vm_name) = candidate_vm_name {
                    workspace_approve_provision_job_with_candidate(&state, &job_id, &vm_name).await
                } else {
                    workspace_approve_provision_job(&state, &job_id).await
                }
            },
            |view, result, ctx| {
                view.workspace_loading = false;
                match result {
                    Ok(job) => {
                        view.workspace_status = job.detail;
                        view.workspace_dismissed_approval = None;
                    }
                    Err(error) => {
                        view.workspace_status = format!("批准安装失败: {error}");
                    }
                }
                view.refresh_workspace(ctx);
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn workspace_next_app(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(entry) = self.selected_share_entry() else {
            return;
        };
        let file_name = entry.name.clone();
        let apps = workspace_apps_for_file(&file_name);
        let current = apps
            .iter()
            .position(|app| *app == self.workspace_selected_app)
            .unwrap_or(0);
        self.workspace_selected_app = apps[(current + 1) % apps.len()].to_string();
        let app = self.workspace_selected_app.clone();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                workspace_save_app_preference(&state, &file_name, &app).await
            },
            |view, result, ctx| {
                if let Err(error) = result {
                    view.workspace_status = format!("保存打开方式失败: {error}");
                }
                ctx.notify();
            },
        );
        self.update_workspace_status_from_selection();
        ctx.notify();
    }

    pub fn open_node_from_chat(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        self.open_node(node_id, ctx);
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
        self.device_context_menu = None;
        let is_remote = node_id != local_id;
        self.browsing_node_id = Some(node_id.clone());
        self.browsing_label = label;
        self.share_entries.clear();
        self.selected_share_file = None;
        self.workspace_remote_job = None;
        self.share_error = None;
        self.share_path.clear();
        self.share_history_back.clear();
        self.share_history_forward.clear();
        self.share_status = None;
        self.share_add_modal_open = false;
        self.device_context_menu = None;
        self.reset_share_scroll();
        self.load_share_directory(ctx);
        if is_remote {
            let core = self.core.clone();
            let opened_node = node_id.clone();
            ctx.spawn(
                async move {
                    let state = core.runtime().state.clone();
                    let gossip =
                        wormhole_desktop_core::cluster_commands::refresh_cluster_gossip_peers_now(
                            &state,
                        )
                        .await;
                    let status = fetch_cluster_for_ui(&state).await;
                    (gossip, status, opened_node)
                },
                move |view, output, ctx| {
                    let (gossip, status, opened_node) = output;
                    if let Err(err) = gossip {
                        tracing::debug!("open_node gossip refresh: {err}");
                    }
                    if let Ok(status) = status {
                        let still_browsing =
                            view.browsing_node_id.as_deref() == Some(opened_node.as_str());
                        view.apply_cluster_status(status, ctx);
                        if still_browsing
                            && view.mode == ViewMode::Files
                            && view.share_path.is_empty()
                        {
                            view.load_share_directory(ctx);
                        }
                    }
                    ctx.notify();
                },
            );
        }
        ctx.notify();
    }

    fn back_to_grid(&mut self, ctx: &mut ViewContext<Self>) {
        self.mode = ViewMode::Grid;
        self.browsing_node_id = None;
        self.browsing_label.clear();
        self.share_entries.clear();
        self.selected_share_file = None;
        self.workspace_remote_job = None;
        self.share_error = None;
        self.cancel_share_directory_load();
        self.share_path.clear();
        self.share_history_back.clear();
        self.share_history_forward.clear();
        self.share_status = None;
        self.share_add_modal_open = false;
        self.share_new_menu_open = false;
        self.share_rename_modal_open = false;
        self.share_rename_entry = None;
        self.share_context_entry = None;
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
        self.selected_share_file = None;
        self.workspace_remote_job = None;
        self.share_status = None;
        self.share_new_menu_open = false;
        self.share_context_entry = None;
        self.share_context_pos = None;
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
        self.selected_share_file = None;
        self.workspace_remote_job = None;
        self.share_status = None;
        self.share_new_menu_open = false;
        self.share_context_entry = None;
        self.share_context_pos = None;
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
        self.selected_share_file = None;
        self.workspace_remote_job = None;
        self.share_status = None;
        self.share_new_menu_open = false;
        self.share_context_entry = None;
        self.share_context_pos = None;
        self.reset_share_scroll();
        self.load_share_directory(ctx);
        ctx.notify();
    }

    fn browsing_local(&self) -> bool {
        self.cluster.as_ref().is_some_and(|cluster| {
            self.browsing_node_id.as_deref() == Some(cluster.local_node_id.as_str())
        })
    }

    fn browsing_can_manage(&self) -> bool {
        let Some(cluster) = self.cluster.as_ref() else {
            return false;
        };
        let Some(node_id) = self.browsing_node_id.as_deref() else {
            return false;
        };
        if node_id == cluster.local_node_id {
            return true;
        }
        cluster.nodes.iter().any(|node| {
            node.node_id == node_id
                && share_node_can_manage(
                    false,
                    node.same_account,
                    node.server_member_confirmed,
                    node.revoked,
                )
        })
    }

    fn share_new_menu_mode(&self) -> ShareNewMenuMode {
        share_new_menu_mode(self.browsing_can_manage(), self.share_path.is_empty())
    }

    fn can_show_share_new(&self) -> bool {
        self.share_new_menu_mode() != ShareNewMenuMode::Hidden
    }

    fn can_create_share_entry(&self) -> bool {
        self.share_new_menu_mode() == ShareNewMenuMode::CreateEntries
    }

    fn close_share_new_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if self.share_new_menu_open {
            self.share_new_menu_open = false;
            ctx.notify();
        }
    }

    fn toggle_share_new_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.can_show_share_new() {
            return;
        }
        self.close_share_context_menu(ctx);
        self.share_new_menu_open = !self.share_new_menu_open;
        ctx.notify();
    }

    fn create_share_item(&mut self, kind: CreateShareEntryKind, ctx: &mut ViewContext<Self>) {
        if !self.can_create_share_entry() || self.share_file_busy {
            return;
        }
        let Some(node_id) = self.browsing_node_id.clone() else {
            return;
        };
        let share_path = self.share_path_string();
        self.share_file_busy = true;
        self.share_new_menu_open = false;
        self.share_status = Some(match kind {
            CreateShareEntryKind::Folder => "正在新建文件夹…".into(),
            CreateShareEntryKind::Txt => "正在新建 txt…".into(),
        });
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                create_share_entry(
                    &state,
                    CreateShareEntryParams {
                        node_id,
                        share_path,
                        kind,
                        name: None,
                    },
                )
                .await
            },
            |view, output, ctx| {
                view.share_file_busy = false;
                match output {
                    Ok(created) => {
                        view.share_status = Some(format!("已新建 · {}", created.name));
                        view.load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_status = Some(format!("新建失败 · {e}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_share_rename_modal(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        if !self.browsing_can_manage() {
            return;
        }
        self.close_share_context_menu(ctx);
        self.share_rename_entry = Some(entry_name.clone());
        self.share_rename_draft = entry_name;
        self.share_rename_field = TextFieldState::new();
        self.share_rename_focused = true;
        self.share_rename_feedback = None;
        self.share_rename_busy = false;
        self.share_rename_modal_open = true;
        ctx.notify();
    }

    fn close_share_rename_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.share_rename_modal_open = false;
        self.share_rename_entry = None;
        self.share_rename_draft.clear();
        self.share_rename_focused = false;
        self.share_rename_feedback = None;
        self.share_rename_busy = false;
        ctx.notify();
    }

    fn edit_share_rename_field(&mut self, edit: &TextFieldEditAction, ctx: &mut ViewContext<Self>) {
        self.share_rename_field
            .apply(&mut self.share_rename_draft, edit);
        self.share_rename_focused = true;
        self.share_rename_feedback = None;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn confirm_share_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if self.share_rename_busy {
            return;
        }
        let Some(entry_name) = self.share_rename_entry.clone() else {
            return;
        };
        let Some(node_id) = self.browsing_node_id.clone() else {
            return;
        };
        let new_name = self.share_rename_draft.trim().to_string();
        if let Err(err) =
            wormhole_desktop_core::cluster_commands::validate_share_entry_name(&new_name)
        {
            self.share_rename_feedback = Some((StatusTone::Warn, err));
            ctx.notify();
            return;
        }
        let share_path = self.share_path_string();
        self.share_rename_busy = true;
        self.share_rename_feedback = Some((StatusTone::Neutral, "正在重命名…".into()));
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                rename_share_entry(
                    &state,
                    RenameShareEntryParams {
                        node_id,
                        share_path,
                        entry_name,
                        new_name,
                    },
                )
                .await
            },
            |view, output, ctx| {
                view.share_rename_busy = false;
                match output {
                    Ok(name) => {
                        view.share_rename_modal_open = false;
                        view.share_rename_entry = None;
                        view.share_rename_feedback = None;
                        view.share_status = Some(format!("已重命名 · {name}"));
                        view.load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_rename_feedback = Some((StatusTone::Warn, e));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_share_add_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.browsing_can_manage() {
            return;
        }
        self.close_share_new_menu(ctx);
        self.share_add_feedback = None;
        if self.browsing_local() && self.share_add_path.is_empty() {
            self.share_add_path = default_share_browse_path();
        } else if !self.browsing_local() {
            self.share_add_path.clear();
            self.share_add_feedback =
                Some((StatusTone::Muted, "请输入目标终端上的文件夹路径".into()));
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
        if !self.browsing_local() {
            self.share_add_feedback =
                Some((StatusTone::Muted, "远端终端请手动输入其本机路径".into()));
            ctx.notify();
            return;
        }
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(|| {
                    #[cfg(windows)]
                    {
                        wormhole_desktop_platform_windows::pick_folder("选择共享文件夹")
                    }
                    #[cfg(not(windows))]
                    {
                        rfd::FileDialog::new()
                            .set_title("选择共享文件夹")
                            .pick_folder()
                    }
                })
                .await
                .ok()
                .flatten()
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
            self.share_add_feedback = Some((StatusTone::Warn, "请输入目标终端路径".into()));
            ctx.notify();
            return;
        }
        let Some(node_id) = self.browsing_node_id.clone() else {
            self.share_add_busy = false;
            self.share_add_feedback = Some((
                StatusTone::Warn,
                "无法确定目标终端，请重新进入共享浏览".into(),
            ));
            ctx.notify();
            return;
        };
        self.share_add_busy = true;
        self.share_add_feedback = Some((StatusTone::Neutral, "正在添加共享文件夹…".into()));
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                add_storage_volume_to_node(&state, &node_id, AddStorageVolumeParams { path }).await
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

    fn share_action_params(&self, entry_name: &str) -> ShareEntryActionParams {
        let node_id = self.browsing_node_id.clone().unwrap_or_default();
        let share_path = self.share_path_string();
        let entry = self
            .share_entries
            .iter()
            .find(|entry| entry.name == entry_name);
        ShareEntryActionParams {
            node_id,
            share_path,
            entry_name: entry_name.to_string(),
            remote_size: entry.map(|entry| entry.size),
            remote_modified_at: entry.map(|entry| entry.modified_at),
            entry_id: entry.and_then(|entry| entry.entry_id.clone()),
            version_id: entry.and_then(|entry| entry.version_id.clone()),
            version_author: entry.and_then(|entry| entry.version_author.clone()),
            requested_app: Some(recommended_workspace_app(entry_name).to_string()),
        }
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
                ShareEntryActionParams,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), String>> + Send>,
            > + Send
            + 'static,
    {
        if self.share_file_busy {
            return;
        }
        let params = self.share_action_params(&entry_name);
        self.share_file_busy = true;
        self.share_status = Some(busy_label.to_string());
        self.close_share_context_menu(ctx);
        ctx.notify();
        let core = self.core.clone();
        let success = success_label.to_string();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                op(state, params).await
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
            |state, params| Box::pin(async move { open_share_entry(&state, params).await }),
            ctx,
        );
    }

    fn sync_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            "正在同步…",
            "已同步",
            |state, params| Box::pin(async move { sync_share_entry(&state, params).await }),
            ctx,
        );
    }

    fn remote_open_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        if self.share_file_busy {
            return;
        }
        let params = self.share_action_params(&entry_name);
        self.share_file_busy = true;
        self.share_status = Some("正在来源 Windows 打开并连接桌面…".into());
        self.close_share_context_menu(ctx);
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                remote_open_share_entry_on_host(&state, params).await
            },
            move |view, result, ctx| {
                view.share_file_busy = false;
                match result {
                    Ok(target) => {
                        view.share_status = Some(format!("Windows 打开方式已启动 · {entry_name}"));
                        ctx.emit(DevicesEvent::OpenRemoteDesktop {
                            node_id: target.node_id,
                        });
                    }
                    Err(error) => {
                        view.share_status = Some(format!("来源 Windows 打开失败 · {error}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn delete_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            "正在删除…",
            if self.browsing_can_manage() {
                "已删除"
            } else {
                "已删除本地副本"
            },
            |state, params| Box::pin(async move { delete_share_entry(&state, params).await }),
            ctx,
        );
    }

    fn handle_share_file_click(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.select_share_file(entry_name.clone(), ctx);
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

    fn handle_node_card_click(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        if self.device_context_menu.is_some() {
            self.device_context_menu = None;
        }
        let now = Instant::now();
        if let Some((last_id, last_at)) = &self.last_node_click {
            if last_id == &node_id && last_at.elapsed() < Duration::from_millis(450) {
                self.last_node_click = None;
                self.open_node(node_id, ctx);
                return;
            }
        }
        self.last_node_click = Some((node_id.clone(), now));
        self.selected_node_id = Some(node_id);
        ctx.notify();
    }

    fn open_delete_node_modal(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        self.delete_modal_node_id = Some(node_id);
        self.device_context_menu = None;
        ctx.notify();
    }

    fn close_delete_node_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.delete_modal_node_id = None;
        ctx.notify();
    }

    fn close_device_context_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.device_context_menu = None;
        ctx.notify();
    }

    fn confirm_delete_node(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(node_id) = self.delete_modal_node_id.clone() else {
            return;
        };
        let device_id = self
            .cluster
            .as_ref()
            .and_then(|cluster| cluster.nodes.iter().find(|node| node.node_id == node_id))
            .and_then(|node| {
                node.server_member_confirmed
                    .then(|| node.device_id.clone())
                    .flatten()
            });
        self.delete_modal_node_id = None;
        self.device_context_menu = None;
        if self.browsing_node_id.as_deref() == Some(node_id.as_str()) {
            self.back_to_grid(ctx);
        }
        self.remove_cluster_device(device_id, node_id, ctx);
    }

    fn cluster_node_label(&self, node_id: &str) -> Option<ClusterNodeDto> {
        self.cluster
            .as_ref()?
            .nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .cloned()
    }

    fn joined_cluster_label(entry: &JoinedClusterDto) -> String {
        entry
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| entry.folder_name.clone())
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

    fn active_cluster_id(cluster: &ClusterStatusDto) -> Option<String> {
        cluster
            .clusters
            .iter()
            .find(|c| c.active)
            .map(|c| c.cluster_id.clone())
            .or_else(|| cluster.cluster_id.clone())
    }

    fn active_cluster_entry(cluster: &ClusterStatusDto) -> Option<&JoinedClusterDto> {
        cluster.clusters.iter().find(|c| c.active)
    }

    fn active_cluster_membership_fresh(cluster: &ClusterStatusDto) -> bool {
        !cluster.role_stale
            && Self::active_cluster_entry(cluster)
                .is_some_and(|entry| !entry.role_stale && !entry.revoked)
    }

    fn active_cluster_can_invite(cluster: &ClusterStatusDto) -> bool {
        Self::active_cluster_membership_fresh(cluster)
            && Self::active_cluster_entry(cluster)
                .is_some_and(|entry| matches!(entry.role.as_str(), "owner" | "admin"))
    }

    fn active_cluster_is_owner(cluster: &ClusterStatusDto) -> bool {
        Self::active_cluster_membership_fresh(cluster)
            && Self::active_cluster_entry(cluster).is_some_and(|entry| entry.role == "owner")
    }

    fn active_cluster_is_default(cluster: &ClusterStatusDto) -> bool {
        Self::active_cluster_entry(cluster).is_some_and(|entry| entry.is_default)
    }

    fn active_cluster_can_leave(cluster: &ClusterStatusDto) -> bool {
        Self::active_cluster_membership_fresh(cluster)
            && Self::active_cluster_entry(cluster).is_some_and(|entry| entry.role != "owner")
    }

    fn membership_gate_message(cluster: &ClusterStatusDto) -> Option<&'static str> {
        if cluster.clusters.is_empty() {
            return None;
        }
        if cluster.role_stale
            || Self::active_cluster_entry(cluster).is_some_and(|entry| entry.role_stale)
        {
            return Some("MEMBERSHIP · 控面成员状态未同步 · 请刷新；若仍失败请重新加入或创建集群");
        }
        if !Self::active_cluster_can_invite(cluster)
            && Self::active_cluster_entry(cluster).is_some_and(|entry| !entry.revoked)
        {
            return Some(
                "MEMBERSHIP · 当前设备不是可管理成员 · 跨账号成员需管理员提升，或重新加入",
            );
        }
        None
    }

    fn active_cluster_folder_name(cluster: &ClusterStatusDto) -> String {
        cluster
            .clusters
            .iter()
            .find(|c| c.active)
            .map(|c| c.folder_name.clone())
            .unwrap_or_else(|| {
                cluster
                    .cluster_id
                    .as_deref()
                    .map(|id| format!("集群 {}", short_cluster_id(id)))
                    .unwrap_or_else(|| "集群".to_string())
            })
    }

    fn active_cluster_tag(cluster: &ClusterStatusDto) -> String {
        cluster
            .clusters
            .iter()
            .find(|c| c.active)
            .map(|c| {
                format!(
                    "{} · {}",
                    Self::joined_cluster_label(c),
                    short_cluster_id(&c.cluster_id)
                )
            })
            .unwrap_or_else(|| Self::cluster_label(cluster))
    }

    fn cluster_status_text(&self, cluster: &ClusterStatusDto) -> String {
        if let Some(flash) = &self.status_flash {
            return flash.clone();
        }
        if self.cluster_refresh_busy {
            return "REFRESH · 正在同步集群终端…".to_string();
        }
        if self.cluster_syncing {
            return "CLUSTER · SYNCING · 后台同步集群…".to_string();
        }
        if cluster.clusters.is_empty() {
            return "CLUSTER · EMPTY · 创建或加入集群开始同步".to_string();
        }
        if let Some(gate) = Self::membership_gate_message(cluster) {
            return gate.to_string();
        }
        let n = cluster.nodes.len();
        let online = cluster.nodes.iter().filter(|node| node.online).count();
        let signed_in = cluster
            .nodes
            .iter()
            .filter(|node| !node.online && node.presence_status == NODE_PRESENCE_SIGNED_IN)
            .count();
        let pending = cluster
            .nodes
            .iter()
            .filter(|node| node.pending_handshake)
            .count();
        let failed = cluster
            .nodes
            .iter()
            .filter(|node| node.presence_status == NODE_PRESENCE_HANDSHAKE_FAILED)
            .count();
        if pending > 0 || signed_in > 0 || failed > 0 {
            let mut parts = vec![
                format!("CLUSTER · {n} NODE{}", if n == 1 { "" } else { "S" }),
                format!("{online} ONLINE"),
            ];
            if signed_in > 0 {
                parts.push(format!("{signed_in} 已登录"));
            }
            if pending > 0 {
                parts.push(format!("{pending} 握手中"));
            }
            if failed > 0 {
                parts.push(format!("{failed} 连接失败"));
            }
            if pending > 0 {
                parts.push("后台重试连接中".into());
            } else if signed_in > 0 {
                if self.signed_in_peers_slow() {
                    parts.push("检查设置→P2P Relay=国内".into());
                } else {
                    parts.push("P2P 经 relay 拨号中".into());
                }
            }
            return parts.join(" · ");
        }
        format!(
            "CLUSTER · {n} NODE{} · {online} ONLINE · E2E ENCRYPTED · 单击共享文件浏览",
            if n == 1 { "" } else { "S" }
        )
    }

    fn cluster_status_color(&self) -> ColorU {
        if self.cluster_refresh_busy {
            theme::accent_cool()
        } else if self
            .cluster
            .as_ref()
            .and_then(Self::membership_gate_message)
            .is_some()
        {
            theme::danger()
        } else {
            theme::muted()
        }
    }

    fn cluster_refresh_button(&self) -> Box<dyn Element> {
        let spinning = self.cluster_refresh_busy;
        let opacity = if spinning { 0.75 } else { 1.0 };
        let icon_color = theme::accent_cool();
        let inner = Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(icons::cluster_refresh_icon(spinning, icon_color, opacity))
                    .finish(),
            )
            .with_width(icons::CLUSTER_REFRESH_BTN_SIZE)
            .with_height(TOOLBAR_BTN_HEIGHT)
            .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();

        if spinning {
            inner
        } else {
            EventHandler::new(inner)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::Refresh);
                    DispatchEventResult::StopPropagation
                })
                .finish()
        }
    }

    fn cluster_menu_divider() -> Box<dyn Element> {
        Container::new(Flex::column().finish())
            .with_vertical_margin(4.0)
            .with_horizontal_margin(6.0)
            .with_border(Border::top(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn cluster_menu_section(label: &str, mono: FamilyId) -> Box<dyn Element> {
        Container::new(
            ui_text::cluster_ctrl(label.to_string(), mono)
                .with_color(theme::muted())
                .finish(),
        )
        .with_horizontal_padding(10.0)
        .with_vertical_padding(6.0)
        .finish()
    }

    fn cluster_menu_action(
        &self,
        label: &str,
        action: DevicesAction,
        accent: bool,
        enabled: bool,
        danger: bool,
    ) -> Box<dyn Element> {
        let color = if !enabled {
            dim_color(
                if danger {
                    theme::danger()
                } else if accent {
                    theme::accent_cool()
                } else {
                    theme::text()
                },
                0.35,
            )
        } else if danger {
            theme::danger()
        } else if accent {
            theme::accent_cool()
        } else {
            theme::text()
        };
        let inner = Container::new(
            ui_text::cluster_label(label.to_string(), self.mono)
                .with_color(color)
                .finish(),
        )
        .with_horizontal_padding(10.0)
        .with_vertical_padding(8.0)
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

    fn cluster_menu_popover(&self, cluster: &ClusterStatusDto) -> Box<dyn Element> {
        let mut menu = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        menu.add_child(Self::cluster_menu_section("切换集群", self.mono));
        if cluster.clusters.is_empty() {
            menu.add_child(
                Container::new(
                    ui_text::cluster_label("暂无集群", self.mono)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_horizontal_padding(10.0)
                .with_vertical_padding(8.0)
                .finish(),
            );
        }
        for entry in &cluster.clusters {
            let cluster_id = entry.cluster_id.clone();
            let name = Self::joined_cluster_label(entry);
            let short_id = short_cluster_id(&entry.cluster_id);
            let (bg, color) = if entry.active {
                (theme::accent_cool_bg(40), theme::accent_cool())
            } else {
                (ColorU::new(0, 0, 0, 0), theme::text())
            };
            let check_color = if entry.active {
                theme::accent_cool()
            } else {
                ColorU::new(0, 0, 0, 0)
            };
            let mut item_row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max);
            item_row.add_child(
                ui_text::cluster_label(name, self.mono)
                    .with_color(color)
                    .finish(),
            );
            item_row.add_child(
                Expanded::new(
                    1.0,
                    Container::new(
                        ui_text::cluster_ctrl(short_id, self.mono)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_horizontal_margin(8.0)
                    .finish(),
                )
                .finish(),
            );
            item_row.add_child(
                ui_text::cluster_ctrl("✓", self.mono)
                    .with_color(check_color)
                    .finish(),
            );
            menu.add_child(
                EventHandler::new(
                    Container::new(item_row.finish())
                        .with_horizontal_padding(10.0)
                        .with_vertical_padding(8.0)
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
        if Self::active_cluster_id(cluster).is_some() {
            menu.add_child(Self::cluster_menu_divider());
            if Self::active_cluster_can_invite(cluster) {
                menu.add_child(self.cluster_menu_action(
                    self.copy_invite_label(),
                    DevicesAction::CopyInvite,
                    false,
                    !self.copy_invite_busy,
                    false,
                ));
            } else {
                menu.add_child(self.cluster_menu_action(
                    "复制邀请码（需先确认成员）",
                    DevicesAction::CopyInvite,
                    false,
                    false,
                    false,
                ));
                menu.add_child(self.cluster_menu_action(
                    "重新加入集群",
                    DevicesAction::OpenJoinModal,
                    true,
                    true,
                    false,
                ));
            }
        }
        menu.add_child(Self::cluster_menu_divider());
        menu.add_child(self.cluster_menu_action(
            if self.create_cluster_busy {
                "创建中…"
            } else {
                "创建集群"
            },
            DevicesAction::OpenCreateClusterModal,
            true,
            !self.create_cluster_busy,
            false,
        ));
        menu.add_child(self.cluster_menu_action(
            "加入集群",
            DevicesAction::OpenJoinModal,
            true,
            true,
            false,
        ));
        if Self::active_cluster_is_owner(cluster) && !Self::active_cluster_is_default(cluster) {
            menu.add_child(Self::cluster_menu_divider());
            menu.add_child(self.cluster_menu_action(
                "删除集群",
                DevicesAction::OpenDeleteClusterModal,
                false,
                true,
                true,
            ));
        } else if Self::active_cluster_can_leave(cluster)
            && !Self::active_cluster_is_default(cluster)
        {
            menu.add_child(Self::cluster_menu_divider());
            menu.add_child(self.cluster_menu_action(
                "退出集群",
                DevicesAction::LeaveCluster,
                false,
                true,
                true,
            ));
        }

        Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(CLUSTER_SELECT_MIN_WIDTH)
                .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish()
    }

    fn cluster_picker_overlay(&self, cluster: &ClusterStatusDto) -> Box<dyn Element> {
        let x = SECTION_PADDING;
        let y = SECTION_PADDING + GRID_SECTION_TITLE_HEIGHT + 10.0 + TOOLBAR_BTN_HEIGHT + 6.0;
        positioned_context_menu(x, y, self.cluster_menu_popover(cluster))
    }

    fn cluster_menu(&self, cluster: &ClusterStatusDto) -> Box<dyn Element> {
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
                    .with_min_width(CLUSTER_SELECT_MIN_WIDTH)
                    .with_max_width(320.0)
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

        ConstrainedBox::new(trigger)
            .with_min_width(CLUSTER_SELECT_MIN_WIDTH)
            .with_max_width(320.0)
            .finish()
    }

    fn cluster_toolbar(&self, cluster: &ClusterStatusDto) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        if cluster.clusters.is_empty() {
            row.add_child(self.toolbar_button(
                if self.create_cluster_busy {
                    "创建中…"
                } else {
                    "创建集群"
                },
                DevicesAction::OpenCreateClusterModal,
                true,
                112.0,
                !self.create_cluster_busy,
            ));
            row.add_child(
                Container::new(self.toolbar_button(
                    "加入集群",
                    DevicesAction::OpenJoinModal,
                    true,
                    104.0,
                    true,
                ))
                .with_horizontal_margin(10.0)
                .finish(),
            );
            row.add_child(
                Shrinkable::new(
                    1.0,
                    Container::new(
                        ConstrainedBox::new(
                            ui_text::cluster_status(self.cluster_status_text(cluster), self.mono)
                                .with_color(self.cluster_status_color())
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
            return row.finish();
        }
        row.add_child(self.cluster_menu(cluster));
        row.add_child(
            Container::new(self.cluster_refresh_button())
                .with_horizontal_margin(10.0)
                .finish(),
        );
        if Self::active_cluster_id(cluster).is_some()
            && !Self::active_cluster_can_invite(cluster)
            && !Self::active_cluster_is_owner(cluster)
            && !Self::active_cluster_can_leave(cluster)
        {
            row.add_child(
                Container::new(self.toolbar_button(
                    "重新加入",
                    DevicesAction::OpenJoinModal,
                    true,
                    104.0,
                    true,
                ))
                .with_horizontal_margin(10.0)
                .finish(),
            );
        }
        row.add_child(
            Shrinkable::new(
                1.0,
                Container::new(
                    ConstrainedBox::new(
                        ui_text::cluster_status(self.cluster_status_text(cluster), self.mono)
                            .with_color(self.cluster_status_color())
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
        self.cached_invite = cached_cluster_invite_from_json(&invite).ok();
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
        if let Some(cluster) = self.cluster.as_ref() {
            if !Self::active_cluster_can_invite(cluster) {
                self.status_flash = Some(
                    Self::membership_gate_message(cluster)
                        .unwrap_or("当前设备不是该集群成员，请用原设备操作或重新加入")
                        .to_string(),
                );
                self.cluster_picker_open = false;
                ctx.notify();
                return;
            }
        }
        self.copy_invite_ack = false;
        self.copy_invite_busy = true;
        self.status_flash = Some("正在生成邀请码…".into());
        ctx.notify();

        let core = self.core.clone();
        let cluster_id = self.cluster.as_ref().and_then(Self::active_cluster_id);
        let cached = self.cached_invite.clone();
        ctx.spawn(
            async move {
                let cluster_id = cluster_id.ok_or_else(|| "尚未选择集群".to_string())?;
                let state = core.runtime().state.clone();
                prepare_cluster_invite_for_copy(
                    &state,
                    CreateClusterInviteParams {
                        cluster_id,
                        role: Some("member".to_string()),
                        ttl_secs: None,
                    },
                    cached,
                )
                .await
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
                        view.cached_invite = None;
                        view.status_flash = Some(format!("生成邀请码失败：{e}"));
                        view.join_feedback = Some((StatusTone::Danger, e));
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn open_join_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.create_cluster_modal_open = false;
        self.create_cluster_name_focused = false;
        self.cluster_picker_open = false;
        self.join_modal_open = true;
        self.join_feedback = None;
        if self.join_invite_draft.is_empty() {
            if let Some(text) = read_clipboard_text() {
                self.join_invite_draft = text;
            }
        }
        ctx.notify();
    }

    fn open_create_cluster_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.join_modal_open = false;
        self.cluster_picker_open = false;
        self.create_cluster_modal_open = true;
        self.create_cluster_feedback = None;
        self.create_cluster_name_focused = true;
        if self.create_cluster_name.trim().is_empty() {
            self.create_cluster_name = "Wormhole Cluster".to_string();
        }
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn create_cluster(&mut self, ctx: &mut ViewContext<Self>) {
        if self.create_cluster_busy {
            return;
        }
        let name = self.create_cluster_name.trim().to_string();
        if name.is_empty() {
            self.create_cluster_feedback = Some((StatusTone::Danger, "请输入集群名。".to_string()));
            self.create_cluster_name_focused = true;
            sync_caret_blink(self, ctx);
            ctx.notify();
            return;
        }
        if name.chars().count() > 128 {
            self.create_cluster_feedback = Some((
                StatusTone::Danger,
                "集群名最多 128 个字符，请缩短后再创建。".to_string(),
            ));
            self.create_cluster_name_focused = true;
            sync_caret_blink(self, ctx);
            ctx.notify();
            return;
        }
        self.cluster_picker_open = false;
        self.create_cluster_busy = true;
        self.status_flash = Some("正在创建新集群…".into());
        self.create_cluster_feedback = None;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                tokio::time::timeout(
                    Duration::from_secs(15),
                    create_cluster_command(&state, CreateClusterParams { name: Some(name) }),
                )
                .await
                .map_err(|_| "创建集群超时，请检查登录状态和控制面连接后重试。".to_string())?
            },
            |view, output, ctx| {
                view.create_cluster_busy = false;
                match output {
                    Ok(status) => {
                        view.apply_cluster_status(status, ctx);
                        view.cluster_error = None;
                        view.cached_invite = None;
                        view.create_cluster_modal_open = false;
                        view.create_cluster_name_focused = false;
                        view.create_cluster_name_field.clear_marked();
                        view.status_flash = Some("已创建新集群".into());
                        sync_caret_blink(view, ctx);
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.create_cluster_feedback = Some((StatusTone::Danger, e.clone()));
                        view.status_flash = Some(format!("创建集群失败：{e}"));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn close_create_cluster_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if self.create_cluster_busy {
            return;
        }
        self.create_cluster_modal_open = false;
        self.create_cluster_name_focused = false;
        self.create_cluster_feedback = None;
        self.create_cluster_name_field.clear_marked();
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn focus_create_cluster_name(&mut self, ctx: &mut ViewContext<Self>) {
        self.create_cluster_name_focused = true;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn edit_create_cluster_name(
        &mut self,
        edit: &TextFieldEditAction,
        ctx: &mut ViewContext<Self>,
    ) {
        self.create_cluster_name_field
            .apply(&mut self.create_cluster_name, edit);
        self.create_cluster_name_focused = true;
        self.create_cluster_feedback = None;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn close_join_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.join_modal_open = false;
        self.join_invite_draft.clear();
        self.join_feedback = None;
        ctx.notify();
    }

    fn toggle_cluster_picker(&mut self, ctx: &mut ViewContext<Self>) {
        self.cluster_picker_open = !self.cluster_picker_open;
        ctx.notify();
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
                        view.apply_cluster_status(status, ctx);
                        view.cluster_error = None;
                        view.cached_invite = None;
                    }
                    Err(e) => {
                        view.cluster_error = Some(e);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn leave_active_cluster(&mut self, ctx: &mut ViewContext<Self>) {
        let cluster_id = self.cluster.as_ref().and_then(Self::active_cluster_id);
        let Some(cluster_id) = cluster_id else {
            self.status_flash = Some("尚未选择集群".into());
            ctx.notify();
            return;
        };
        self.cluster_picker_open = false;
        self.apply_optimistic_remove_cluster(&cluster_id, ctx);
        self.status_flash = Some("已退出集群".into());
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                leave_cluster(&state, LeaveClusterParams { cluster_id }).await
            },
            |view, output, ctx| {
                match output {
                    Ok(status) => {
                        view.apply_cluster_status(status, ctx);
                        view.cluster_error = None;
                        view.cached_invite = None;
                        view.status_flash = Some("已退出集群".into());
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.status_flash = Some(format!("退出集群失败：{e}"));
                        view.refresh_cluster(ctx);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_delete_cluster_modal(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(cluster) = self.cluster.as_ref() else {
            return;
        };
        if Self::active_cluster_is_default(cluster) {
            self.status_flash = Some("账号默认集群不可删除".into());
            self.cluster_picker_open = false;
            ctx.notify();
            return;
        }
        if !Self::active_cluster_is_owner(cluster) {
            self.status_flash = Some("只有集群创建者可以删除集群；成员请使用退出集群".into());
            self.cluster_picker_open = false;
            ctx.notify();
            return;
        }
        let cluster_id = Self::active_cluster_id(cluster);
        self.delete_modal_cluster_id = cluster_id;
        self.cluster_picker_open = false;
        ctx.notify();
    }

    fn close_delete_cluster_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.delete_modal_cluster_id = None;
        ctx.notify();
    }

    fn confirm_delete_cluster(&mut self, ctx: &mut ViewContext<Self>) {
        if self.delete_cluster_busy {
            return;
        }
        let cluster_id = self
            .delete_modal_cluster_id
            .clone()
            .or_else(|| self.cluster.as_ref().and_then(Self::active_cluster_id));
        let is_owner = self
            .cluster
            .as_ref()
            .is_some_and(Self::active_cluster_is_owner);
        if !is_owner {
            self.delete_modal_cluster_id = None;
            self.status_flash = Some(
                self.cluster
                    .as_ref()
                    .and_then(Self::membership_gate_message)
                    .unwrap_or("只有集群创建者可以删除集群；成员请使用退出集群")
                    .to_string(),
            );
            ctx.notify();
            return;
        }
        let removed_label = self
            .cluster
            .as_ref()
            .and_then(|cluster| {
                cluster_id.as_ref().and_then(|id| {
                    cluster
                        .clusters
                        .iter()
                        .find(|entry| entry.cluster_id == *id)
                        .map(Self::joined_cluster_label)
                })
            })
            .unwrap_or_else(|| "集群".to_string());
        let Some(cluster_id) = cluster_id else {
            self.status_flash = Some("尚未选择集群".into());
            ctx.notify();
            return;
        };
        self.delete_cluster_busy = true;
        self.delete_modal_cluster_id = None;
        self.cluster_picker_open = false;
        self.apply_optimistic_remove_cluster(&cluster_id, ctx);
        self.status_flash = Some(format!("CLUSTER REMOVED · {removed_label}"));
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                delete_cluster(&state, DeleteClusterParams { cluster_id }).await
            },
            move |view, output, ctx| {
                view.delete_cluster_busy = false;
                match output {
                    Ok(status) => {
                        view.apply_cluster_status(status, ctx);
                        view.cluster_error = None;
                        view.cached_invite = None;
                        view.status_flash = Some(format!("CLUSTER REMOVED · {removed_label}"));
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.status_flash = Some(format!("删除集群失败：{e}"));
                        view.refresh_cluster(ctx);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn remove_cluster_device(
        &mut self,
        device_id: Option<String>,
        node_id: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let cluster_id = self.cluster.as_ref().and_then(Self::active_cluster_id);
        let Some(cluster_id) = cluster_id else {
            self.status_flash = Some("尚未选择集群".into());
            ctx.notify();
            return;
        };
        let removing_server_member = device_id.is_some();
        self.apply_optimistic_remove_node(&node_id, ctx);
        self.status_flash = Some(if removing_server_member {
            "已移除设备".into()
        } else {
            "已隐藏离线终端".into()
        });
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                if device_id.is_some() {
                    remove_cluster_device(
                        &state,
                        RemoveClusterDeviceParams {
                            cluster_id,
                            device_id,
                            node_id: Some(node_id),
                        },
                    )
                    .await
                } else {
                    remove_cluster_node(&state, RemoveClusterNodeParams { node_id }).await
                }
            },
            move |view, output, ctx| {
                match output {
                    Ok(status) => {
                        view.apply_cluster_status(status, ctx);
                        view.cluster_error = None;
                        view.cached_invite = None;
                        view.status_flash = Some(if removing_server_member {
                            "已移除设备".into()
                        } else {
                            "已隐藏离线终端".into()
                        });
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.status_flash = Some(format!("移除设备失败：{e}"));
                        view.refresh_cluster(ctx);
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
                        let status = result.status;
                        let cluster_label = DevicesView::cluster_label(&status);
                        view.apply_cluster_status(status, ctx);
                        view.cluster_error = None;
                        view.join_modal_open = false;
                        view.join_invite_draft.clear();
                        view.join_feedback = None;
                        view.cached_invite = None;
                        view.status_flash = Some(match result.outcome {
                            JoinClusterOutcome::Joined => format!("已加入集群 · {cluster_label}"),
                            JoinClusterOutcome::AlreadyActive => "您已在该集群中".to_string(),
                            JoinClusterOutcome::SwitchedActive => {
                                format!("已切换到集群 · {cluster_label}")
                            }
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

    fn create_cluster_modal(&self) -> Box<dyn Element> {
        let draft = self.create_cluster_name.clone();
        let marked = self.create_cluster_name_field.marked_text.clone();
        let field = render_field_with_caret(
            &draft,
            &marked,
            "例如：家庭集群…",
            self.font,
            self.create_cluster_name_focused,
            false,
            self.caret_blink.visible,
        );
        let input = wrap_text_field_focus_on_click(
            TextFieldInput::builder(field, |ctx, action| {
                ctx.dispatch_typed_action(DevicesAction::CreateClusterNameEdit(action));
            })
            .focused(self.create_cluster_name_focused)
            .ime_preedit(!marked.is_empty())
            .on_keydown(|ctx, keystroke| match keystroke.key.as_str() {
                "enter" | "return" => {
                    ctx.dispatch_typed_action(DevicesAction::CreateCluster);
                    DispatchEventResult::StopPropagation
                }
                "escape" => {
                    ctx.dispatch_typed_action(DevicesAction::CloseCreateClusterModal);
                    DispatchEventResult::StopPropagation
                }
                _ => DispatchEventResult::PropagateToParent,
            })
            .finish(),
            |ctx| ctx.dispatch_typed_action(DevicesAction::FocusCreateClusterName),
        );

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("创建集群", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body("输入一个用于设备列表展示的集群名。", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            ui_text::hud_title("集群名", self.font)
                .with_color(theme::muted())
                .finish(),
        );
        dialog.add_child(
            Container::new(input)
                .with_uniform_padding(10.0)
                .with_vertical_margin(6.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
        );

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.toolbar_button(
            "取消",
            DevicesAction::CloseCreateClusterModal,
            false,
            72.0,
            !self.create_cluster_busy,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        actions.add_child(self.toolbar_button(
            if self.create_cluster_busy {
                "创建中…"
            } else {
                "创建"
            },
            DevicesAction::CreateCluster,
            true,
            80.0,
            !self.create_cluster_busy,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(12.0)
                .finish(),
        );
        if let Some((tone, msg)) = &self.create_cluster_feedback {
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
                ctx.dispatch_typed_action(DevicesAction::CloseCreateClusterModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn join_modal(&self) -> Box<dyn Element> {
        let join_preview = if self.join_invite_draft.is_empty() {
            "粘贴 https://w.hdz73.com/j/... 邀请链接".to_string()
        } else {
            truncate_middle(&self.join_invite_draft, 240)
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("加入集群", self.font)
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
            Container::new(
                ui_text::hud_title("加入集群 · 粘贴邀请码", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ConstrainedBox::new(
                    ui_text::mono(join_preview, self.mono)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_min_height(72.0)
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
                    .with_width(480.0)
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
            } else if cluster.clusters.is_empty() {
                let mut empty = Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max);
                empty.add_child(
                    ui_text::title("暂无集群", self.font)
                        .with_color(theme::text())
                        .finish(),
                );
                empty.add_child(
                    Container::new(
                        ui_text::body("点击上方「创建集群」或「加入集群」开始同步。", self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_vertical_margin(10.0)
                    .finish(),
                );
                col.add_child(
                    Expanded::new(
                        1.0,
                        Container::new(empty.finish())
                            .with_uniform_padding(SECTION_PADDING)
                            .with_background(theme::panel())
                            .finish(),
                    )
                    .finish(),
                );
            } else {
                let local_id = cluster.local_node_id.clone();
                let selected_id = self
                    .selected_node_id
                    .clone()
                    .unwrap_or_else(|| local_id.clone());
                let hovered_id = self.hovered_node_id.clone().unwrap_or_default();
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
                                nodes,
                                local_id,
                                selected_id,
                                hovered_id,
                                hub_index,
                                self.device_remarks.clone(),
                                self.mono,
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
        let has_overlay = self.join_modal_open
            || self.create_cluster_modal_open
            || self.delete_modal_node_id.is_some()
            || self.delete_modal_cluster_id.is_some()
            || self.device_context_menu.is_some()
            || self.cluster_picker_open;
        if !has_overlay {
            return self.grid_view();
        }
        let mut stack = Stack::new();
        let grid = if self.cluster_picker_open
            && !self.join_modal_open
            && !self.create_cluster_modal_open
            && self.delete_modal_node_id.is_none()
            && self.delete_modal_cluster_id.is_none()
            && self.device_context_menu.is_none()
        {
            EventHandler::new(self.grid_view())
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::CloseClusterPicker);
                    DispatchEventResult::PropagateToParent
                })
                .finish()
        } else {
            self.grid_view()
        };
        stack.add_child(grid);
        if self.cluster_picker_open {
            if let Some(cluster) = &self.cluster {
                if !cluster.auth_required && !cluster.device_bootstrap_required {
                    stack.add_child(self.cluster_picker_overlay(cluster));
                }
            }
        }
        if self.create_cluster_modal_open {
            stack.add_child(self.create_cluster_modal());
        }
        if self.device_context_menu.is_some() {
            let scrim = EventHandler::new(
                Container::new(Flex::column().finish())
                    .with_background(ColorU::new(8, 7, 11, 40))
                    .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseDeviceContextMenu);
                DispatchEventResult::StopPropagation
            })
            .finish();
            stack.add_child(scrim);
            stack.add_child(self.device_context_menu());
        }
        if self.delete_modal_node_id.is_some() {
            stack.add_child(self.delete_node_modal());
        }
        if self.delete_modal_cluster_id.is_some() {
            stack.add_child(self.delete_cluster_modal());
        }
        if self.join_modal_open {
            stack.add_child(self.join_modal());
        }
        let delete_node_modal_open = self.delete_modal_node_id.is_some();
        let delete_cluster_modal_open = self.delete_modal_cluster_id.is_some();
        let device_menu_open = self.device_context_menu.is_some();
        let join_modal_open = self.join_modal_open;
        let create_cluster_modal_open = self.create_cluster_modal_open;
        let cluster_picker_open = self.cluster_picker_open;
        EventHandler::new(stack.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                if device_menu_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseDeviceContextMenu);
                    return DispatchEventResult::StopPropagation;
                }
                DispatchEventResult::PropagateToParent
            })
            .on_keydown(move |ctx, _, keystroke| {
                if keystroke.key.as_str() != "escape" {
                    return DispatchEventResult::PropagateToParent;
                }
                if delete_cluster_modal_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseDeleteClusterModal);
                } else if delete_node_modal_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseDeleteNodeModal);
                } else if device_menu_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseDeviceContextMenu);
                } else if join_modal_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseJoinModal);
                } else if create_cluster_modal_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseCreateClusterModal);
                } else if cluster_picker_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseClusterPicker);
                }
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn delete_cluster_modal(&self) -> Box<dyn Element> {
        let target = self
            .delete_modal_cluster_id
            .as_deref()
            .and_then(|id| {
                self.cluster.as_ref().and_then(|cluster| {
                    cluster
                        .clusters
                        .iter()
                        .find(|entry| entry.cluster_id == id)
                        .map(|entry| {
                            format!(
                                "{} · {}",
                                Self::joined_cluster_label(entry),
                                short_cluster_id(id)
                            )
                        })
                })
            })
            .unwrap_or_else(|| "—".to_string());
        let is_owner = self
            .cluster
            .as_ref()
            .and_then(|cluster| {
                self.delete_modal_cluster_id.as_ref().and_then(|id| {
                    cluster
                        .clusters
                        .iter()
                        .find(|entry| entry.cluster_id == *id)
                        .map(|entry| entry.role == "owner")
                })
            })
            .unwrap_or(false);
        let body = if is_owner {
            "将从本机移除该集群及其全部终端、共享文件夹与同步索引；控制面上的集群记录将一并销毁。此操作不可撤销。"
        } else {
            "将从本机移除该集群及其全部终端、共享文件夹与同步索引。此操作不可撤销。"
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("删除集群", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(body, self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::mono(target, self.mono)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        );

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.toolbar_button(
            "取消",
            DevicesAction::CloseDeleteClusterModal,
            false,
            72.0,
            !self.delete_cluster_busy,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        actions.add_child(self.toolbar_button(
            if self.delete_cluster_busy {
                "删除中…"
            } else {
                "删除"
            },
            DevicesAction::ConfirmDeleteCluster,
            true,
            80.0,
            !self.delete_cluster_busy,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_margin_top(16.0)
                .finish(),
        );

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(420.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel_elevated())
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
                ctx.dispatch_typed_action(DevicesAction::CloseDeleteClusterModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn delete_node_modal(&self) -> Box<dyn Element> {
        let target = self
            .delete_modal_node_id
            .as_deref()
            .and_then(|id| self.cluster_node_label(id))
            .map(|node| format!("{} · {}", node.os, node.node_id))
            .unwrap_or_else(|| "—".to_string());

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("删除终端", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    "将从当前集群移除该终端。其共享文件夹与同步状态将不再可见，此操作不可撤销。",
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::mono(target, self.mono)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        );

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.toolbar_button(
            "取消",
            DevicesAction::CloseDeleteNodeModal,
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
            "删除",
            DevicesAction::ConfirmDeleteNode,
            true,
            80.0,
            true,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_margin_top(16.0)
                .finish(),
        );

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(420.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel_elevated())
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
                ctx.dispatch_typed_action(DevicesAction::CloseDeleteNodeModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn device_context_menu(&self) -> Box<dyn Element> {
        let (node_id, x, y) = self.device_context_menu.clone().unwrap_or_default();
        let is_local = self
            .cluster
            .as_ref()
            .is_some_and(|cluster| cluster.local_node_id == node_id);

        let mut menu = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        menu.add_child(self.device_context_item(
            "浏览共享文件夹",
            DevicesAction::OpenNode(node_id.clone()),
            ContextItemStyle::Normal,
            true,
        ));
        let remote_available = self.cluster.as_ref().is_some_and(|cluster| {
            cluster
                .nodes
                .iter()
                .find(|node| node.node_id == node_id)
                .is_some_and(|node| node_remote_desktop_available(node, is_local))
        });
        if !is_local {
            menu.add_child(self.device_context_item(
                "远程桌面",
                DevicesAction::OpenRemoteDesktop(node_id.clone()),
                ContextItemStyle::Normal,
                remote_available,
            ));
        }
        menu.add_child(self.device_context_item(
            if is_local {
                "无法给本机发信息"
            } else {
                "发信息"
            },
            DevicesAction::SendMessage(node_id.clone()),
            ContextItemStyle::Accent,
            !is_local,
        ));
        menu.add_child(Self::cluster_menu_divider());
        menu.add_child(self.device_context_item(
            if is_local {
                "无法删除本机"
            } else {
                "删除终端"
            },
            DevicesAction::OpenDeleteNodeModal(node_id),
            ContextItemStyle::Danger,
            !is_local,
        ));

        let panel = Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(168.0)
                .finish(),
        )
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();

        positioned_context_menu(x, y, panel)
    }

    fn device_context_item(
        &self,
        label: &str,
        action: DevicesAction,
        style: ContextItemStyle,
        enabled: bool,
    ) -> Box<dyn Element> {
        let color = if !enabled {
            theme::muted()
        } else {
            match style {
                ContextItemStyle::Normal => theme::text(),
                ContextItemStyle::Accent => theme::accent_cool(),
                ContextItemStyle::Danger => theme::danger(),
            }
        };
        let handler = EventHandler::new(
            Container::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(color)
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .finish(),
        );
        if enabled {
            handler
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            handler.finish()
        }
    }

    fn share_status_line(&self) -> String {
        if let Some(msg) = &self.share_status {
            return msg.clone();
        }
        if self.share_loading {
            return "VAULT · 正在读取…".to_string();
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
        if entry.local && entry.volume_id.is_some() {
            name_row.add_child(
                Container::new(
                    ui_text::cluster_ctrl("共享".to_string(), self.mono)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_margin_left(8.0)
                .with_padding_left(6.0)
                .with_padding_right(6.0)
                .with_padding_top(2.0)
                .with_padding_bottom(2.0)
                .with_background(theme::accent_cool_bg(28))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
                .finish(),
            );
        }

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

        let selected =
            !is_folder && self.selected_share_file.as_deref() == Some(entry.name.as_str());
        let inner = Container::new(row.finish())
            .with_background(if selected {
                theme::accent_cool_bg(18)
            } else {
                theme::panel()
            })
            .with_border(Border::all(1.0).with_border_fill(if selected {
                theme::accent_cool()
            } else {
                theme::border()
            }))
            .finish();

        let menu_name = name.clone();
        if is_folder {
            EventHandler::new(inner)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::ShareNavigate {
                        volume_id: volume_id.clone(),
                        name: name.clone(),
                    });
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
        } else {
            let click_name = name.clone();
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

    fn share_empty_hint(&self) -> &'static str {
        if self.share_loading {
            "正在读取共享文件夹…"
        } else if self.browsing_local() && self.share_path.is_empty() {
            "本机尚未添加共享文件夹。点击「增加共享文件夹」添加，或返回打开其它终端卡片浏览远端共享。"
        } else if !self.browsing_local() && self.share_path.is_empty() {
            "此终端尚未发布共享文件夹，或名单未同步。请在对端添加共享后刷新。"
        } else {
            "此文件夹为空"
        }
    }

    fn workspace_panel(&self) -> Box<dyn Element> {
        let mut panel = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        panel.add_child(section_title("在来源电脑打开", self.font));
        panel.add_child(section_hint(
            "应用在来源电脑的隔离运行器中执行，不控制来源桌面。",
            self.font,
        ));

        let Some(entry) = self.selected_share_entry() else {
            panel.add_child(status_line(
                "选择一个远端文件",
                self.font,
                StatusTone::Placeholder,
            ));
            return Container::new(
                ConstrainedBox::new(panel.finish())
                    .with_width(340.0)
                    .with_min_width(300.0)
                    .finish(),
            )
            .with_uniform_padding(14.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish();
        };

        panel.add_child(
            ui_text::body(entry.name.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        panel.add_child(
            ui_text::mono(format_size(entry.size), self.mono)
                .with_color(theme::muted())
                .finish(),
        );
        panel.add_child(section_title("来源电脑 Ubuntu 工具运行器", self.font));
        panel.add_child(section_hint(
            "文件不上传云端；客户端仅接收隔离运行器的画面和输入。",
            self.font,
        ));
        panel.add_child(section_title("来源", self.font));
        let source = entry
            .version_author
            .as_deref()
            .map(|author| truncate_middle(author, 34))
            .unwrap_or_else(|| "未解析".into());
        panel.add_child(
            ui_text::mono(source, self.mono)
                .with_color(theme::muted())
                .finish(),
        );
        panel.add_child(section_title("打开方式", self.font));
        if self.workspace_selected_app == "onlyoffice" {
            panel.add_child(status_line(
                workspace_app_label(&self.workspace_selected_app),
                self.font,
                StatusTone::Neutral,
            ));
        } else {
            panel.add_child(self.toolbar_button(
                workspace_app_label(&self.workspace_selected_app),
                DevicesAction::WorkspaceNextApp,
                false,
                156.0,
                true,
            ));
        }

        let tone = match self.workspace_remote_job.as_ref().map(|job| &job.stage) {
            Some(WorkspaceProvisionStage::Failed) => StatusTone::Danger,
            Some(WorkspaceProvisionStage::RebootRequired) => StatusTone::Warn,
            Some(WorkspaceProvisionStage::Ready) => StatusTone::Success,
            _ => StatusTone::Neutral,
        };
        panel.add_child(section_title("环境", self.font));
        panel.add_child(status_line(self.workspace_status.clone(), self.font, tone));
        if let Some(job) = &self.workspace_remote_job {
            if job.bytes_total > 0 && job.bytes_downloaded > 0 {
                let percent = job.bytes_downloaded.saturating_mul(100) / job.bytes_total;
                panel.add_child(
                    ui_text::mono(
                        format!(
                            "{}% · {} / {}",
                            percent,
                            format_size(job.bytes_downloaded),
                            format_size(job.bytes_total)
                        ),
                        self.mono,
                    )
                    .with_color(theme::accent_cool())
                    .finish(),
                );
            }
        }

        let can_open = entry.version_author.as_deref().is_some_and(|author| {
            self.workspace_worker_for_author(author)
                .is_some_and(|worker| {
                    worker.available
                        && worker.vm_ready
                        && workspace_worker_supports_app(worker, &self.workspace_selected_app)
                })
        });
        panel.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(8.0)
                .finish(),
        );
        if can_open {
            panel.add_child(self.toolbar_button(
                if self.workspace_selected_app == "onlyoffice" {
                    "用 ONLYOFFICE 打开"
                } else {
                    "在来源电脑打开"
                },
                DevicesAction::WorkspaceOpenSelected,
                false,
                180.0,
                !self.share_file_busy,
            ));
        } else if self
            .workspace_remote_job
            .as_ref()
            .is_some_and(|job| job.stage == WorkspaceProvisionStage::Failed)
        {
            panel.add_child(self.toolbar_button(
                "重新请求安装",
                DevicesAction::WorkspaceRetryProvision,
                false,
                180.0,
                !self.workspace_loading,
            ));
        } else if self.workspace_remote_job.is_none() {
            panel.add_child(self.toolbar_button(
                "请求来源电脑安装",
                DevicesAction::WorkspaceRequestProvision,
                false,
                180.0,
                entry.entry_id.is_some()
                    && entry.version_author.is_some()
                    && !self.workspace_loading,
            ));
        } else {
            panel.add_child(self.toolbar_button(
                "刷新安装状态",
                DevicesAction::WorkspaceRefresh,
                false,
                180.0,
                !self.workspace_loading,
            ));
        }

        Container::new(
            ConstrainedBox::new(panel.finish())
                .with_width(340.0)
                .with_min_width(300.0)
                .finish(),
        )
        .with_uniform_padding(14.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
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
        if self.can_show_share_new() {
            toolbar.add_child(
                Container::new(self.toolbar_button(
                    "新建",
                    DevicesAction::ToggleShareNewMenu,
                    false,
                    72.0,
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
                    ui_text::body(self.share_empty_hint(), self.font)
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

        let file_list = Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish();
        if self.browsing_local() {
            return file_list;
        }
        let mut split = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        split.add_child(Expanded::new(1.0, file_list).finish());
        split.add_child(
            Container::new(self.workspace_panel())
                .with_margin_left(8.0)
                .finish(),
        );
        split.finish()
    }

    fn share_add_modal(&self) -> Box<dyn Element> {
        let path_preview = if self.share_add_path.is_empty() {
            format!("例如：{}", default_share_browse_path())
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
                    "选择本机目录并发布到当前终端的集群共享空间。显示名称将自动取自路径末段。",
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
        let browsing_local = self.browsing_local();
        let can_manage = self.browsing_can_manage();
        let has_local_replica = entry.map(|entry| entry.local).unwrap_or(false);
        let can_unshare = can_manage
            && entry
                .map(|entry| entry.local && entry.volume_id.is_some())
                .unwrap_or(false);
        let is_folder = entry
            .map(|entry| entry.kind.eq_ignore_ascii_case("folder"))
            .unwrap_or(false);
        let needs_sync = !browsing_local && !is_folder && !has_local_replica;
        let can_remote = !browsing_local && !is_folder;
        let can_rename = can_manage;
        let can_delete = if can_manage {
            true
        } else {
            !is_folder && has_local_replica
        };

        let open_label = if is_folder {
            "打开文件夹"
        } else {
            "打开"
        };
        let open_action = if is_folder {
            DevicesAction::ShareNavigate {
                volume_id: entry.and_then(|e| e.volume_id.clone()),
                name: name.clone(),
            }
        } else {
            DevicesAction::ShareOpenFile(name.clone())
        };

        let mut menu = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        menu.add_child(self.share_context_item(open_label, Some(open_action), false, true));
        menu.add_child(self.share_context_item(
            "同步",
            Some(DevicesAction::ShareSyncFile(name.clone())),
            false,
            needs_sync,
        ));
        menu.add_child(self.share_context_item(
            "在 Windows 打开…",
            Some(DevicesAction::ShareRemoteOpenFile(name.clone())),
            false,
            can_remote,
        ));
        menu.add_child(self.share_context_item(
            "重命名",
            Some(DevicesAction::OpenShareRenameModal(name.clone())),
            false,
            can_rename,
        ));
        menu.add_child(self.share_context_item(
            "删除",
            Some(DevicesAction::ShareDeleteFile(name.clone())),
            true,
            can_delete,
        ));
        menu.add_child(self.share_context_item(
            "取消共享",
            Some(DevicesAction::OpenShareUnshareModal(name)),
            true,
            can_unshare,
        ));

        let panel = Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(168.0)
                .finish(),
        )
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();

        positioned_context_menu(x, y, panel)
    }

    fn share_unshare_modal(&self) -> Box<dyn Element> {
        let name = self
            .share_unshare_name
            .clone()
            .unwrap_or_else(|| "—".into());
        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("取消共享", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    "将从当前终端的集群共享中移除此项。其他终端将无法再浏览或同步其中的内容。",
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    "本机路径中的原始文件不会被删除，仅取消共享发布。同级其他共享项不受影响。",
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_margin_bottom(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::mono(name, self.mono)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        );

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.toolbar_button(
            "返回",
            DevicesAction::CloseShareUnshareModal,
            false,
            72.0,
            !self.share_unshare_busy,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        actions.add_child(self.toolbar_button(
            if self.share_unshare_busy {
                "处理中…"
            } else {
                "取消共享"
            },
            DevicesAction::ConfirmShareUnshare,
            true,
            96.0,
            !self.share_unshare_busy,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_margin_top(16.0)
                .finish(),
        );

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(440.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel_elevated())
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
                ctx.dispatch_typed_action(DevicesAction::CloseShareUnshareModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn open_share_unshare_modal(&mut self, name: String, ctx: &mut ViewContext<Self>) {
        self.close_share_context_menu(ctx);
        let entry = self.share_entries.iter().find(|entry| entry.name == name);
        let Some(volume_id) = entry.and_then(|entry| entry.volume_id.clone()) else {
            return;
        };
        if !self.browsing_can_manage() {
            return;
        }
        self.share_unshare_volume_id = Some(volume_id);
        self.share_unshare_name = Some(name);
        self.share_unshare_busy = false;
        ctx.notify();
    }

    fn close_share_unshare_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.share_unshare_volume_id = None;
        self.share_unshare_name = None;
        self.share_unshare_busy = false;
        ctx.notify();
    }

    fn confirm_share_unshare(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(volume_id) = self.share_unshare_volume_id.clone() else {
            return;
        };
        if self.share_unshare_busy {
            return;
        }
        let name = self.share_unshare_name.clone().unwrap_or_default();
        let Some(node_id) = self.browsing_node_id.clone() else {
            return;
        };
        self.share_unshare_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                remove_storage_volume_from_node(&state, &node_id, volume_id).await
            },
            move |view, output, ctx| {
                view.share_unshare_busy = false;
                match output {
                    Ok(_) => {
                        view.close_share_unshare_modal(ctx);
                        view.share_status = Some(format!("已取消共享 · {name} · 本机文件保留"));
                        view.load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_error = Some(format!("取消共享失败: {e}"));
                        view.close_share_unshare_modal(ctx);
                    }
                }
                ctx.notify();
            },
        );
    }

    fn share_context_item(
        &self,
        label: &str,
        action: Option<DevicesAction>,
        danger: bool,
        enabled: bool,
    ) -> Box<dyn Element> {
        let color = if !enabled {
            dim_color(theme::muted(), 0.7)
        } else if danger {
            theme::danger()
        } else {
            theme::text()
        };
        let inner = Container::new(
            ui_text::body(label.to_string(), self.font)
                .with_color(color)
                .finish(),
        )
        .with_uniform_padding(10.0)
        .finish();
        if enabled {
            if let Some(action) = action {
                EventHandler::new(inner)
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(action.clone());
                        DispatchEventResult::StopPropagation
                    })
                    .finish()
            } else {
                inner
            }
        } else {
            inner
        }
    }

    fn share_new_menu_item(
        &self,
        label: &str,
        is_folder: bool,
        action: DevicesAction,
    ) -> Box<dyn Element> {
        let icon_el = icons::share_file_icon(if is_folder { "folder" } else { "a.txt" }, is_folder);
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(Container::new(icon_el).with_horizontal_margin(2.0).finish());
        row.add_child(
            Container::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_horizontal_margin(8.0)
            .finish(),
        );
        EventHandler::new(
            Container::new(row.finish())
                .with_uniform_padding(10.0)
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn share_new_menu(&self) -> Box<dyn Element> {
        let mut menu = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        match self.share_new_menu_mode() {
            ShareNewMenuMode::AddSharedFolder => {
                menu.add_child(self.share_new_menu_item(
                    "共享文件夹",
                    true,
                    DevicesAction::OpenShareAddModal,
                ));
            }
            ShareNewMenuMode::CreateEntries => {
                menu.add_child(self.share_new_menu_item(
                    "文件夹",
                    true,
                    DevicesAction::ShareCreateFolder,
                ));
                menu.add_child(self.share_new_menu_item(
                    "txt 文件",
                    false,
                    DevicesAction::ShareCreateTxt,
                ));
            }
            ShareNewMenuMode::Hidden => {}
        }
        let panel = Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(148.0)
                .finish(),
        )
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();
        let panel = EventHandler::new(panel)
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish();
        Align::new(
            Container::new(panel)
                .with_margin_top(52.0)
                .with_margin_right(16.0)
                .finish(),
        )
        .top_right()
        .finish()
    }

    fn share_rename_modal(&self) -> Box<dyn Element> {
        let draft = self.share_rename_draft.clone();
        let marked = self.share_rename_field.marked_text.clone();
        let field = render_field_with_caret(
            &draft,
            &marked,
            "新名称",
            self.font,
            self.share_rename_focused,
            false,
            self.caret_blink.visible,
        );
        let input = wrap_text_field_focus_on_click(
            TextFieldInput::builder(field, |ctx, action| {
                ctx.dispatch_typed_action(DevicesAction::ShareRenameEdit(action));
            })
            .focused(self.share_rename_focused)
            .ime_preedit(!marked.is_empty())
            .on_keydown(|ctx, keystroke| match keystroke.key.as_str() {
                "enter" | "return" => {
                    ctx.dispatch_typed_action(DevicesAction::ConfirmShareRename);
                    DispatchEventResult::StopPropagation
                }
                "escape" => {
                    ctx.dispatch_typed_action(DevicesAction::CloseShareRenameModal);
                    DispatchEventResult::StopPropagation
                }
                _ => DispatchEventResult::PropagateToParent,
            })
            .finish(),
            |ctx| ctx.dispatch_typed_action(DevicesAction::FocusShareRenameField),
        );

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title("重命名", self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(input)
                .with_vertical_margin(12.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .with_uniform_padding(8.0)
                .finish(),
        );
        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        actions.add_child(Expanded::new(1.0, Flex::column().finish()).finish());
        actions.add_child(self.toolbar_button(
            "取消",
            DevicesAction::CloseShareRenameModal,
            false,
            88.0,
            true,
        ));
        actions.add_child(
            Container::new(self.toolbar_button(
                if self.share_rename_busy {
                    "保存中…"
                } else {
                    "确定"
                },
                DevicesAction::ConfirmShareRename,
                true,
                88.0,
                !self.share_rename_busy,
            ))
            .with_horizontal_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(8.0)
                .finish(),
        );
        if let Some((tone, msg)) = &self.share_rename_feedback {
            dialog.add_child(status_line(msg.clone(), self.font, *tone));
        }

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(400.0)
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
                ctx.dispatch_typed_action(DevicesAction::CloseShareRenameModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn files_shell(&self) -> Box<dyn Element> {
        let has_overlay = self.share_add_modal_open
            || self.share_context_entry.is_some()
            || self.share_unshare_volume_id.is_some()
            || self.share_new_menu_open
            || self.share_rename_modal_open;
        if !has_overlay {
            return self.files_view();
        }
        let mut stack = Stack::new();
        stack.add_child(self.files_view());
        if self.share_add_modal_open {
            stack.add_child(self.share_add_modal());
        }
        if self.share_rename_modal_open {
            stack.add_child(self.share_rename_modal());
        }
        if self.share_new_menu_open {
            let scrim = EventHandler::new(
                Container::new(Flex::column().finish())
                    .with_background(ColorU::new(8, 7, 11, 1))
                    .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseShareNewMenu);
                DispatchEventResult::StopPropagation
            })
            .finish();
            stack.add_child(scrim);
            stack.add_child(self.share_new_menu());
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
        if self.share_unshare_volume_id.is_some() {
            stack.add_child(self.share_unshare_modal());
        }
        let share_menu_open = self.share_context_entry.is_some() || self.share_new_menu_open;
        let body = stack.finish();
        if share_menu_open {
            EventHandler::new(body)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(DevicesAction::CloseShareContextMenu);
                    ctx.dispatch_typed_action(DevicesAction::CloseShareNewMenu);
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            body
        }
    }

    fn pending_workspace_approval(&self) -> Option<&WorkspaceProvisionJob> {
        self.workspace_jobs.iter().find(|job| {
            (job.stage == WorkspaceProvisionStage::AwaitingLocalApproval
                || (job.stage == WorkspaceProvisionStage::Failed && job.import_candidate.is_some()))
                && self.workspace_dismissed_approval.as_deref() != Some(job.job_id.as_str())
        })
    }

    fn workspace_approval_modal(&self, job: &WorkspaceProvisionJob) -> Box<dyn Element> {
        let requester = job
            .request
            .requested_by_node_id
            .as_deref()
            .map(|value| truncate_middle(value, 36))
            .unwrap_or_else(|| "集群成员".into());
        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let failed_candidate =
            job.stage == WorkspaceProvisionStage::Failed && job.import_candidate.is_some();
        dialog.add_child(
            ui_text::title(
                if failed_candidate {
                    "已有虚拟机无法安全克隆"
                } else {
                    "允许安装远程虚拟机？"
                },
                self.font,
            )
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    format!(
                        "{requester} 请求在这台来源电脑安装 {}，用于打开同步文件。",
                        job.manifest.name
                    ),
                    self.font,
                )
                .with_color(theme::text())
                .finish(),
            )
            .with_margin_top(12.0)
            .finish(),
        );
        dialog.add_child(status_line(
            format!(
                "镜像 {} · {} · {}",
                job.manifest.version,
                job.manifest.installed_apps.join(", "),
                format_size(job.manifest.size_bytes)
            ),
            self.font,
            StatusTone::Neutral,
        ));
        if failed_candidate {
            dialog.add_child(status_line(
                job.last_error.clone().unwrap_or_else(|| job.detail.clone()),
                self.font,
                StatusTone::Danger,
            ));
        }
        let compatible_candidates = self
            .workspace_vm_candidates
            .iter()
            .filter(|candidate| candidate.importable)
            .count();
        dialog.add_child(status_line(
            if failed_candidate {
                "原 VM 保持不变。可以显式改用组织私有签名镜像。".into()
            } else if compatible_candidates > 0 {
                format!(
                    "已检测到 {compatible_candidates} 个关闭的 Hyper-V VHDX 候选；将只读验收并克隆，不修改原 VM。"
                )
            } else {
                "未发现可安全克隆的已关闭 Hyper-V VM，将使用组织私有镜像。".into()
            },
            self.font,
            StatusTone::Muted,
        ));
        dialog.add_child(status_line(
            "批准后将触发本机 UAC，可能启用 Hyper-V 并要求重启。镜像及应用许可由组织负责。",
            self.font,
            StatusTone::Warn,
        ));
        if !failed_candidate {
            for candidate in self
                .workspace_vm_candidates
                .iter()
                .filter(|candidate| candidate.importable)
                .take(3)
            {
                dialog.add_child(
                    Container::new(self.toolbar_button(
                        &format!("克隆 {}", truncate_middle(&candidate.vm_name, 28)),
                        DevicesAction::WorkspaceApproveProvisionCandidate {
                            job_id: job.job_id.clone(),
                            vm_name: candidate.vm_name.clone(),
                        },
                        false,
                        260.0,
                        !self.workspace_loading,
                    ))
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
            if compatible_candidates > 3 {
                dialog.add_child(status_line(
                    format!(
                        "另有 {} 个候选未显示，请整理 VM 后刷新。",
                        compatible_candidates - 3
                    ),
                    self.font,
                    StatusTone::Muted,
                ));
            }
        }
        let mut actions = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        actions.add_child(self.toolbar_button(
            "稍后",
            DevicesAction::WorkspaceDismissApproval(job.job_id.clone()),
            false,
            72.0,
            true,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(6.0)
                .finish(),
        );
        actions.add_child(self.toolbar_button(
            if failed_candidate {
                "改用组织镜像"
            } else {
                "下载组织镜像"
            },
            DevicesAction::WorkspaceApproveProvision(job.job_id.clone()),
            true,
            136.0,
            !self.workspace_loading,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_margin_top(16.0)
                .finish(),
        );
        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(460.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        )
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();
        Container::new(Align::new(panel).finish())
            .with_uniform_padding(24.0)
            .with_background(ColorU::new(8, 7, 11, 190))
            .finish()
    }
}

fn positioned_context_menu(x: f32, y: f32, panel: Box<dyn Element>) -> Box<dyn Element> {
    let panel = EventHandler::new(panel)
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();

    Align::new(
        Container::new(panel)
            .with_margin_left(x.max(8.0))
            .with_margin_top(y.max(8.0))
            .finish(),
    )
    .top_left()
    .finish()
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

fn workspace_apps_for_file(name: &str) -> &'static [&'static str] {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" => &["paint", "default"],
        "doc" | "docx" | "rtf" | "xls" | "xlsx" | "csv" | "ppt" | "pptx" => &["onlyoffice"],
        "txt" | "md" | "log" => &["notepad", "default"],
        _ => &["default"],
    }
}

fn recommended_workspace_app(name: &str) -> &'static str {
    workspace_apps_for_file(name)[0]
}

fn workspace_app_label(app: &str) -> &'static str {
    match app {
        "paint" => "Paint",
        "word" => "Microsoft Word",
        "excel" => "Microsoft Excel",
        "powerpoint" => "Microsoft PowerPoint",
        "notepad" => "Notepad",
        "onlyoffice" => "ONLYOFFICE Desktop Editors",
        _ => "虚拟机默认应用",
    }
}

fn workspace_worker_supports_app(worker: &WorkspaceWorker, app: &str) -> bool {
    worker.images.iter().any(|image| {
        if app.eq_ignore_ascii_case("default") {
            !image.installed_apps.is_empty()
        } else {
            image
                .installed_apps
                .iter()
                .any(|installed| installed.eq_ignore_ascii_case(app))
        }
    })
}

fn default_share_browse_path() -> String {
    #[cfg(windows)]
    {
        "D:\\Projects\\Share".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "/Users/Shared/Wormhole".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("HOME")
            .map(|home| format!("{home}/Wormhole"))
            .unwrap_or_else(|_| "/home".to_string())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "/Wormhole".to_string()
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

impl CaretBlinkHost for DevicesView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.create_cluster_modal_open && self.create_cluster_name_focused
    }
}

impl Entity for DevicesView {
    type Event = DevicesEvent;
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
        if let Some(job) = self.pending_workspace_approval() {
            let mut stack = Stack::new();
            stack.add_child(body);
            stack.add_child(self.workspace_approval_modal(job));
            tab_content_fill(stack.finish())
        } else {
            tab_content_fill(body)
        }
    }
}

impl TypedActionView for DevicesView {
    type Action = DevicesAction;

    fn handle_action(&mut self, action: &DevicesAction, ctx: &mut ViewContext<Self>) {
        match action {
            DevicesAction::Refresh => self.manual_refresh_cluster(ctx),
            DevicesAction::RetryDeviceBootstrap => self.retry_device_bootstrap(ctx),
            DevicesAction::OpenNode(node_id) => self.open_node(node_id.clone(), ctx),
            DevicesAction::BackToGrid => self.back_to_grid(ctx),
            DevicesAction::CopyInvite => {
                self.close_cluster_picker(ctx);
                self.copy_invite(ctx);
            }
            DevicesAction::OpenJoinModal => {
                self.close_cluster_picker(ctx);
                self.open_join_modal(ctx);
            }
            DevicesAction::OpenCreateClusterModal => self.open_create_cluster_modal(ctx),
            DevicesAction::CreateCluster => self.create_cluster(ctx),
            DevicesAction::CreateClusterNameEdit(edit) => self.edit_create_cluster_name(edit, ctx),
            DevicesAction::FocusCreateClusterName => self.focus_create_cluster_name(ctx),
            DevicesAction::CloseCreateClusterModal => self.close_create_cluster_modal(ctx),
            DevicesAction::CloseJoinModal => self.close_join_modal(ctx),
            DevicesAction::PasteJoinInvite => self.paste_join_invite(ctx),
            DevicesAction::SubmitJoin => {
                if !self.invite_busy {
                    self.submit_join(ctx);
                }
            }
            DevicesAction::ToggleClusterPicker => self.toggle_cluster_picker(ctx),
            DevicesAction::CloseClusterPicker => self.close_cluster_picker(ctx),
            DevicesAction::SelectCluster(cluster_id) => {
                self.select_cluster(cluster_id.clone(), ctx);
            }
            DevicesAction::LeaveCluster => self.leave_active_cluster(ctx),
            DevicesAction::OpenDeleteClusterModal => self.open_delete_cluster_modal(ctx),
            DevicesAction::CloseDeleteClusterModal => self.close_delete_cluster_modal(ctx),
            DevicesAction::ConfirmDeleteCluster => self.confirm_delete_cluster(ctx),
            DevicesAction::RemoveClusterDevice { device_id, node_id } => {
                self.remove_cluster_device(device_id.clone(), node_id.clone(), ctx);
            }
            DevicesAction::ShareBack => self.share_back(ctx),
            DevicesAction::ShareForward => self.share_forward(ctx),
            DevicesAction::ShareNavigate { volume_id, name } => {
                self.close_share_context_menu(ctx);
                self.close_share_new_menu(ctx);
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
            DevicesAction::ToggleShareNewMenu => self.toggle_share_new_menu(ctx),
            DevicesAction::CloseShareNewMenu => self.close_share_new_menu(ctx),
            DevicesAction::ShareCreateFolder => {
                self.create_share_item(CreateShareEntryKind::Folder, ctx);
            }
            DevicesAction::ShareCreateTxt => {
                self.create_share_item(CreateShareEntryKind::Txt, ctx);
            }
            DevicesAction::ShareOpenFile(name) => self.open_share_file(name.clone(), ctx),
            DevicesAction::ShareSyncFile(name) => self.sync_share_file(name.clone(), ctx),
            DevicesAction::ShareRemoteOpenFile(name) => {
                self.remote_open_share_file(name.clone(), ctx);
            }
            DevicesAction::ShareDeleteFile(name) => self.delete_share_file(name.clone(), ctx),
            DevicesAction::OpenShareRenameModal(name) => {
                self.open_share_rename_modal(name.clone(), ctx);
            }
            DevicesAction::CloseShareRenameModal => self.close_share_rename_modal(ctx),
            DevicesAction::ShareRenameEdit(edit) => self.edit_share_rename_field(edit, ctx),
            DevicesAction::FocusShareRenameField => {
                self.share_rename_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            DevicesAction::ConfirmShareRename => self.confirm_share_rename(ctx),
            DevicesAction::OpenShareUnshareModal(name) => {
                self.open_share_unshare_modal(name.clone(), ctx);
            }
            DevicesAction::CloseShareUnshareModal => self.close_share_unshare_modal(ctx),
            DevicesAction::ConfirmShareUnshare => self.confirm_share_unshare(ctx),
            DevicesAction::OpenShareContextMenu { name, x, y } => {
                self.share_new_menu_open = false;
                self.share_context_entry = Some(name.clone());
                self.share_context_pos = Some((*x, *y));
                ctx.notify();
            }
            DevicesAction::CloseShareContextMenu => self.close_share_context_menu(ctx),
            DevicesAction::ShareFileClick(name) => self.handle_share_file_click(name.clone(), ctx),
            DevicesAction::WorkspaceRefresh => self.refresh_workspace(ctx),
            DevicesAction::WorkspaceNextApp => self.workspace_next_app(ctx),
            DevicesAction::WorkspaceOpenSelected => self.workspace_open_selected(ctx),
            DevicesAction::WorkspaceRequestProvision => self.workspace_request_provision(ctx),
            DevicesAction::WorkspaceRetryProvision => self.workspace_retry_provision(ctx),
            DevicesAction::WorkspaceApproveProvision(job_id) => {
                self.workspace_approve_job(job_id.clone(), None, ctx);
            }
            DevicesAction::WorkspaceApproveProvisionCandidate { job_id, vm_name } => {
                self.workspace_approve_job(job_id.clone(), Some(vm_name.clone()), ctx);
            }
            DevicesAction::WorkspaceDismissApproval(job_id) => {
                self.workspace_dismissed_approval = Some(job_id.clone());
                ctx.notify();
            }
            DevicesAction::NodeCardClick(node_id) => {
                self.handle_node_card_click(node_id.clone(), ctx);
            }
            DevicesAction::SetNodeHover(node_id) => {
                if self.hovered_node_id != *node_id {
                    self.hovered_node_id = node_id.clone();
                    ctx.notify();
                }
            }
            DevicesAction::ClearNodeHoverIf(node_id) => {
                if self.hovered_node_id.as_ref() == Some(node_id) {
                    self.hovered_node_id = None;
                    ctx.notify();
                }
            }
            DevicesAction::OpenDeleteNodeModal(node_id) => {
                self.open_delete_node_modal(node_id.clone(), ctx);
            }
            DevicesAction::CloseDeleteNodeModal => self.close_delete_node_modal(ctx),
            DevicesAction::ConfirmDeleteNode => self.confirm_delete_node(ctx),
            DevicesAction::OpenDeviceContextMenu { node_id, x, y } => {
                self.device_context_menu = Some((node_id.clone(), *x, *y));
                self.selected_node_id = Some(node_id.clone());
                ctx.notify();
            }
            DevicesAction::CloseDeviceContextMenu => self.close_device_context_menu(ctx),
            DevicesAction::SendMessage(node_id) => {
                self.close_device_context_menu(ctx);
                let is_local = self
                    .cluster
                    .as_ref()
                    .is_some_and(|cluster| cluster.local_node_id == *node_id);
                if is_local {
                    self.status_flash = Some("无法给本机发信息".into());
                    ctx.notify();
                    return;
                }
                self.status_flash = Some("正在打开聊天…".into());
                ctx.notify();
                ctx.spawn(
                    async move {
                        tokio::time::sleep(Duration::from_millis(2600)).await;
                    },
                    |view, _, ctx| {
                        if view.status_flash.as_deref() == Some("正在打开聊天…") {
                            view.status_flash = None;
                            ctx.notify();
                        }
                    },
                );
                ctx.emit(DevicesEvent::OpenChat {
                    node_id: node_id.clone(),
                });
            }
            DevicesAction::OpenRemoteDesktop(node_id) => {
                self.close_device_context_menu(ctx);
                let is_local = self
                    .cluster
                    .as_ref()
                    .is_some_and(|cluster| cluster.local_node_id == *node_id);
                if is_local {
                    self.status_flash = Some("无法对本机打开远程桌面".into());
                    ctx.notify();
                    return;
                }
                let available = self.cluster.as_ref().is_some_and(|cluster| {
                    cluster
                        .nodes
                        .iter()
                        .find(|node| node.node_id == *node_id)
                        .is_some_and(|node| node_remote_desktop_available(node, false))
                });
                if !available {
                    self.status_flash = Some("终端离线或缺少远程桌面地址".into());
                    ctx.notify();
                    return;
                }
                self.status_flash = Some("正在打开远程桌面…".into());
                ctx.notify();
                ctx.emit(DevicesEvent::OpenRemoteDesktop {
                    node_id: node_id.clone(),
                });
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShareNewMenuMode {
    Hidden,
    AddSharedFolder,
    CreateEntries,
}

fn share_node_can_manage(
    is_local: bool,
    same_account: bool,
    server_member_confirmed: bool,
    revoked: bool,
) -> bool {
    is_local || (same_account && server_member_confirmed && !revoked)
}

fn share_new_menu_mode(can_manage: bool, share_path_empty: bool) -> ShareNewMenuMode {
    if !can_manage {
        ShareNewMenuMode::Hidden
    } else if share_path_empty {
        ShareNewMenuMode::AddSharedFolder
    } else {
        ShareNewMenuMode::CreateEntries
    }
}

fn short_cluster_id(id: &str) -> String {
    if id.len() > 12 {
        id.chars().take(12).collect()
    } else {
        id.to_string()
    }
}

/// Whether Files share-root listing should reload after a cluster status update.
fn should_reload_share_root_on_cluster_update(
    previous: Option<&ClusterStatusDto>,
    next: &ClusterStatusDto,
    browsing_node_id: Option<&str>,
    at_share_root: bool,
    share_entries_empty: bool,
) -> bool {
    if !at_share_root {
        return false;
    }
    let Some(node_id) = browsing_node_id else {
        return false;
    };
    if next.local_node_id == node_id {
        return false;
    }
    let next_fp = share_root_fingerprint(next, node_id);
    let prev_fp = previous.map(|status| share_root_fingerprint(status, node_id));
    if prev_fp.as_ref() != Some(&next_fp) {
        return true;
    }
    let _ = share_entries_empty;
    false
}

fn share_root_fingerprint(status: &ClusterStatusDto, node_id: &str) -> String {
    let Some(node) = status.nodes.iter().find(|node| node.node_id == node_id) else {
        return String::new();
    };
    let mut ids: Vec<&str> = node
        .share_volumes
        .iter()
        .map(|volume| volume.volume_id.as_str())
        .collect();
    ids.sort_unstable();
    format!("{}|{}|{}", node.online, node.presence_status, ids.join(","))
}

#[cfg(test)]
mod share_root_reload_tests {
    use super::{share_root_fingerprint, should_reload_share_root_on_cluster_update};
    use wormhole_desktop_core::cluster_commands::{
        ClusterNodeDto, ClusterStatusDto, ShareVolumeRosterDto, NODE_PRESENCE_SIGNED_IN,
    };

    fn sample_status(node_id: &str, online: bool, volumes: &[(&str, &str)]) -> ClusterStatusDto {
        ClusterStatusDto {
            configured: true,
            cluster_id: Some("c1".into()),
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: "local".into(),
            transport: "iroh".into(),
            nodes: vec![ClusterNodeDto {
                node_id: node_id.into(),
                device_id: None,
                chat_endpoint_id: None,
                chat_bootstrap_addrs: Vec::new(),
                hostname: "remote".into(),
                os: "linux".into(),
                roles: Vec::new(),
                online,
                presence_status: if online {
                    "online".into()
                } else {
                    NODE_PRESENCE_SIGNED_IN.into()
                },
                cpu_cores: 1,
                memory_total: 1,
                storage_total: 0,
                storage_free: 0,
                billing_node_score: None,
                billing_expired: false,
                role: "member".into(),
                removable: false,
                revoked: false,
                server_member_confirmed: true,
                same_account: true,
                pending_handshake: false,
                handshake_error: None,
                share_volumes: volumes
                    .iter()
                    .map(|(id, name)| ShareVolumeRosterDto {
                        volume_id: (*id).into(),
                        name: (*name).into(),
                    })
                    .collect(),
            }],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
            role_stale: false,
        }
    }

    #[test]
    fn reloads_when_remote_share_roster_changes() {
        let prev = sample_status("remote", false, &[]);
        let next = sample_status("remote", false, &[("v1", "图片")]);
        assert!(should_reload_share_root_on_cluster_update(
            Some(&prev),
            &next,
            Some("remote"),
            true,
            true,
        ));
        assert_ne!(
            share_root_fingerprint(&prev, "remote"),
            share_root_fingerprint(&next, "remote")
        );
    }

    #[test]
    fn does_not_loop_reload_unchanged_empty_remote_root() {
        let status = sample_status("remote", false, &[("v1", "图片")]);
        assert!(!should_reload_share_root_on_cluster_update(
            Some(&status),
            &status,
            Some("remote"),
            true,
            true,
        ));
    }

    #[test]
    fn skips_reload_for_local_or_nested_path() {
        let status = sample_status("remote", true, &[("v1", "图片")]);
        assert!(!should_reload_share_root_on_cluster_update(
            None,
            &status,
            Some("local"),
            true,
            true,
        ));
        assert!(!should_reload_share_root_on_cluster_update(
            None,
            &status,
            Some("remote"),
            false,
            true,
        ));
    }
}

#[cfg(test)]
mod share_browse_path_tests {
    use super::default_share_browse_path;

    #[test]
    fn default_share_browse_path_matches_platform() {
        let path = default_share_browse_path();
        #[cfg(windows)]
        {
            assert_eq!(path, "D:\\Projects\\Share");
        }
        #[cfg(target_os = "macos")]
        {
            assert_eq!(path, "/Users/Shared/Wormhole");
        }
        #[cfg(target_os = "linux")]
        {
            assert!(
                path.ends_with("/Wormhole") || path == "/home",
                "unexpected Linux default share path: {path}"
            );
            assert!(
                !path.starts_with("/Users/Shared"),
                "Linux must not use macOS Shared path: {path}"
            );
        }
    }
}

#[cfg(test)]
mod workspace_office_route_tests {
    use super::{recommended_workspace_app, workspace_app_label, workspace_apps_for_file};

    #[test]
    fn office_files_use_onlyoffice_without_default_fallback() {
        assert_eq!(workspace_apps_for_file("report.docx"), &["onlyoffice"]);
        assert_eq!(recommended_workspace_app("budget.xlsx"), "onlyoffice");
        assert_eq!(
            workspace_app_label("onlyoffice"),
            "ONLYOFFICE Desktop Editors"
        );
    }
}

#[cfg(test)]
mod share_new_menu_tests {
    use super::{share_new_menu_mode, share_node_can_manage, ShareNewMenuMode};

    #[test]
    fn different_account_remote_hides_new_menu() {
        assert_eq!(share_new_menu_mode(false, true), ShareNewMenuMode::Hidden);
        assert_eq!(share_new_menu_mode(false, false), ShareNewMenuMode::Hidden);
    }

    #[test]
    fn local_and_confirmed_same_account_remote_can_manage() {
        assert!(share_node_can_manage(true, false, false, false));
        assert!(share_node_can_manage(false, true, true, false));
        assert!(!share_node_can_manage(false, false, true, false));
        assert!(!share_node_can_manage(false, true, false, false));
        assert!(!share_node_can_manage(false, true, true, true));
    }

    #[test]
    fn manageable_root_offers_add_shared_folder() {
        assert_eq!(
            share_new_menu_mode(true, true),
            ShareNewMenuMode::AddSharedFolder
        );
    }

    #[test]
    fn manageable_folder_offers_create_entries() {
        assert_eq!(
            share_new_menu_mode(true, false),
            ShareNewMenuMode::CreateEntries
        );
    }
}
