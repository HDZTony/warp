use pathfinder_color::ColorU;
use warpui::assets::asset_cache::AssetCache;
use warpui::elements::Fill;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex, Image,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::{CacheOption, ImageType};

use crate::ui::chat::image_asset::decode_image_asset_payload;
use crate::ui::clipboard::{read_clipboard_text, write_clipboard_text};
use crate::ui::cluster_topology_panel::{
    device_matches_query, node_display_label, node_remote_desktop_available, ClusterTopologyPanel,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{fetch_cluster_for_ui, refresh_cluster_for_ui};
use crate::ui::devices_actions::DevicesAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    chat_search_pill, chat_sidebar_search_bg, section_hint, section_title, status_line,
    tab_content_fill, truncate_middle, StatusTone, AGENT_ROW_RADIUS, HUD_RADIUS, SECTION_PADDING,
};
use crate::ui::text_field_input::{
    render_field_with_caret, render_search_field_with_caret, sync_caret_blink,
    wrap_text_field_focus_on_click, wrap_text_field_focus_on_click_with_label, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::agent_pair_commands::{
    create_agent_pair_code, AgentPairCodeDto, CreateAgentPairCodeParams,
};
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
use wormhole_desktop_core::toolbox_ui::{
    is_user_app_capability, toolbox_list_tools, user_app_supports_file, CatalogUserAppSummary,
    ToolExecutorKind, ToolSummary, WorkspaceUserAppManifest, workspace_user_app_catalog_list,
    workspace_user_app_list,
};
use wormhole_desktop_core::workspace_ui::{
    workspace_app_preference, workspace_approve_provision_job,
    workspace_approve_provision_job_with_candidate, workspace_list_provision_jobs,
    workspace_list_workers, workspace_probe_vm_candidates, workspace_save_app_preference,
    WorkspaceExecutionMode, WorkspaceProvisionJob, WorkspaceProvisionRequest,
    WorkspaceProvisionStage, WorkspaceVmCandidate, WorkspaceWorker,
};

use std::collections::{BTreeMap, HashMap};
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
    /// Session cache keyed by `(node_id, share_path_string)`; back/forward reuse without refetch.
    share_listing_cache: HashMap<(String, String), Vec<ShareEntryDto>>,
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
    agent_pair_modal_open: bool,
    agent_pair_busy: bool,
    agent_pair: Option<AgentPairCodeDto>,
    agent_pair_error: Option<String>,
    agent_pair_qr_asset_id: Option<String>,
    agent_pair_copy_ack: bool,
    create_cluster_busy: bool,
    share_scroll: ClippedScrollStateHandle,
    topology_scroll: ClippedScrollStateHandle,
    share_context_entry: Option<String>,
    share_context_pos: Option<(f32, f32)>,
    share_unshare_volume_id: Option<String>,
    share_unshare_name: Option<String>,
    share_unshare_busy: bool,
    share_file_busy: bool,
    last_file_click: Option<(String, std::time::Instant)>,
    selected_share_file: Option<String>,
    workspace_workers: Vec<WorkspaceWorker>,
    workspace_tools: Vec<ToolSummary>,
    workspace_user_apps: Vec<WorkspaceUserAppManifest>,
    workspace_catalog_user_apps: Vec<CatalogUserAppSummary>,
    workspace_vm_candidates: Vec<WorkspaceVmCandidate>,
    workspace_jobs: Vec<WorkspaceProvisionJob>,
    workspace_remote_job: Option<WorkspaceProvisionJob>,
    workspace_status: String,
    workspace_loading: bool,
    workspace_selected_app: String,
    workspace_app_picker_open: bool,
    workspace_app_search: String,
    workspace_app_search_field: TextFieldState,
    workspace_app_search_focused: bool,
    device_search: String,
    device_search_field: TextFieldState,
    device_search_focused: bool,
    workspace_dismissed_approval: Option<String>,
    workspace_auto_opened_job: Option<String>,
    bootstrap_busy: bool,
    bootstrap_pending_since: Option<Instant>,
    /// First time we observed a remote device that was recently seen but is not online.
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
const RECENTLY_SEEN_HINT_AFTER: Duration = Duration::from_secs(30);
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
            share_listing_cache: HashMap::new(),
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
            agent_pair_modal_open: false,
            agent_pair_busy: false,
            agent_pair: None,
            agent_pair_error: None,
            agent_pair_qr_asset_id: None,
            agent_pair_copy_ack: false,
            create_cluster_busy: false,
            share_scroll: ClippedScrollStateHandle::new(),
            topology_scroll: ClippedScrollStateHandle::new(),
            share_context_entry: None,
            share_context_pos: None,
            share_unshare_volume_id: None,
            share_unshare_name: None,
            share_unshare_busy: false,
            share_file_busy: false,
            last_file_click: None,
            selected_share_file: None,
            workspace_workers: Vec::new(),
            workspace_tools: Vec::new(),
            workspace_user_apps: Vec::new(),
            workspace_catalog_user_apps: Vec::new(),
            workspace_vm_candidates: Vec::new(),
            workspace_jobs: Vec::new(),
            workspace_remote_job: None,
            workspace_status: wormhole_i18n::t("devices.workspace.pick_file"),
            workspace_loading: false,
            workspace_selected_app: "default".into(),
            workspace_app_picker_open: false,
            workspace_app_search: String::new(),
            workspace_app_search_field: TextFieldState::new(),
            workspace_app_search_focused: false,
            device_search: String::new(),
            device_search_field: TextFieldState::new(),
            device_search_focused: false,
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
            Ok(()) => wormhole_i18n::t("devices.rdp.opened"),
            Err(err) => wormhole_i18n::t_args(
                "devices.rdp.open_failed",
                &[("err", &err)],
            ),
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
            .is_some_and(|started| started.elapsed() >= RECENTLY_SEEN_HINT_AFTER)
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
            self.force_load_share_directory(ctx);
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
                self.clear_share_listing_cache();
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
            self.clear_share_listing_cache();
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
                let status = refresh_cluster_for_ui(&state).await;
                let remarks = load_device_remarks(&state.data_dir)
                    .await
                    .unwrap_or_default();
                (status, remarks)
            },
            move |view, output, ctx| {
                view.cluster_refresh_busy = false;
                let (status, remarks) = output;
                view.device_remarks = remarks;
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
                                    view.force_load_share_directory(ctx);
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

    fn clear_share_listing_cache(&mut self) {
        self.share_listing_cache.clear();
    }

    /// Load the current share path. Cache hits return immediately without network.
    fn load_share_directory(&mut self, ctx: &mut ViewContext<Self>) {
        self.load_share_directory_inner(ctx, false);
    }

    /// Drop the current path cache entry (if any) and fetch from core.
    fn force_load_share_directory(&mut self, ctx: &mut ViewContext<Self>) {
        self.load_share_directory_inner(ctx, true);
    }

    fn load_share_directory_inner(&mut self, ctx: &mut ViewContext<Self>, force: bool) {
        let Some(node_id) = self.browsing_node_id.clone() else {
            return;
        };
        let path = self.share_path_string();
        let cache_key = (node_id.clone(), path.clone());
        if force {
            self.share_listing_cache.remove(&cache_key);
        } else if let Some(cached) = self.share_listing_cache.get(&cache_key).cloned() {
            self.share_load_seq = self.share_load_seq.wrapping_add(1);
            self.share_loading = false;
            self.share_error = None;
            self.share_entries = cached;
            if self
                .selected_share_file
                .as_ref()
                .is_some_and(|name| !self.share_entries.iter().any(|e| &e.name == name))
            {
                self.selected_share_file = None;
            }
            ctx.notify();
            return;
        }

        self.share_load_seq = self.share_load_seq.wrapping_add(1);
        let load_seq = self.share_load_seq;
        self.share_loading = true;
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
                        view.share_listing_cache
                            .insert((node_id, path), entries.clone());
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
        if self.workspace_app_picker_open {
            self.workspace_app_picker_open = false;
            self.workspace_app_search_focused = false;
            self.workspace_app_search.clear();
            self.workspace_app_search_field = TextFieldState::new();
        }
        self.workspace_selected_app = self.recommended_workspace_app(&preference_file);
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
                    let app = workspace_canonical_tool_id(&app);
                    if view.selected_share_entry().is_some_and(|entry| {
                        view.workspace_catalog_tool_for_file(&entry.name, &app)
                            .is_some()
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
                let tools = toolbox_list_tools(&state).await;
                let local_user_apps = workspace_user_app_list(&state).await;
                let catalog_user_apps = workspace_user_app_catalog_list(&state).await;
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
                (
                    workers,
                    tools,
                    local_user_apps,
                    catalog_user_apps,
                    jobs,
                    candidates,
                    remote,
                )
            },
            |view, output, ctx| {
                view.workspace_loading = false;
                let (workers, tools, local_user_apps, catalog_user_apps, jobs, candidates, remote) =
                    output;
                match workers {
                    Ok(workers) => view.workspace_workers = workers,
                    Err(error) => {
                        view.workspace_workers.clear();
                        view.set_workspace_progress(wormhole_i18n::t_args(
                            "devices.workspace.worker_detect_failed",
                            &[("err", &error)],
                        ));
                    }
                }
                match tools {
                    Ok(tools) => {
                        view.workspace_tools =
                            workspace_source_runtime_tools_from_catalog(tools);
                    }
                    Err(error) => {
                        view.workspace_tools.clear();
                        view.set_workspace_progress(wormhole_i18n::t_args(
                            "toolbox.read_failed",
                            &[("err", &error)],
                        ));
                    }
                }
                match local_user_apps {
                    Ok(apps) => view.workspace_user_apps = apps,
                    Err(_) => view.workspace_user_apps.clear(),
                }
                match catalog_user_apps {
                    Ok(apps) => view.workspace_catalog_user_apps = apps,
                    Err(_) => view.workspace_catalog_user_apps.clear(),
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
                            let mut progress = job.detail.clone();
                            if job.bytes_total > 0 && job.bytes_downloaded > 0 {
                                let percent =
                                    job.bytes_downloaded.saturating_mul(100) / job.bytes_total;
                                progress = format!(
                                    "{progress} · {}% · {} / {}",
                                    percent,
                                    format_size(job.bytes_downloaded),
                                    format_size(job.bytes_total)
                                );
                            }
                            view.set_workspace_progress(progress);
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
                            view.set_workspace_progress(wormhole_i18n::t_args(
                                "devices.workspace.read_progress_failed",
                                &[("err", &error)],
                            ));
                        }
                    }
                } else {
                    view.update_workspace_status_from_selection();
                }
                ctx.notify();
            },
        );
    }

    fn set_workspace_progress(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.workspace_status = message.clone();
        self.share_status = Some(message);
    }

    fn update_workspace_status_from_selection(&mut self) {
        let Some(entry) = self.selected_share_entry() else {
            self.workspace_status = wormhole_i18n::t("devices.workspace.pick_file");
            return;
        };
        let Some(author) = entry.version_author.as_deref() else {
            self.workspace_status = wormhole_i18n::t("devices.workspace.prepare_identity");
            return;
        };
        let Some(worker) = self.workspace_worker_for_author(author) else {
            self.workspace_status = wormhole_i18n::t("devices.workspace.runner_not_ready");
            return;
        };
        if workspace_worker_supports_app(worker, &self.workspace_selected_app) {
            let mode = workspace_worker_mode_label(worker, &self.workspace_selected_app);
            self.workspace_status = wormhole_i18n::t_args(
                "devices.workspace.runner_ready",
                &[("host", &worker.hostname), ("mode", &mode)],
            );
        } else {
            self.workspace_status = wormhole_i18n::t_args(
                "devices.workspace.online_image_missing",
                &[
                    ("name", &worker.hostname),
                    ("what", &self.workspace_selected_app),
                ],
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
        let mode_hint = self
            .selected_share_entry()
            .and_then(|e| e.version_author.as_deref())
            .and_then(|author| self.workspace_worker_for_author(author))
            .map(|worker| workspace_worker_mode_label(worker, &self.workspace_selected_app))
            .unwrap_or_else(|| wormhole_i18n::t("devices.workspace.runner_default"));
        let host_native = self
            .selected_share_entry()
            .and_then(|e| e.version_author.as_deref())
            .and_then(|author| self.workspace_worker_for_author(author))
            .and_then(|worker| {
                workspace_worker_execution_mode_for_app(worker, &self.workspace_selected_app)
            })
            == Some(WorkspaceExecutionMode::HostNative);
        let mut opening = wormhole_i18n::t_args(
            "devices.workspace.opening",
            &[("name", &entry_name), ("mode", &mode_hint)],
        );
        if host_native {
            opening.push_str(&wormhole_i18n::t("devices.workspace.wayland_focus_hint"));
        }
        self.set_workspace_progress(opening);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                remote_open_share_entry(&state, params).await
            },
            move |view, result, ctx| {
                view.share_file_busy = false;
                view.set_workspace_progress(match result {
                    Ok(session) => wormhole_i18n::t_args(
                        "devices.workspace.session_submitted",
                        &[("id", &session.session_id)],
                    ),
                    Err(error) => wormhole_i18n::t_args(
                        "devices.workspace.open_failed",
                        &[("err", &error)],
                    ),
                });
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
            self.set_workspace_progress(wormhole_i18n::t("devices.workspace.no_cluster"));
            ctx.notify();
            return;
        };
        let Some(source_node_id) = entry.version_author.clone() else {
            self.set_workspace_progress(wormhole_i18n::t("devices.workspace.missing_source"));
            ctx.notify();
            return;
        };
        let Some(entry_id) = entry.entry_id.clone() else {
            self.set_workspace_progress(wormhole_i18n::t("devices.workspace.missing_entry"));
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
        self.set_workspace_progress(wormhole_i18n::t("devices.workspace.provisioning"));
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
                        view.set_workspace_progress(job.detail.clone());
                        view.workspace_remote_job = Some(job);
                    }
                    Err(error) => {
                        view.set_workspace_progress(wormhole_i18n::t_args(
                            "devices.workspace.request_install_failed",
                            &[("err", &error)],
                        ));
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
                        view.set_workspace_progress(job.detail);
                        view.workspace_dismissed_approval = None;
                    }
                    Err(error) => {
                        view.set_workspace_progress(wormhole_i18n::t_args(
                            "devices.workspace.approve_install_failed",
                            &[("err", &error)],
                        ));
                    }
                }
                view.refresh_workspace(ctx);
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn open_workspace_app_picker(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(entry) = self.selected_share_entry() else {
            return;
        };
        let file_name = entry.name.clone();
        let installed = self.workspace_installed_apps_for_file(&file_name);
        if !installed
            .iter()
            .any(|app| app == &self.workspace_selected_app)
        {
            self.workspace_selected_app = installed
                .first()
                .cloned()
                .unwrap_or_else(|| self.recommended_workspace_app(&file_name));
        }
        self.workspace_app_search.clear();
        self.workspace_app_search_field = TextFieldState::new();
        self.workspace_app_search_focused = false;
        self.workspace_app_picker_open = true;
        self.close_share_context_menu(ctx);
        // Ensure catalog is fresh when opening the picker.
        self.refresh_workspace(ctx);
        ctx.notify();
    }

    fn close_workspace_app_picker(&mut self, ctx: &mut ViewContext<Self>) {
        self.workspace_app_picker_open = false;
        self.workspace_app_search_focused = false;
        self.workspace_app_search.clear();
        self.workspace_app_search_field = TextFieldState::new();
        ctx.notify();
    }

    fn select_workspace_app(&mut self, app: String, ctx: &mut ViewContext<Self>) {
        let app = workspace_canonical_tool_id(&app);
        let known = self.workspace_catalog_tool(&app).is_some()
            || is_user_app_capability(&app)
            || self
                .workspace_user_apps
                .iter()
                .any(|m| m.app_id.eq_ignore_ascii_case(&app));
        if !known {
            self.set_workspace_progress(wormhole_i18n::t("devices.workspace.unsupported_open"));
            ctx.notify();
            return;
        }
        self.workspace_selected_app = app;
        self.update_workspace_status_from_selection();
        ctx.notify();
    }

    fn edit_workspace_app_search(
        &mut self,
        edit: &TextFieldEditAction,
        ctx: &mut ViewContext<Self>,
    ) {
        self.workspace_app_search_field
            .apply(&mut self.workspace_app_search, edit);
        self.workspace_app_search_focused = true;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn clear_device_search(&mut self) {
        self.device_search.clear();
        self.device_search_field = TextFieldState::new();
        self.device_search_focused = false;
    }

    fn edit_device_search(&mut self, edit: &TextFieldEditAction, ctx: &mut ViewContext<Self>) {
        self.device_search_field
            .apply(&mut self.device_search, edit);
        self.device_search_focused = true;
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn device_search_box(&self) -> Box<dyn Element> {
        let search_focused = self.device_search_focused;
        let draft = self.device_search.clone();
        let marked = self.device_search_field.marked_text.clone();
        let search_placeholder = wormhole_i18n::t("devices.search.placeholder");
        let field = render_search_field_with_caret(
            &draft,
            &marked,
            &search_placeholder,
            self.font,
            search_focused,
            false,
            self.caret_blink.visible,
            self.device_search_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(DevicesAction::DeviceSearchEdit(action));
        })
        .focused(search_focused)
        .ime_preedit(!marked.is_empty())
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "tab" || keystroke.key == "escape" {
                ctx.dispatch_typed_action(DevicesAction::BlurDeviceSearch);
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        let input = wrap_text_field_focus_on_click_with_label(
            input,
            wormhole_i18n::t("devices.search.placeholder"),
            |ctx| {
                ctx.dispatch_typed_action(DevicesAction::FocusDeviceSearch);
            },
        );
        let input = EventHandler::new(input)
            .with_automation_label(wormhole_i18n::t("devices.search.placeholder"))
            .with_automation_id("devices:search")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::FocusDeviceSearch);
                DispatchEventResult::PropagateToParent
            })
            .finish();

        let border_color = if search_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };

        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::chat_sidebar_search_icon(theme::muted()))
                    .with_horizontal_margin(2.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, input).finish())
            .finish();

        chat_search_pill(
            row,
            chat_sidebar_search_bg(),
            border_color,
            7.0,
            10.0,
            AGENT_ROW_RADIUS,
        )
    }

    fn confirm_workspace_app_open(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(entry) = self.selected_share_entry() else {
            self.close_workspace_app_picker(ctx);
            return;
        };
        let file_name = entry.name.clone();
        let version_author = entry.version_author.clone();
        let app = self.workspace_selected_app.clone();
        if !self
            .workspace_installed_apps_for_file(&file_name)
            .iter()
            .any(|installed| installed == &app)
        {
            return;
        }
        let core = self.core.clone();
        let preference_app = app.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                workspace_save_app_preference(&state, &file_name, &preference_app).await
            },
            |view, result, ctx| {
                if let Err(error) = result {
                    view.set_workspace_progress(wormhole_i18n::t_args(
                        "devices.workspace.save_open_failed",
                        &[("err", &error)],
                    ));
                    ctx.notify();
                }
            },
        );
        self.close_workspace_app_picker(ctx);
        if version_author.as_deref().is_none_or(str::is_empty) {
            self.workspace_open_selected(ctx);
            return;
        }
        let can_open = version_author.as_deref().is_some_and(|author| {
            self.workspace_worker_for_author(author)
                .is_some_and(|worker| {
                    worker.available
                        && worker.vm_ready
                        && workspace_worker_supports_app(worker, &app)
                })
        });
        if can_open {
            self.workspace_open_selected(ctx);
        } else {
            self.workspace_request_provision(ctx);
        }
    }

    fn workspace_install_and_open(&mut self, app: String, ctx: &mut ViewContext<Self>) {
        let app = workspace_canonical_tool_id(&app);
        if is_user_app_capability(&app) {
            let local_ready = self
                .workspace_user_apps
                .iter()
                .any(|item| item.app_id.eq_ignore_ascii_case(&app));
            let catalog = self
                .workspace_catalog_user_apps
                .iter()
                .find(|item| item.app_id.eq_ignore_ascii_case(&app) && item.package_ready)
                .cloned();
            if !local_ready && catalog.is_none() {
                self.set_workspace_progress(
                    wormhole_i18n::t("devices.workspace.no_user_app_version"),
                );
                ctx.notify();
                return;
            }
            // Do not install on the viewer host. The Linux worker pulls the package
            // (ACL-checked) at session start via ensure_user_app_staged_for_session.
            let app_id = catalog
                .map(|item| item.app_id)
                .unwrap_or_else(|| app.clone());
            self.workspace_open_user_app_via_worker(app_id, ctx);
            return;
        }
        if self.workspace_catalog_tool(&app).is_none() {
            return;
        }
        self.workspace_selected_app = app;
        self.update_workspace_status_from_selection();
        let Some(entry) = self.selected_share_entry() else {
            self.close_workspace_app_picker(ctx);
            return;
        };
        let file_name = entry.name.clone();
        let preference_app = self.workspace_selected_app.clone();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                workspace_save_app_preference(&state, &file_name, &preference_app).await
            },
            |view, result, ctx| {
                if let Err(error) = result {
                    view.set_workspace_progress(wormhole_i18n::t_args(
                        "devices.workspace.save_open_failed",
                        &[("err", &error)],
                    ));
                    ctx.notify();
                }
            },
        );
        self.close_workspace_app_picker(ctx);
        if self.workspace_remote_job.as_ref().is_some_and(|job| {
            job.stage == WorkspaceProvisionStage::Failed
        }) {
            self.workspace_retry_provision(ctx);
        } else {
            self.workspace_request_provision(ctx);
        }
    }

    fn workspace_open_user_app_via_worker(&mut self, app_id: String, ctx: &mut ViewContext<Self>) {
        self.workspace_selected_app = app_id.clone();
        self.update_workspace_status_from_selection();
        self.set_workspace_progress(wormhole_i18n::t_args(
            "devices.workspace.pulling_user_app",
            &[("app_id", &app_id)],
        ));
        let Some(entry) = self.selected_share_entry() else {
            self.close_workspace_app_picker(ctx);
            return;
        };
        let file_name = entry.name.clone();
        let version_author = entry.version_author.clone();
        let core = self.core.clone();
        let preference_app = app_id;
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                if !file_name.is_empty() {
                    workspace_save_app_preference(&state, &file_name, &preference_app).await?;
                }
                Ok::<_, String>(())
            },
            |view, result, ctx| {
                if let Err(error) = result {
                    view.set_workspace_progress(wormhole_i18n::t_args(
                        "devices.workspace.save_open_failed",
                        &[("err", &error)],
                    ));
                    ctx.notify();
                }
            },
        );
        self.close_workspace_app_picker(ctx);
        if self.workspace_remote_job.as_ref().is_some_and(|job| {
            job.stage == WorkspaceProvisionStage::Failed
        }) {
            self.workspace_retry_provision(ctx);
            return;
        }
        if version_author.as_deref().is_none_or(str::is_empty) {
            self.workspace_open_selected(ctx);
            return;
        }
        let can_open = version_author.as_deref().is_some_and(|author| {
            self.workspace_worker_for_author(author)
                .is_some_and(|worker| {
                    worker.available
                        && worker.vm_ready
                        && workspace_worker_supports_app(worker, &self.workspace_selected_app)
                })
        });
        if can_open {
            self.workspace_open_selected(ctx);
        } else {
            self.workspace_request_provision(ctx);
        }
    }

    fn workspace_installed_apps_for_file(&self, file_name: &str) -> Vec<String> {
        let Some(author) = self
            .selected_share_entry()
            .and_then(|entry| entry.version_author.as_deref())
        else {
            return Vec::new();
        };
        let Some(worker) = self.workspace_worker_for_author(author) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for tool in self.workspace_catalog_tools_for_file(file_name) {
            let id = workspace_canonical_tool_id(&tool.descriptor.id);
            if workspace_worker_supports_app(worker, &id) && !out.iter().any(|e| e == &id) {
                out.push(id);
            }
        }
        for app in &self.workspace_user_apps {
            if !user_app_supports_file(&app.extensions, file_name) {
                continue;
            }
            if workspace_worker_supports_app(worker, &app.app_id)
                && !out.iter().any(|e| e.eq_ignore_ascii_case(&app.app_id))
            {
                out.push(app.app_id.clone());
            }
        }
        for image in &worker.images {
            for installed in &image.installed_apps {
                if !is_user_app_capability(installed) {
                    continue;
                }
                if out.iter().any(|e| e.eq_ignore_ascii_case(installed)) {
                    continue;
                }
                if let Some(local) = self
                    .workspace_user_apps
                    .iter()
                    .find(|app| app.app_id.eq_ignore_ascii_case(installed))
                {
                    if !user_app_supports_file(&local.extensions, file_name) {
                        continue;
                    }
                } else if let Some(catalog) = self
                    .workspace_catalog_user_apps
                    .iter()
                    .find(|app| app.app_id.eq_ignore_ascii_case(installed))
                {
                    if !user_app_supports_file(&catalog.extensions, file_name) {
                        continue;
                    }
                }
                out.push(installed.clone());
            }
        }
        out
    }

    fn workspace_catalog_tools_for_file(&self, file_name: &str) -> Vec<&ToolSummary> {
        self.workspace_tools
            .iter()
            .filter(|tool| workspace_tool_supports_file(tool, file_name))
            .collect()
    }

    fn workspace_catalog_tool(&self, app: &str) -> Option<&ToolSummary> {
        let app = workspace_canonical_tool_id(app);
        self.workspace_tools
            .iter()
            .find(|tool| workspace_canonical_tool_id(&tool.descriptor.id) == app)
    }

    fn workspace_catalog_tool_for_file(&self, file_name: &str, app: &str) -> Option<&ToolSummary> {
        self.workspace_catalog_tools_for_file(file_name)
            .into_iter()
            .find(|tool| workspace_canonical_tool_id(&tool.descriptor.id) == workspace_canonical_tool_id(app))
    }

    fn recommended_workspace_app(&self, name: &str) -> String {
        if let Some(app) = self.workspace_installed_apps_for_file(name).into_iter().next() {
            return app;
        }
        self.workspace_catalog_tools_for_file(name)
            .into_iter()
            .next()
            .map(|tool| workspace_canonical_tool_id(&tool.descriptor.id))
            .unwrap_or_default()
    }

    fn workspace_app_label(&self, app: &str) -> String {
        if let Some(tool) = self.workspace_catalog_tool(app) {
            return tool.descriptor.name.clone();
        }
        if let Some(local) = self
            .workspace_user_apps
            .iter()
            .find(|item| item.app_id.eq_ignore_ascii_case(app))
        {
            return local.display_name.clone();
        }
        if let Some(catalog) = self
            .workspace_catalog_user_apps
            .iter()
            .find(|item| item.app_id.eq_ignore_ascii_case(app))
        {
            return catalog.display_name.clone();
        }
        if app.is_empty() {
            wormhole_i18n::t("devices.workspace.no_app_selected")
        } else {
            app.to_string()
        }
    }

    fn workspace_app_desc(&self, app: &str) -> String {
        if let Some(tool) = self.workspace_catalog_tool(app) {
            return tool.descriptor.description.clone();
        }
        if let Some(local) = self
            .workspace_user_apps
            .iter()
            .find(|item| item.app_id.eq_ignore_ascii_case(app))
        {
            let exts = if local.extensions.is_empty() {
                wormhole_i18n::t("devices.workspace.any_ext")
            } else {
                local.extensions.join(", ")
            };
            return wormhole_i18n::t_args(
                "devices.workspace.user_app_exts",
                &[("exts", &exts)],
            );
        }
        if let Some(catalog) = self
            .workspace_catalog_user_apps
            .iter()
            .find(|item| item.app_id.eq_ignore_ascii_case(app))
        {
            return wormhole_i18n::t_args(
                "devices.workspace.user_app_catalog",
                &[("visibility", &catalog.visibility)],
            );
        }
        if is_user_app_capability(app) {
            return wormhole_i18n::t("devices.workspace.registered_user_apps");
        }
        String::new()
    }

    pub fn open_node_from_chat(&mut self, node_id: String, ctx: &mut ViewContext<Self>) {
        self.open_node(node_id, ctx);
    }

    /// Prefill cluster join modal from a deeplink or saved pending invite.
    pub fn prefill_join_invite(&mut self, invite: String, ctx: &mut ViewContext<Self>) {
        let invite = invite.trim().to_string();
        if invite.is_empty() {
            return;
        }
        self.join_invite_draft = invite;
        self.join_feedback = None;
        self.open_join_modal(ctx);
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
                let remark = self.device_remarks.get(&n.node_id).map(String::as_str);
                node_display_label(n, node_id == local_id, remark)
            })
            .unwrap_or_else(|| node_id.clone());

        self.mode = ViewMode::Files;
        self.clear_device_search();
        self.join_modal_open = false;
        self.join_invite_draft.clear();
        self.join_feedback = None;
        self.cluster_picker_open = false;
        self.device_context_menu = None;
        let is_remote = node_id != local_id;
        self.browsing_node_id = Some(node_id.clone());
        self.browsing_label = label;
        self.share_entries.clear();
        self.clear_share_listing_cache();
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
        self.force_load_share_directory(ctx);
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
                            view.force_load_share_directory(ctx);
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
        self.clear_share_listing_cache();
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

    /// True when the browsed peer can accept live P2P share operations.
    ///
    /// Fully offline peers still allow browse/open of local ClusterShare replicas, but
    /// mutations and sync/remote-open require a reachable endpoint.
    fn browsing_peer_live(&self) -> bool {
        if self.browsing_local() {
            return true;
        }
        let Some(cluster) = self.cluster.as_ref() else {
            return false;
        };
        let Some(node_id) = self.browsing_node_id.as_deref() else {
            return false;
        };
        cluster.nodes.iter().any(|node| {
            if node.node_id != node_id {
                return false;
            }
            if node.online {
                return true;
            }
            let has_endpoint = node
                .chat_endpoint_id
                .as_deref()
                .is_some_and(|id| !id.trim().is_empty());
            node.presence_status == NODE_PRESENCE_SIGNED_IN && has_endpoint
        })
    }

    fn browsing_can_manage(&self) -> bool {
        if !self.browsing_peer_live() {
            return false;
        }
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
            CreateShareEntryKind::Folder => wormhole_i18n::t("devices.share.creating_folder"),
            CreateShareEntryKind::Txt => wormhole_i18n::t("devices.share.creating_txt"),
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
                        view.share_status = Some(wormhole_i18n::t_args(
                            "devices.share.created",
                            &[("name", &created.name)],
                        ));
                        view.force_load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_status = Some(wormhole_i18n::t_args(
                            "devices.share.create_failed",
                            &[("err", &e)],
                        ));
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
        self.share_rename_feedback = Some((
            StatusTone::Neutral,
            wormhole_i18n::t("devices.share.renaming"),
        ));
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
                        view.share_status = Some(wormhole_i18n::t_args(
                            "devices.share.renamed",
                            &[("name", &name)],
                        ));
                        view.force_load_share_directory(ctx);
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
                Some((StatusTone::Muted, wormhole_i18n::t("devices.share.need_folder_path")));
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
                Some((StatusTone::Muted, wormhole_i18n::t("devices.share.remote_path_hint")));
            ctx.notify();
            return;
        }
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(|| {
                    #[cfg(windows)]
                    {
                        wormhole_desktop_platform_windows::pick_folder(&wormhole_i18n::t("devices.share.pick_folder"))
                    }
                    #[cfg(not(windows))]
                    {
                        rfd::FileDialog::new()
                            .set_title(&wormhole_i18n::t("devices.share.pick_folder"))
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
            self.share_add_feedback = Some((StatusTone::Warn, wormhole_i18n::t("devices.share.need_path")));
            ctx.notify();
            return;
        }
        let Some(node_id) = self.browsing_node_id.clone() else {
            self.share_add_busy = false;
            self.share_add_feedback = Some((
                StatusTone::Warn,
                wormhole_i18n::t("devices.share.no_target"),
            ));
            ctx.notify();
            return;
        };
        self.share_add_busy = true;
        self.share_add_feedback = Some((
            StatusTone::Neutral,
            wormhole_i18n::t("devices.share.adding"),
        ));
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
                        view.share_status = Some(wormhole_i18n::t("devices.share.added"));
                        view.force_load_share_directory(ctx);
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
            requested_app: Some(self.recommended_workspace_app(entry_name)),
        }
    }

    fn run_share_file_action<F>(
        &mut self,
        entry_name: String,
        busy_label: impl Into<String>,
        success_label: impl Into<String>,
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
        self.share_status = Some(busy_label.into());
        self.close_share_context_menu(ctx);
        ctx.notify();
        let core = self.core.clone();
        let success = success_label.into();
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
                        view.force_load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_status = Some(wormhole_i18n::t_args(
                        "devices.share.failed",
                        &[("err", &e)],
                    ));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            wormhole_i18n::t("devices.share.opening"),
            wormhole_i18n::t("devices.share.opened"),
            |state, params| Box::pin(async move { open_share_entry(&state, params).await }),
            ctx,
        );
    }

    fn sync_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            wormhole_i18n::t("devices.share.syncing"),
            wormhole_i18n::t("devices.share.synced"),
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
        self.share_status = Some(wormhole_i18n::t("devices.share.windows_open_progress"));
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
                        view.share_status = Some(wormhole_i18n::t_args(
                            "devices.share.windows_open_ok",
                            &[("name", &entry_name)],
                        ));
                        ctx.emit(DevicesEvent::OpenRemoteDesktop {
                            node_id: target.node_id,
                        });
                    }
                    Err(error) => {
                        view.share_status = Some(wormhole_i18n::t_args(
                            "devices.share.windows_open_failed",
                            &[("err", &error)],
                        ));
                    }
                }
                ctx.notify();
            },
        );
    }

    fn delete_share_file(&mut self, entry_name: String, ctx: &mut ViewContext<Self>) {
        self.run_share_file_action(
            entry_name.clone(),
            wormhole_i18n::t("devices.share.deleting"),
            if self.browsing_can_manage() {
                wormhole_i18n::t("devices.share.deleted")
            } else {
                wormhole_i18n::t("devices.share.deleted_local")
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
        let removable = self
            .cluster
            .as_ref()
            .and_then(|cluster| {
                cluster
                    .nodes
                    .iter()
                    .find(|node| node.node_id == node_id)
                    .map(|node| node.removable)
            })
            .unwrap_or(false);
        if !removable {
            self.status_flash = Some(wormhole_i18n::t("devices.toast.admin_only_remove"));
            self.device_context_menu = None;
            ctx.notify();
            return;
        }
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
                    .unwrap_or_else(|| wormhole_i18n::t("devices.status.cluster_default"))
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

    fn membership_gate_message(cluster: &ClusterStatusDto) -> Option<String> {
        if cluster.clusters.is_empty() {
            return None;
        }
        if cluster.role_stale
            || Self::active_cluster_entry(cluster).is_some_and(|entry| entry.role_stale)
        {
            return Some(wormhole_i18n::t("devices.membership.role_stale"));
        }
        if !Self::active_cluster_can_invite(cluster)
            && Self::active_cluster_entry(cluster).is_some_and(|entry| !entry.revoked)
        {
            return Some(wormhole_i18n::t("devices.membership.not_manager"));
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
                    .map(|id| {
                        wormhole_i18n::t_args(
                            "devices.status.cluster_named",
                            &[("id", &short_cluster_id(id))],
                        )
                    })
                    .unwrap_or_else(|| wormhole_i18n::t("devices.status.cluster_default"))
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
            return wormhole_i18n::t("devices.status.refreshing");
        }
        if self.cluster_syncing {
            return wormhole_i18n::t("devices.status.syncing");
        }
        if cluster.clusters.is_empty() {
            return wormhole_i18n::t("devices.status.empty");
        }
        if let Some(gate) = Self::membership_gate_message(cluster) {
            return gate;
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
                parts.push(wormhole_i18n::t_args(
                    "devices.status.recently_online",
                    &[("n", &signed_in.to_string())],
                ));
            }
            if pending > 0 {
                parts.push(wormhole_i18n::t_args(
                    "devices.status.handshaking",
                    &[("n", &pending.to_string())],
                ));
            }
            if failed > 0 {
                parts.push(wormhole_i18n::t_args(
                    "devices.status.connect_failed",
                    &[("n", &failed.to_string())],
                ));
            }
            if pending > 0 {
                parts.push(wormhole_i18n::t("devices.status.retrying"));
            } else if signed_in > 0 {
                if self.signed_in_peers_slow() {
                    parts.push(wormhole_i18n::t("devices.status.wait_lease"));
                } else {
                    parts.push(wormhole_i18n::t("devices.status.confirming"));
                }
            }
            return parts.join(" · ");
        }
        format!(
            "CLUSTER · {n} NODE{} · {online} ONLINE · E2E ENCRYPTED · {}",
            if n == 1 { "" } else { "S" },
            wormhole_i18n::t("devices.status.click_browse"),
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
                .with_automation_label("刷新集群")
                .with_automation_id("devices:refresh")
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
        let menu_label = label.to_string();
        let inner = Container::new(
            ui_text::cluster_label(menu_label.clone(), self.mono)
                .with_color(color)
                .finish(),
        )
        .with_horizontal_padding(10.0)
        .with_vertical_padding(8.0)
        .finish();
        if enabled {
            EventHandler::new(inner)
                .with_automation_label(menu_label)
                .with_automation_id(format!("devices:cluster_menu:{label}"))
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
        let btn_label = label.to_string();
        let inner = Container::new(
            ConstrainedBox::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_child(
                        ui_text::cluster_ctrl(btn_label.clone(), self.mono)
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
                .with_automation_label(btn_label)
                .with_automation_id(format!("devices:toolbar:{label}"))
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

        let nav_label = if back { "上一级" } else { "下一级" };
        let nav_id = if back {
            "devices:share_back"
        } else {
            "devices:share_forward"
        };
        if enabled {
            EventHandler::new(inner)
                .with_automation_label(nav_label)
                .with_automation_id(nav_id)
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
        let switch_cluster = wormhole_i18n::t("devices.cluster.switch");
        menu.add_child(Self::cluster_menu_section(&switch_cluster, self.mono));
        if cluster.clusters.is_empty() {
            let no_clusters = wormhole_i18n::t("devices.cluster.none");
            menu.add_child(
                Container::new(
                    ui_text::cluster_label(no_clusters, self.mono)
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
            let cluster_name = name.clone();
            item_row.add_child(
                ui_text::cluster_label(cluster_name.clone(), self.mono)
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
            let cluster_name = name.clone();
            menu.add_child(
                EventHandler::new(
                    Container::new(item_row.finish())
                        .with_horizontal_padding(10.0)
                        .with_vertical_padding(8.0)
                        .with_background(bg)
                        .finish(),
                )
                .with_automation_label(cluster_name)
                .with_automation_id(format!("devices:cluster:{cluster_id}"))
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
                    &self.copy_invite_label(),
                    DevicesAction::CopyInvite,
                    false,
                    !self.copy_invite_busy,
                    false,
                ));
                menu.add_child(self.cluster_menu_action(
                    &wormhole_i18n::t("devices.cluster.agent_pair"),
                    DevicesAction::OpenAgentPairModal,
                    false,
                    true,
                    false,
                ));
            } else {
                menu.add_child(self.cluster_menu_action(
                    &wormhole_i18n::t("devices.cluster.copy_invite_needs_member"),
                    DevicesAction::CopyInvite,
                    false,
                    false,
                    false,
                ));
                menu.add_child(self.cluster_menu_action(
                    &wormhole_i18n::t("devices.cluster.rejoin"),
                    DevicesAction::OpenJoinModal,
                    true,
                    true,
                    false,
                ));
            }
        }
        menu.add_child(Self::cluster_menu_divider());
        let create_label = if self.create_cluster_busy {
            wormhole_i18n::t("devices.cluster.creating")
        } else {
            wormhole_i18n::t("devices.toolbar.create")
        };
        menu.add_child(self.cluster_menu_action(
            &create_label,
            DevicesAction::OpenCreateClusterModal,
            true,
            !self.create_cluster_busy,
            false,
        ));
        menu.add_child(self.cluster_menu_action(
            &wormhole_i18n::t("devices.toolbar.join"),
            DevicesAction::OpenJoinModal,
            true,
            true,
            false,
        ));
        if Self::active_cluster_is_owner(cluster) && !Self::active_cluster_is_default(cluster) {
            menu.add_child(Self::cluster_menu_divider());
            menu.add_child(self.cluster_menu_action(
                &wormhole_i18n::t("devices.cluster.delete"),
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
                &wormhole_i18n::t("devices.cluster.leave"),
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
        let picker_label = label.clone();
        trigger_row.add_child(
            Shrinkable::new(
                1.0,
                ui_text::cluster_label(picker_label.clone(), self.mono)
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
        .with_automation_label(picker_label)
        .with_automation_id("devices:cluster_picker")
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
            let create_label = if self.create_cluster_busy {
                wormhole_i18n::t("devices.cluster.creating")
            } else {
                wormhole_i18n::t("devices.toolbar.create")
            };
            row.add_child(self.toolbar_button(
                &create_label,
                DevicesAction::OpenCreateClusterModal,
                true,
                112.0,
                !self.create_cluster_busy,
            ));
            row.add_child(
                Container::new(self.toolbar_button(
                    &wormhole_i18n::t("devices.toolbar.join"),
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
                    &wormhole_i18n::t("devices.cluster.rejoin_short"),
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

    fn copy_invite_label(&self) -> String {
        if self.copy_invite_ack {
            wormhole_i18n::t("devices.invite.copied")
        } else if self.copy_invite_busy {
            wormhole_i18n::t("devices.invite.copying")
        } else {
            wormhole_i18n::t("devices.invite.copy")
        }
    }

    fn apply_copy_invite_success(&mut self, invite: String, ctx: &mut ViewContext<Self>) {
        self.cached_invite = cached_cluster_invite_from_json(&invite).ok();
        let name = self
            .cluster
            .as_ref()
            .map(DevicesView::cluster_label)
            .unwrap_or_else(|| wormhole_i18n::t("devices.status.cluster_default"));
        self.status_flash = Some(wormhole_i18n::t_args(
            "devices.invite.copied_flash",
            &[("name", &name)],
        ));
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
                    Self::membership_gate_message(cluster).unwrap_or_else(|| {
                        wormhole_i18n::t("devices.cluster.not_member")
                    }),
                );
                self.cluster_picker_open = false;
                ctx.notify();
                return;
            }
        }
        self.copy_invite_ack = false;
        self.copy_invite_busy = true;
        self.status_flash = Some(wormhole_i18n::t("devices.invite.generating"));
        ctx.notify();

        let core = self.core.clone();
        let cluster_id = self.cluster.as_ref().and_then(Self::active_cluster_id);
        let cached = self.cached_invite.clone();
        ctx.spawn(
            async move {
                let cluster_id = cluster_id.ok_or_else(|| wormhole_i18n::t("devices.cluster.none_selected"))?;
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
                            view.status_flash = Some(wormhole_i18n::t_args(
                            "devices.invite.copy_failed",
                            &[("err", &e)],
                        ));
                            view.join_feedback = Some((StatusTone::Danger, e));
                            ctx.notify();
                        }
                    },
                    Err(e) => {
                        view.cached_invite = None;
                        view.status_flash = Some(wormhole_i18n::t_args(
                            "devices.invite.gen_failed",
                            &[("err", &e)],
                        ));
                        view.join_feedback = Some((StatusTone::Danger, e));
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn open_agent_pair_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.agent_pair_modal_open = true;
        self.agent_pair = None;
        self.agent_pair_error = None;
        self.agent_pair_qr_asset_id = None;
        self.agent_pair_copy_ack = false;
        self.cluster_picker_open = false;
        ctx.notify();
        self.create_agent_pair(ctx);
    }

    fn close_agent_pair_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.agent_pair_modal_open = false;
        self.agent_pair_busy = false;
        self.agent_pair_error = None;
        self.agent_pair_copy_ack = false;
        ctx.notify();
    }

    fn create_agent_pair(&mut self, ctx: &mut ViewContext<Self>) {
        if self.agent_pair_busy {
            return;
        }
        self.agent_pair_busy = true;
        self.agent_pair_error = None;
        self.agent_pair = None;
        self.agent_pair_qr_asset_id = None;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                create_agent_pair_code(&state, CreateAgentPairCodeParams { ttl_secs: None }).await
            },
            |view, output, ctx| {
                view.agent_pair_busy = false;
                match output {
                    Ok(pair) => view.apply_agent_pair_success(pair, ctx),
                    Err(err) => {
                        view.agent_pair_error = Some(err);
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn apply_agent_pair_success(&mut self, pair: AgentPairCodeDto, ctx: &mut ViewContext<Self>) {
        let asset_id = format!("wormhole-agent-pair-qr-{}", pair.code);
        match base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            pair.qr_png_base64.as_bytes(),
        ) {
            Ok(png_bytes) => match decode_image_asset_payload(png_bytes) {
                Ok(payload) => {
                    AssetCache::handle(ctx).update(ctx, |cache, model_ctx| {
                        cache.insert_raw_asset_bytes::<ImageType>(
                            asset_id.clone(),
                            &payload,
                            model_ctx,
                        );
                    });
                    self.agent_pair_qr_asset_id = Some(asset_id);
                }
                Err(err) => {
                    tracing::warn!("agent pair qr decode failed: {err}");
                    self.agent_pair_qr_asset_id = None;
                }
            },
            Err(err) => {
                tracing::warn!("agent pair qr base64 failed: {err}");
                self.agent_pair_qr_asset_id = None;
            }
        }
        self.agent_pair = Some(pair);
        self.agent_pair_error = None;
        ctx.notify();
    }

    fn copy_agent_pair_url(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(url) = self.agent_pair.as_ref().map(|pair| pair.pair_url.clone()) else {
            return;
        };
        match write_clipboard_text(&url) {
            Ok(()) => {
                self.agent_pair_copy_ack = true;
                self.status_flash =
                    Some(wormhole_i18n::t("devices.modal.agent_pair_copied_flash"));
                ctx.notify();
                ctx.spawn(
                    async move {
                        tokio::time::sleep(Duration::from_millis(2600)).await;
                    },
                    |view, _, ctx| {
                        view.agent_pair_copy_ack = false;
                        view.status_flash = None;
                        ctx.notify();
                    },
                );
            }
            Err(err) => {
                self.agent_pair_error = Some(err);
                ctx.notify();
            }
        }
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
            self.create_cluster_feedback = Some((
                StatusTone::Danger,
                wormhole_i18n::t("devices.cluster.need_name"),
            ));
            self.create_cluster_name_focused = true;
            sync_caret_blink(self, ctx);
            ctx.notify();
            return;
        }
        if name.chars().count() > 128 {
            self.create_cluster_feedback = Some((
                StatusTone::Danger,
                wormhole_i18n::t("devices.cluster.name_too_long"),
            ));
            self.create_cluster_name_focused = true;
            sync_caret_blink(self, ctx);
            ctx.notify();
            return;
        }
        self.cluster_picker_open = false;
        self.create_cluster_busy = true;
        self.status_flash = Some(wormhole_i18n::t("devices.cluster.creating_flash"));
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
                .map_err(|_| wormhole_i18n::t("devices.cluster.create_timeout"))?
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
                        view.status_flash = Some(wormhole_i18n::t("devices.cluster.created"));
                        sync_caret_blink(view, ctx);
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.create_cluster_feedback = Some((StatusTone::Danger, e.clone()));
                        view.status_flash = Some(wormhole_i18n::t_args(
                            "devices.cluster.create_failed",
                            &[("err", &e)],
                        ));
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
        self.clear_device_search();
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
            self.status_flash = Some(wormhole_i18n::t("devices.cluster.none_selected"));
            ctx.notify();
            return;
        };
        self.cluster_picker_open = false;
        self.apply_optimistic_remove_cluster(&cluster_id, ctx);
        self.status_flash = Some(wormhole_i18n::t("devices.cluster.left"));
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
                        view.status_flash = Some(wormhole_i18n::t("devices.cluster.left"));
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.status_flash = Some(wormhole_i18n::t_args(
                            "devices.cluster.leave_failed",
                            &[("err", &e)],
                        ));
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
            self.status_flash = Some(wormhole_i18n::t("devices.cluster.cannot_delete_default"));
            self.cluster_picker_open = false;
            ctx.notify();
            return;
        }
        if !Self::active_cluster_is_owner(cluster) {
            self.status_flash = Some(wormhole_i18n::t("devices.cluster.owner_only_delete"));
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
                    .unwrap_or_else(|| wormhole_i18n::t("devices.cluster.owner_only_delete")),
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
            .unwrap_or_else(|| wormhole_i18n::t("devices.status.cluster_default"));
        let Some(cluster_id) = cluster_id else {
            self.status_flash = Some(wormhole_i18n::t("devices.cluster.none_selected"));
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
                        view.status_flash = Some(wormhole_i18n::t_args(
                            "devices.cluster.delete_failed",
                            &[("err", &e)],
                        ));
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
            self.status_flash = Some(wormhole_i18n::t("devices.cluster.none_selected"));
            ctx.notify();
            return;
        };
        let removing_server_member = device_id.is_some();
        self.apply_optimistic_remove_node(&node_id, ctx);
        self.status_flash = Some(if removing_server_member {
            wormhole_i18n::t("devices.device.removed")
        } else {
            wormhole_i18n::t("devices.grid.hidden_offline")
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
                            wormhole_i18n::t("devices.device.removed")
                        } else {
                            wormhole_i18n::t("devices.grid.hidden_offline")
                        });
                    }
                    Err(e) => {
                        view.cluster_error = Some(e.clone());
                        view.status_flash = Some(wormhole_i18n::t_args(
                            "devices.device.remove_failed",
                            &[("err", &e)],
                        ));
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
            self.join_feedback = Some((StatusTone::Warn, wormhole_i18n::t("devices.invite.need_paste")));
            ctx.notify();
            return;
        }
        self.invite_busy = true;
        self.join_feedback = Some((
            StatusTone::Neutral,
            wormhole_i18n::t("devices.invite.joining"),
        ));
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
                            JoinClusterOutcome::Joined => wormhole_i18n::t_args(
                                "devices.invite.joined",
                                &[("name", &cluster_label)],
                            ),
                            JoinClusterOutcome::AlreadyActive => wormhole_i18n::t("devices.invite.already"),
                            JoinClusterOutcome::SwitchedActive => {
                                wormhole_i18n::t_args(
                                    "devices.invite.switched",
                                    &[("name", &cluster_label)],
                                )
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
            &wormhole_i18n::t("devices.cluster.name_placeholder"),
            self.font,
            self.create_cluster_name_focused,
            false,
            self.caret_blink.visible,
            self.create_cluster_name_field.cursor,
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
            ui_text::title(wormhole_i18n::t("devices.modal.create_title"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("devices.modal.create_body"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            ui_text::hud_title(wormhole_i18n::t("devices.modal.cluster_name"), self.font)
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
            &wormhole_i18n::t("common.cancel"),
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
        let submit_label = if self.create_cluster_busy {
            wormhole_i18n::t("devices.cluster.creating")
        } else {
            wormhole_i18n::t("devices.cluster.create_btn")
        };
        actions.add_child(self.toolbar_button(
            &submit_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭创建集群")
            .with_automation_id("devices:scrim_create_cluster")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseCreateClusterModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn join_modal(&self) -> Box<dyn Element> {
        let join_preview = if self.join_invite_draft.is_empty() {
            wormhole_i18n::t("devices.modal.join_paste_preview")
        } else {
            truncate_middle(&self.join_invite_draft, 240)
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title(wormhole_i18n::t("devices.modal.join_title"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("devices.modal.join_body"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::hud_title(wormhole_i18n::t("devices.modal.join_paste_title"), self.font)
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
        let paste_clipboard = wormhole_i18n::t("devices.modal.paste_clipboard");
        dialog.add_child(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(
                        Expanded::new(
                            1.0,
                            self.toolbar_button(
                                &paste_clipboard,
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
        actions.add_child(            self.toolbar_button(
            &wormhole_i18n::t("common.cancel"),
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
        let join_label = if self.invite_busy {
            wormhole_i18n::t("devices.invite.joining")
        } else {
            wormhole_i18n::t("devices.toolbar.join")
        };
        actions.add_child(self.toolbar_button(
            &join_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭加入集群")
            .with_automation_id("devices:scrim_join")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseJoinModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn agent_pair_modal(&self) -> Box<dyn Element> {
        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title(wormhole_i18n::t("devices.modal.agent_pair_title"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("devices.modal.agent_pair_body"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );

        if let Some(pair) = &self.agent_pair {
            let ready = if pair.agent_mode_enabled && pair.agentd_running {
                wormhole_i18n::t("devices.modal.agent_pair_ready")
            } else if !pair.agent_mode_enabled {
                wormhole_i18n::t("devices.modal.agent_pair_need_mode")
            } else {
                wormhole_i18n::t("devices.modal.agent_pair_need_agentd")
            };
            dialog.add_child(status_line(
                ready,
                self.font,
                if pair.agent_mode_enabled && pair.agentd_running {
                    StatusTone::Success
                } else {
                    StatusTone::Warn
                },
            ));
            if let Some(asset_id) = &self.agent_pair_qr_asset_id {
                dialog.add_child(
                    Container::new(
                        ConstrainedBox::new(
                            Image::new(
                                AssetSource::Raw {
                                    id: asset_id.clone(),
                                },
                                CacheOption::BySize,
                            )
                            .finish(),
                        )
                        .with_width(220.0)
                        .with_height(220.0)
                        .finish(),
                    )
                    .with_vertical_margin(12.0)
                    .finish(),
                );
            }
            dialog.add_child(
                Container::new(
                    ui_text::mono(truncate_middle(&pair.pair_url, 64), self.mono)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_uniform_padding(10.0)
                .with_vertical_margin(8.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .finish(),
            );
        } else if self.agent_pair_busy {
            dialog.add_child(status_line(
                wormhole_i18n::t("devices.modal.agent_pair_creating"),
                self.font,
                StatusTone::Muted,
            ));
        }

        if let Some(err) = &self.agent_pair_error {
            dialog.add_child(status_line(err.clone(), self.font, StatusTone::Danger));
        }

        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.toolbar_button(
            &wormhole_i18n::t("common.cancel"),
            DevicesAction::CloseAgentPairModal,
            false,
            72.0,
            true,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        if self.agent_pair.is_some() {
            let copy_label = if self.agent_pair_copy_ack {
                wormhole_i18n::t("devices.modal.agent_pair_copied")
            } else {
                wormhole_i18n::t("devices.modal.agent_pair_copy")
            };
            actions.add_child(self.toolbar_button(
                &copy_label,
                DevicesAction::CopyAgentPairUrl,
                true,
                120.0,
                true,
            ));
        } else {
            actions.add_child(self.toolbar_button(
                &wormhole_i18n::t("devices.modal.agent_pair_retry"),
                DevicesAction::CreateAgentPair,
                true,
                100.0,
                !self.agent_pair_busy,
            ));
        }
        dialog.add_child(
            Container::new(actions.finish())
                .with_vertical_margin(12.0)
                .finish(),
        );

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
        .with_automation_label("手机连本机 Agent")
        .with_automation_id("devices:agent_pair_dialog")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭手机连本机")
            .with_automation_id("devices:scrim_agent_pair")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::CloseAgentPairModal);
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn grid_view(&self) -> Box<dyn Element> {
        let mut header = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        header.add_child(section_title(wormhole_i18n::t("devices.cluster_nodes"), self.mono));

        if let Some(err) = &self.cluster_error {
            header.add_child(section_hint(
                wormhole_i18n::t("devices.bootstrap.cluster_offline"),
                self.font,
            ));
            header.add_child(status_line(err.clone(), self.font, StatusTone::Danger));
        } else if let Some(cluster) = &self.cluster {
            if cluster.auth_required {
                header.add_child(section_hint(
                    wormhole_i18n::t("devices.cluster.need_login"),
                    self.font,
                ));
                header.add_child(status_line(
                    wormhole_i18n::t("devices.bootstrap.login_for_p2p"),
                    self.font,
                    StatusTone::Placeholder,
                ));
            } else if cluster.device_bootstrap_required {
                header.add_child(section_hint(
                    wormhole_i18n::t("devices.bootstrap.restoring_identity"),
                    self.font,
                ));
                if let Some(err) = &cluster.device_bootstrap_error {
                    header.add_child(status_line(err.clone(), self.font, StatusTone::Danger));
                } else {
                    header.add_child(status_line(
                        wormhole_i18n::t("devices.bootstrap.restoring_progress"),
                        self.font,
                        StatusTone::Placeholder,
                    ));
                    if self.bootstrap_pending_slow(cluster) {
                        header.add_child(status_line(
                            wormhole_i18n::t("devices.bootstrap.waiting_control_plane"),
                            self.font,
                            StatusTone::Placeholder,
                        ));
                    }
                }
                let bootstrap_label = if self.bootstrap_busy {
                    wormhole_i18n::t("common.loading")
                } else {
                    wormhole_i18n::t("common.retry")
                };
                header.add_child(
                    Container::new(self.toolbar_button(
                        &bootstrap_label,
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
                header.add_child(
                    Container::new(self.device_search_box())
                        .with_margin_top(4.0)
                        .finish(),
                );
            }
        } else {
            header.add_child(section_hint("CLUSTER · LOADING", self.font));
            header.add_child(status_line(
                wormhole_i18n::t("common.loading"),
                self.font,
                StatusTone::Placeholder,
            ));
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
                                    wormhole_i18n::t("devices.bootstrap.after_login_hint")
                                } else if cluster.device_bootstrap_error.is_some() {
                                    wormhole_i18n::t("devices.bootstrap.restore_failed_hint")
                                } else if self.bootstrap_pending_slow(cluster) {
                                    wormhole_i18n::t("devices.bootstrap.slow_hint")
                                } else {
                                    wormhole_i18n::t("devices.bootstrap.will_sync_hint")
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
                    ui_text::title(wormhole_i18n::t("devices.cluster.none"), self.font)
                        .with_color(theme::text())
                        .finish(),
                );
                empty.add_child(
                    Container::new(
                        ui_text::body(
                            wormhole_i18n::t("devices.bootstrap.empty_cluster_cta"),
                            self.font,
                        )
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
                let selected_id = self.selected_node_id.clone().unwrap_or_default();
                let hovered_id = self.hovered_node_id.clone().unwrap_or_default();
                let mut nodes = cluster.nodes.clone();
                nodes.sort_by(|a, b| {
                    let a_local = a.node_id == local_id;
                    let b_local = b.node_id == local_id;
                    b_local
                        .cmp(&a_local)
                        .then_with(|| a.hostname.cmp(&b.hostname))
                });
                nodes.retain(|n| {
                    let remark = self.device_remarks.get(&n.node_id).map(String::as_str);
                    device_matches_query(n, n.node_id == local_id, remark, &self.device_search)
                });
                if nodes.is_empty() {
                    let mut empty = Flex::column()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_main_axis_size(MainAxisSize::Max);
                    empty.add_child(
                        ui_text::body(wormhole_i18n::t("devices.search.empty"), self.font)
                            .with_color(theme::muted())
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
                    let hub_index = nodes
                        .iter()
                        .position(|n| n.node_id == local_id)
                        .unwrap_or(0);
                    col.add_child(
                        Expanded::new(
                            1.0,
                            Container::new(ClusterTopologyPanel::element(
                                nodes,
                                local_id,
                                selected_id,
                                hovered_id,
                                hub_index,
                                self.device_remarks.clone(),
                                self.mono,
                                self.topology_scroll.clone(),
                            ))
                            .with_background(theme::panel())
                            .finish(),
                        )
                        .finish(),
                    );
                }
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
            || self.agent_pair_modal_open
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
            && !self.agent_pair_modal_open
            && self.delete_modal_node_id.is_none()
            && self.delete_modal_cluster_id.is_none()
            && self.device_context_menu.is_none()
        {
            EventHandler::new(self.grid_view())
                .with_automation_label("关闭集群选择器")
                .with_automation_id("devices:close_cluster_picker")
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
            .with_automation_label("关闭设备菜单")
            .with_automation_id("devices:scrim_device_menu")
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
        if self.agent_pair_modal_open {
            stack.add_child(self.agent_pair_modal());
        }
        let delete_node_modal_open = self.delete_modal_node_id.is_some();
        let delete_cluster_modal_open = self.delete_modal_cluster_id.is_some();
        let device_menu_open = self.device_context_menu.is_some();
        let join_modal_open = self.join_modal_open;
        let agent_pair_modal_open = self.agent_pair_modal_open;
        let create_cluster_modal_open = self.create_cluster_modal_open;
        let cluster_picker_open = self.cluster_picker_open;
        EventHandler::new(stack.finish())
            .with_automation_label("关闭浮层")
            .with_automation_id("devices:overlay_dismiss")
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
                } else if agent_pair_modal_open {
                    ctx.dispatch_typed_action(DevicesAction::CloseAgentPairModal);
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
            wormhole_i18n::t("devices.modal.delete_cluster_owner_body")
        } else {
            wormhole_i18n::t("devices.modal.delete_cluster_member_body")
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title(wormhole_i18n::t("devices.cluster.delete"), self.font)
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
            &wormhole_i18n::t("common.cancel"),
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
        let delete_label = if self.delete_cluster_busy {
            wormhole_i18n::t("devices.cluster.deleting")
        } else {
            wormhole_i18n::t("common.delete")
        };
        actions.add_child(self.toolbar_button(
            &delete_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭删除集群")
            .with_automation_id("devices:scrim_delete_cluster")
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
            ui_text::title(wormhole_i18n::t("devices.action.remove"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("devices.modal.remove_device_body"),
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
        actions.add_child(            self.toolbar_button(
            &wormhole_i18n::t("common.cancel"),
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
            &wormhole_i18n::t("devices.action.remove"),
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭删除设备")
            .with_automation_id("devices:scrim_delete_node")
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
        let browse_shares = wormhole_i18n::t("devices.share.shared_folders");
        let rdp_label = wormhole_i18n::t("devices.action.rdp");
        let chat_cannot_self = wormhole_i18n::t("devices.chat.cannot_self");
        let chat_message = wormhole_i18n::t("devices.action.message");
        let remove_device = wormhole_i18n::t("devices.action.remove");

        let mut menu = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        menu.add_child(self.device_context_item(
            &browse_shares,
            DevicesAction::OpenNode(node_id.clone()),
            ContextItemStyle::Normal,
            self.cluster.as_ref().is_some_and(|cluster| {
                cluster
                    .nodes
                    .iter()
                    .find(|node| node.node_id == node_id)
                    .is_some_and(|node| {
                        crate::ui::cluster_topology_panel::node_share_browsable(node, is_local)
                    })
            }),
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
                &rdp_label,
                DevicesAction::OpenRemoteDesktop(node_id.clone()),
                ContextItemStyle::Normal,
                remote_available,
            ));
        }
        menu.add_child(self.device_context_item(
            if is_local {
                chat_cannot_self.as_str()
            } else {
                chat_message.as_str()
            },
            DevicesAction::SendMessage(node_id.clone()),
            ContextItemStyle::Accent,
            !is_local,
        ));
        let can_remove = self.cluster.as_ref().is_some_and(|cluster| {
            cluster
                .nodes
                .iter()
                .find(|node| node.node_id == node_id)
                .is_some_and(|node| node.removable)
        });
        let cannot_delete_self = wormhole_i18n::t("devices.device.cannot_delete_self");
        let no_permission_remove = wormhole_i18n::t("devices.device.no_permission_remove");
        menu.add_child(Self::cluster_menu_divider());
        menu.add_child(self.device_context_item(
            if is_local {
                cannot_delete_self.as_str()
            } else if can_remove {
                remove_device.as_str()
            } else {
                no_permission_remove.as_str()
            },
            DevicesAction::OpenDeleteNodeModal(node_id),
            ContextItemStyle::Danger,
            can_remove,
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
        let item_label = label.to_string();
        let handler = EventHandler::new(
            Container::new(
                ui_text::body(item_label.clone(), self.font)
                    .with_color(color)
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .finish(),
        );
        if enabled {
            handler
                .with_automation_label(item_label)
                .with_automation_id(format!("devices:context:{label}"))
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
            return format!("VAULT · {}", wormhole_i18n::t("devices.share.reading"));
        }
        let count = self.share_entries.len();
        if let Some(err) = &self.share_error {
            return format!("VAULT · {err}");
        }
        if !self.browsing_local() && !self.browsing_peer_live() {
            return wormhole_i18n::t_args(
            "devices.share.vault_offline",
            &[("count", &count.to_string())],
        );
        }
        wormhole_i18n::t_args("devices.share.vault_count", &[("count", &count.to_string())])
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
        if !is_folder && !entry.local && self.browsing_peer_live() {
            let sync_name = name.clone();
            name_row.add_child(
                EventHandler::new(
                    Container::new(icons::share_sync_icon(self.share_file_busy))
                        .with_horizontal_margin(4.0)
                        .finish(),
                )
                .with_automation_label("同步文件")
                .with_automation_id(format!("devices:share_sync:{sync_name}"))
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
                    ui_text::cluster_ctrl(wormhole_i18n::t("devices.share.share_badge"), self.mono)
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
            let folder_name = name.clone();
            let folder_id = format!("devices:share_folder:{folder_name}");
            EventHandler::new(inner)
                .with_automation_label(folder_name)
                .with_automation_id(folder_id)
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
                .with_automation_label(click_name.clone())
                .with_automation_id(format!("devices:share_file:{click_name}"))
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
        for (label_key, weight) in [
            ("devices.share.column.name", 0.48),
            ("devices.share.column.modified", 0.22),
            ("devices.share.column.type", 0.18),
            ("devices.share.column.size", 0.12),
        ] {
            row.add_child(
                Shrinkable::new(
                    weight,
                    Container::new(
                        ui_text::hud_title(wormhole_i18n::t(label_key), self.font)
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

    fn share_empty_hint(&self) -> String {
        if self.share_loading {
            wormhole_i18n::t("devices.share.reading")
        } else if self.browsing_local() && self.share_path.is_empty() {
            wormhole_i18n::t("devices.share.empty_local")
        } else if !self.browsing_local() && self.share_path.is_empty() {
            if self.browsing_peer_live() {
                wormhole_i18n::t("devices.share.empty_remote")
            } else {
                wormhole_i18n::t("devices.share.empty_offline_root")
            }
        } else if !self.browsing_local() && !self.browsing_peer_live() {
            wormhole_i18n::t("devices.share.empty_offline_path")
        } else {
            wormhole_i18n::t("common.empty.folder")
        }
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
                    ui_text::cluster_ctrl(wormhole_i18n::t("devices.share.back_to_devices"), self.mono)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_horizontal_padding(4.0)
                .finish(),
            )
            .with_automation_label(wormhole_i18n::t("devices.share.back_to_devices"))
            .with_automation_id("devices:back_to_grid")
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
                    &wormhole_i18n::t("devices.share.new"),
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
        file_list
    }

    fn share_add_modal(&self) -> Box<dyn Element> {
        let path_preview = if self.share_add_path.is_empty() {
            wormhole_i18n::t_args(
                "devices.share.path_example",
                &[("path", &default_share_browse_path())],
            )
        } else {
            self.share_add_path.clone()
        };

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title(wormhole_i18n::t("devices.share.shared_folders"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("devices.share.add_hint"),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_vertical_margin(8.0)
            .finish(),
        );
        dialog.add_child(
            ui_text::hud_title(wormhole_i18n::t("devices.share.local_path"), self.font)
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
                &wormhole_i18n::t("toolbox.user_apps.browse"),
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
        actions.add_child(            self.toolbar_button(
            &wormhole_i18n::t("common.cancel"),
            DevicesAction::CloseShareAddModal,
            false,
            88.0,
            true,
        ));
        let share_add_label = if self.share_add_busy {
            wormhole_i18n::t("devices.share.adding")
        } else {
            wormhole_i18n::t("common.add")
        };
        actions.add_child(
            Container::new(self.toolbar_button(
                &share_add_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭共享添加")
            .with_automation_id("devices:scrim_share_add")
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
        let needs_sync =
            !browsing_local && self.browsing_peer_live() && !is_folder && !has_local_replica;
        let can_remote = !browsing_local && !is_folder && self.browsing_peer_live();
        let can_rename = can_manage;
        let can_delete = if can_manage {
            true
        } else {
            !is_folder && has_local_replica
        };

        let open_folder_label = wormhole_i18n::t("devices.share.open_folder");
        let sync_label = wormhole_i18n::t("devices.share.sync");
        let remote_open_label = wormhole_i18n::t("devices.share.remote_open");
        let remote_vm_label = wormhole_i18n::t("devices.share.remote_vm_open");
        let rename_label = wormhole_i18n::t("devices.share.rename");
        let delete_label = wormhole_i18n::t("common.delete");
        let unshare_label = wormhole_i18n::t("devices.share.unshare");

        let open_file_label = wormhole_i18n::t("common.open");
        let open_label = if is_folder {
            open_folder_label.as_str()
        } else {
            open_file_label.as_str()
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
            &sync_label,
            Some(DevicesAction::ShareSyncFile(name.clone())),
            false,
            needs_sync,
        ));
        menu.add_child(self.share_context_item(
            &remote_open_label,
            Some(DevicesAction::ShareRemoteOpenOnHost(name.clone())),
            false,
            can_remote,
        ));
        menu.add_child(self.share_context_item(
            &remote_vm_label,
            Some(DevicesAction::ShareRemoteOpenFile(name.clone())),
            false,
            can_remote,
        ));
        menu.add_child(self.share_context_item(
            &rename_label,
            Some(DevicesAction::OpenShareRenameModal(name.clone())),
            false,
            can_rename,
        ));
        menu.add_child(self.share_context_item(
            &delete_label,
            Some(DevicesAction::ShareDeleteFile(name.clone())),
            true,
            can_delete,
        ));
        menu.add_child(self.share_context_item(
            &unshare_label,
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
            ui_text::title(wormhole_i18n::t("devices.share.unshare"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("devices.share.unshare_confirm_body"),
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
                    wormhole_i18n::t("devices.share.unshare_body"),
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
        let back_label = wormhole_i18n::t("common.back");
        actions.add_child(self.toolbar_button(
            &back_label,
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
        let unshare_label = if self.share_unshare_busy {
            wormhole_i18n::t("common.loading")
        } else {
            wormhole_i18n::t("devices.share.unshare")
        };
        actions.add_child(self.toolbar_button(
            &unshare_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭取消共享")
            .with_automation_id("devices:scrim_share_unshare")
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
                        view.share_status = Some(wormhole_i18n::t_args(
                            "devices.share.unshared_kept",
                            &[("name", &name)],
                        ));
                        view.force_load_share_directory(ctx);
                    }
                    Err(e) => {
                        view.share_error = Some(wormhole_i18n::t_args(
                            "devices.share.unshare_failed",
                            &[("err", &e)],
                        ));
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
                let item_label = label.to_string();
                EventHandler::new(inner)
                    .with_automation_label(item_label)
                    .with_automation_id(format!("devices:share_menu:{label}"))
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
        let new_label = label.to_string();
        EventHandler::new(
            Container::new(row.finish())
                .with_uniform_padding(10.0)
                .finish(),
        )
        .with_automation_label(new_label)
        .with_automation_id(format!("devices:share_new:{label}"))
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
                let shared_folders = wormhole_i18n::t("devices.share.shared_folders");
                menu.add_child(self.share_new_menu_item(
                    &shared_folders,
                    true,
                    DevicesAction::OpenShareAddModal,
                ));
            }
            ShareNewMenuMode::CreateEntries => {
                let new_folder = wormhole_i18n::t("devices.share.new_folder");
                let new_txt = wormhole_i18n::t("devices.share.new_txt");
                menu.add_child(self.share_new_menu_item(
                    &new_folder,
                    true,
                    DevicesAction::ShareCreateFolder,
                ));
                menu.add_child(self.share_new_menu_item(
                    &new_txt,
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
            .with_automation_label("共享新建菜单")
            .with_automation_id("devices:share_new_menu")
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
        let rename_placeholder = wormhole_i18n::t("devices.share.rename_placeholder");
        let field = render_field_with_caret(
            &draft,
            &marked,
            &rename_placeholder,
            self.font,
            self.share_rename_focused,
            false,
            self.caret_blink.visible,
            self.share_rename_field.cursor,
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
            ui_text::title(wormhole_i18n::t("devices.share.rename"), self.font)
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
        actions.add_child(            self.toolbar_button(
            &wormhole_i18n::t("common.cancel"),
            DevicesAction::CloseShareRenameModal,
            false,
            88.0,
            true,
        ));
        let confirm_rename_label = if self.share_rename_busy {
            wormhole_i18n::t("common.saving")
        } else {
            wormhole_i18n::t("common.ok")
        };
        actions.add_child(
            Container::new(self.toolbar_button(
                &confirm_rename_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = Container::new(
            Align::new(Container::new(panel).with_uniform_padding(24.0).finish()).finish(),
        )
        .with_background(ColorU::new(8, 7, 11, 180))
        .finish();

        EventHandler::new(scrim)
            .with_automation_label("关闭重命名")
            .with_automation_id("devices:scrim_share_rename")
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
            .with_automation_label("关闭新建菜单")
            .with_automation_id("devices:scrim_share_new")
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
            .with_automation_label("关闭共享菜单")
            .with_automation_id("devices:scrim_share_context")
            .on_left_mouse_down(|ctx, _, _| {
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
                .with_automation_label("关闭共享浮层")
                .with_automation_id("devices:share_overlay_dismiss")
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

    fn workspace_app_picker_modal(&self) -> Box<dyn Element> {
        let file_name = self
            .selected_share_entry()
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| wormhole_i18n::t("devices.workspace.this_file"));
        let installed = self.workspace_installed_apps_for_file(&file_name);
        let query = self.workspace_app_search.trim().to_ascii_lowercase();

        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        dialog.add_child(
            ui_text::title(wormhole_i18n::t("devices.workspace.picker_title"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        dialog.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("devices.workspace.picker_body"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );
        if let Some(hint) = self.workspace_picker_host_native_hint(&file_name) {
            dialog.add_child(
                Container::new(
                    ui_text::body(hint, self.font)
                        .with_color(theme::warn())
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        dialog.add_child(
            Container::new(
                ui_text::mono(file_name.clone(), self.mono)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_top(14.0)
            .with_margin_bottom(4.0)
            .with_uniform_padding(10.0)
            .with_horizontal_padding(12.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish(),
        );

        let mut section_head = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max);
        section_head.add_child(
            ui_text::cluster_ctrl(wormhole_i18n::t("devices.workspace.installed_apps"), self.mono)
                .with_color(theme::muted())
                .finish(),
        );
        section_head.add_child(
            ui_text::cluster_ctrl(
                wormhole_i18n::t_args(
                    "common.count_n",
                    &[("n", &installed.len().to_string())],
                ),
                self.mono,
            )
                .with_color(theme::muted())
                .finish(),
        );
        dialog.add_child(
            Container::new(section_head.finish())
                .with_margin_top(14.0)
                .with_margin_bottom(8.0)
                .finish(),
        );

        let mut app_list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if installed.is_empty() {
            app_list.add_child(
                ui_text::body(wormhole_i18n::t("devices.workspace.no_ready_apps"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        } else {
            for app in &installed {
                app_list.add_child(
                    Container::new(self.workspace_app_option_row(
                        app,
                        *app == self.workspace_selected_app.as_str(),
                    ))
                    .with_margin_bottom(8.0)
                    .finish(),
                );
            }
        }
        dialog.add_child(app_list.finish());

        dialog.add_child(
            Container::new(
                ui_text::cluster_ctrl(wormhole_i18n::t("devices.workspace.search_apps"), self.mono)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(16.0)
            .finish(),
        );

        let marked = self.workspace_app_search_field.marked_text.clone();
        let search_placeholder = wormhole_i18n::t("devices.workspace.search_placeholder");
        let search_field = render_field_with_caret(
            &self.workspace_app_search,
            &marked,
            &search_placeholder,
            self.font,
            self.workspace_app_search_focused,
            false,
            self.caret_blink.visible,
            self.workspace_app_search_field.cursor,
        );
        let search_input = wrap_text_field_focus_on_click(
            TextFieldInput::builder(search_field, |ctx, action| {
                ctx.dispatch_typed_action(DevicesAction::WorkspaceAppSearchEdit(action));
            })
            .focused(self.workspace_app_search_focused)
            .ime_preedit(!marked.is_empty())
            .on_keydown(|ctx, keystroke| match keystroke.key.as_str() {
                "escape" => {
                    ctx.dispatch_typed_action(DevicesAction::CloseWorkspaceAppPicker);
                    DispatchEventResult::StopPropagation
                }
                _ => DispatchEventResult::PropagateToParent,
            })
            .finish(),
            |ctx| ctx.dispatch_typed_action(DevicesAction::FocusWorkspaceAppSearch),
        );
        dialog.add_child(
            Container::new(search_input)
                .with_margin_top(8.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(if self.workspace_app_search_focused {
                    theme::accent_cool()
                } else {
                    theme::border()
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .with_uniform_padding(8.0)
                .with_horizontal_padding(11.0)
                .finish(),
        );

        let mut search_results =
            Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if query.is_empty() {
            search_results.add_child(
                Container::new(
                    ui_text::body(
                        wormhole_i18n::t("devices.workspace.search_hint"),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            let mut matches: Vec<String> = self
                .workspace_tools
                .iter()
                .filter(|tool| {
                    let haystack = format!(
                        "{} {}",
                        tool.descriptor.name, tool.descriptor.description
                    )
                    .to_ascii_lowercase();
                    haystack.contains(&query)
                })
                .map(|tool| workspace_canonical_tool_id(&tool.descriptor.id))
                .collect();
            for app in &self.workspace_user_apps {
                let haystack = format!("{} {}", app.display_name, app.app_id).to_ascii_lowercase();
                if haystack.contains(&query)
                    && user_app_supports_file(&app.extensions, &file_name)
                    && !matches
                        .iter()
                        .any(|id| id.eq_ignore_ascii_case(&app.app_id))
                {
                    matches.push(app.app_id.clone());
                }
            }
            for app in &self.workspace_catalog_user_apps {
                if !app.package_ready {
                    continue;
                }
                let haystack = format!("{} {}", app.display_name, app.app_id).to_ascii_lowercase();
                if haystack.contains(&query)
                    && user_app_supports_file(&app.extensions, &file_name)
                    && !matches
                        .iter()
                        .any(|id| id.eq_ignore_ascii_case(&app.app_id))
                {
                    matches.push(app.app_id.clone());
                }
            }
            if matches.is_empty() {
                search_results.add_child(
                    Container::new(
                        ui_text::body(wormhole_i18n::t("devices.workspace.search_empty"), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                );
            } else {
                for app_id in matches {
                    let is_installed = installed.iter().any(|app| app == &app_id);
                    search_results.add_child(
                        Container::new(self.workspace_app_search_row(&app_id, is_installed))
                            .with_margin_top(6.0)
                            .finish(),
                    );
                }
            }
        }
        dialog.add_child(
            Container::new(
                ConstrainedBox::new(search_results.finish())
                    .with_max_height(170.0)
                    .finish(),
            )
            .finish(),
        );

        let can_confirm = installed
            .iter()
            .any(|app| app == &self.workspace_selected_app);
        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(            self.toolbar_button(
            &wormhole_i18n::t("common.cancel"),
            DevicesAction::CloseWorkspaceAppPicker,
            false,
            72.0,
            true,
        ));
        actions.add_child(
            Container::new(Flex::column().finish())
                .with_horizontal_margin(8.0)
                .finish(),
        );
        let open_label = wormhole_i18n::t("common.open");
        actions.add_child(self.toolbar_button(
            &open_label,
            DevicesAction::ConfirmWorkspaceAppOpen,
            true,
            72.0,
            can_confirm && !self.share_file_busy,
        ));
        dialog.add_child(
            Container::new(actions.finish())
                .with_margin_top(16.0)
                .finish(),
        );

        let panel = EventHandler::new(
            Container::new(
                ConstrainedBox::new(dialog.finish())
                    .with_width(480.0)
                    .with_max_height(720.0)
                    .finish(),
            )
            .with_uniform_padding(24.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish(),
        )
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
        .finish();

        let scrim = EventHandler::new(
            Container::new(Align::new(panel).finish())
                .with_uniform_padding(24.0)
                .with_background(ColorU::new(8, 7, 11, 190))
                .finish(),
        )
        .with_automation_label("关闭应用选择器")
        .with_automation_id("devices:scrim_workspace_picker")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::CloseWorkspaceAppPicker);
            DispatchEventResult::StopPropagation
        })
        .on_keydown(|ctx, _, keystroke| {
            if keystroke.key.as_str() == "escape" {
                ctx.dispatch_typed_action(DevicesAction::CloseWorkspaceAppPicker);
                DispatchEventResult::StopPropagation
            } else {
                DispatchEventResult::PropagateToParent
            }
        })
        .finish();
        scrim
    }

    fn workspace_app_option_row(&self, app: &str, selected: bool) -> Box<dyn Element> {
        let name = self.workspace_app_label(app);
        let app_label = name.clone();
        let desc = self.workspace_app_desc(app);
        let mut copy = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
        copy.add_child(
            ui_text::body(app_label.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        copy.add_child(
            Container::new(
                    ui_text::chat_sidebar_time(desc, self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(3.0)
            .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, copy.finish()).finish());
        row.add_child(
            ui_text::cluster_ctrl(
                if selected {
                    wormhole_i18n::t("common.selected")
                } else {
                    String::new()
                },
                self.mono,
            )
                .with_color(theme::accent_cool())
                .finish(),
        );
        let border = if selected {
            theme::accent_cool()
        } else {
            theme::border()
        };
        let bg = if selected {
            theme::accent_cool_bg(28)
        } else {
            theme::panel()
        };
        let app_id = app.to_string();
        EventHandler::new(
            Container::new(row.finish())
                .with_uniform_padding(11.0)
                .with_horizontal_padding(12.0)
                .with_background(bg)
                .with_border(Border::all(1.0).with_border_fill(border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .finish(),
        )
        .with_automation_label(app_label)
        .with_automation_id(format!("devices:workspace_app:{app_id}"))
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::SelectWorkspaceApp(app_id.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn workspace_app_search_row(&self, app: &str, installed: bool) -> Box<dyn Element> {
        let name = self.workspace_app_label(app);
        let desc = self.workspace_app_desc(app);
        let mut copy = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
        copy.add_child(
            ui_text::body(name, self.font)
                .with_color(theme::text())
                .finish(),
        );
        copy.add_child(
            Container::new(
                ui_text::chat_sidebar_time(desc, self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(3.0)
            .finish(),
        );
        let action_label = if installed {
            wormhole_i18n::t("devices.workspace.select")
        } else {
            wormhole_i18n::t("devices.workspace.install_and_open")
        };
        let action = if installed {
            DevicesAction::SelectWorkspaceApp(app.to_string())
        } else {
            DevicesAction::WorkspaceInstallAndOpen(app.to_string())
        };
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, copy.finish()).finish());
        row.add_child(
            Container::new(self.toolbar_button(&action_label, action, false, 88.0, true))
                .with_margin_left(10.0)
                .finish(),
        );
        Container::new(row.finish())
            .with_uniform_padding(9.0)
            .with_horizontal_padding(10.0)
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn workspace_approval_modal(&self, job: &WorkspaceProvisionJob) -> Box<dyn Element> {
        let requester = job
            .request
            .requested_by_node_id
            .as_deref()
            .map(|value| truncate_middle(value, 36))
            .unwrap_or_else(|| wormhole_i18n::t("devices.member.default"));
        let mut dialog = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let failed_candidate =
            job.stage == WorkspaceProvisionStage::Failed && job.import_candidate.is_some();
        dialog.add_child(
            ui_text::title(
                if failed_candidate {
                    wormhole_i18n::t("devices.provision.clone_unsafe_title")
                } else {
                    wormhole_i18n::t("devices.provision.allow_install_title")
                },
                self.font,
            )
            .finish(),
        );
        let manifest_name = job.manifest.name.clone();
        dialog.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t_args(
                        "devices.provision.request_body",
                        &[("requester", &requester), ("name", &manifest_name)],
                    ),
                    self.font,
                )
                .with_color(theme::text())
                .finish(),
            )
            .with_margin_top(12.0)
            .finish(),
        );
        let apps = job.manifest.installed_apps.join(", ");
        let size = format_size(job.manifest.size_bytes);
        dialog.add_child(status_line(
            wormhole_i18n::t_args(
                "devices.provision.image_meta",
                &[
                    ("version", &job.manifest.version),
                    ("apps", &apps),
                    ("size", &size),
                ],
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
                wormhole_i18n::t("devices.provision.keep_original_hint")
            } else if compatible_candidates > 0 {
                wormhole_i18n::t_args(
                    "devices.provision.candidates_hint",
                    &[("n", &compatible_candidates.to_string())],
                )
            } else {
                wormhole_i18n::t("devices.provision.no_safe_candidate")
            },
            self.font,
            StatusTone::Muted,
        ));
        dialog.add_child(status_line(
            wormhole_i18n::t("devices.provision.uac_warn"),
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
                let vm_name = truncate_middle(&candidate.vm_name, 28);
                let clone_label = wormhole_i18n::t_args(
                    "devices.provision.clone_named",
                    &[("name", &vm_name)],
                );
                dialog.add_child(
                    Container::new(self.toolbar_button(
                        &clone_label,
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
                    wormhole_i18n::t_args(
                        "devices.provision.more_candidates",
                        &[("n", &(compatible_candidates - 3).to_string())],
                    ),
                    self.font,
                    StatusTone::Muted,
                ));
            }
        }
        let mut actions = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        let later_label = wormhole_i18n::t("common.later");
        actions.add_child(self.toolbar_button(
            &later_label,
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
        let approve_label = if failed_candidate {
            wormhole_i18n::t("devices.provision.use_org_image")
        } else {
            wormhole_i18n::t("devices.provision.download_org_image")
        };
        actions.add_child(self.toolbar_button(
            &approve_label,
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
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
        .with_automation_label("对话框")
        .with_automation_id("devices:dialog_panel")
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
        return wormhole_i18n::t("devices.share.type.folder");
    }
    let ext = entry
        .name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" => wormhole_i18n::t("devices.share.type.txt"),
        "md" => "Markdown".into(),
        "json" => "JSON".into(),
        "pdf" => "PDF".into(),
        "toml" => "TOML".into(),
        "rs" => wormhole_i18n::t("devices.share.type.rs"),
        "swift" => wormhole_i18n::t("devices.share.type.swift"),
        _ => wormhole_i18n::t("devices.share.type.file"),
    }
}

fn workspace_file_extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

fn workspace_canonical_tool_id(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    if id == "onlyoffice" {
        "libreoffice".into()
    } else {
        id
    }
}

fn workspace_office_capability(app: &str) -> bool {
    matches!(
        workspace_canonical_tool_id(app).as_str(),
        "libreoffice" | "word" | "excel" | "powerpoint"
    )
}

fn workspace_capability_matches(declared: &str, requested: &str) -> bool {
    let declared = workspace_canonical_tool_id(declared);
    let requested = workspace_canonical_tool_id(requested);
    if declared == "default" || declared == requested {
        return true;
    }
    workspace_office_capability(&declared) && workspace_office_capability(&requested)
}

fn workspace_tool_supports_file(tool: &ToolSummary, file_name: &str) -> bool {
    let extension = workspace_file_extension(file_name);
    if extension.is_empty() {
        return false;
    }
    tool.descriptor
        .extensions
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&extension))
}

fn workspace_source_runtime_tools_from_catalog(tools: Vec<ToolSummary>) -> Vec<ToolSummary> {
    let mut out = Vec::new();
    for mut tool in tools {
        if tool.descriptor.executor != ToolExecutorKind::SourceRuntime {
            continue;
        }
        let canonical = workspace_canonical_tool_id(&tool.descriptor.id);
        if out.iter().any(|existing: &ToolSummary| {
            workspace_canonical_tool_id(&existing.descriptor.id) == canonical
        }) {
            continue;
        }
        if tool.descriptor.id.eq_ignore_ascii_case("onlyoffice") {
            tool.descriptor.id = "libreoffice".into();
            tool.descriptor.name = "LibreOffice".into();
            tool.descriptor.description =
                wormhole_i18n::t("devices.workspace.host_native_docs_hint");
        }
        out.push(tool);
    }
    out
}

fn workspace_worker_supports_app(worker: &WorkspaceWorker, app: &str) -> bool {
    let app = workspace_canonical_tool_id(app);
    worker.images.iter().any(|image| {
        if app == "default" {
            return !image.installed_apps.is_empty();
        }
        image
            .installed_apps
            .iter()
            .any(|installed| workspace_capability_matches(installed, &app))
    })
}

fn workspace_worker_execution_mode_for_app(
    worker: &WorkspaceWorker,
    app: &str,
) -> Option<WorkspaceExecutionMode> {
    let app = workspace_canonical_tool_id(app);
    worker.images.iter().find_map(|image| {
        let matches = if app == "default" {
            !image.installed_apps.is_empty()
        } else {
            image
                .installed_apps
                .iter()
                .any(|installed| workspace_capability_matches(installed, &app))
        };
        matches.then(|| image.execution_mode.clone())
    })
}

fn workspace_worker_mode_label(worker: &WorkspaceWorker, app: &str) -> String {
    match workspace_worker_execution_mode_for_app(worker, app) {
        Some(WorkspaceExecutionMode::HostNative) => wormhole_i18n::t("devices.workspace.mode_host_native"),
        Some(WorkspaceExecutionMode::VmGuest) => wormhole_i18n::t("devices.workspace.mode_vm_guest"),
        None => wormhole_i18n::t("devices.workspace.runner_default"),
    }
}

impl DevicesView {
    fn workspace_picker_host_native_hint(&self, file_name: &str) -> Option<String> {
        let author = self
            .selected_share_entry()
            .and_then(|entry| entry.version_author.as_deref())?;
        let worker = self.workspace_worker_for_author(author)?;
        let app = if self.workspace_selected_app.is_empty() {
            self.recommended_workspace_app(file_name)
        } else {
            self.workspace_selected_app.clone()
        };
        match workspace_worker_execution_mode_for_app(worker, &app) {
            Some(WorkspaceExecutionMode::HostNative) => Some(
                wormhole_i18n::t("devices.workspace.host_native_focus_warn"),
            ),
            Some(WorkspaceExecutionMode::VmGuest) => Some(
                wormhole_i18n::t("devices.workspace.vm_guest_focus_hint"),
            ),
            None => None,
        }
    }
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
        (self.create_cluster_modal_open && self.create_cluster_name_focused)
            || (self.share_rename_modal_open && self.share_rename_focused)
            || (self.workspace_app_picker_open && self.workspace_app_search_focused)
            || (self.mode == ViewMode::Grid && self.device_search_focused)
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
        } else if self.workspace_app_picker_open {
            let mut stack = Stack::new();
            stack.add_child(body);
            stack.add_child(self.workspace_app_picker_modal());
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
            DevicesAction::OpenAgentPairModal => {
                self.close_cluster_picker(ctx);
                self.open_agent_pair_modal(ctx);
            }
            DevicesAction::CloseAgentPairModal => self.close_agent_pair_modal(ctx),
            DevicesAction::CreateAgentPair => self.create_agent_pair(ctx),
            DevicesAction::CopyAgentPairUrl => self.copy_agent_pair_url(ctx),
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
                self.select_share_file(name.clone(), ctx);
                self.open_workspace_app_picker(ctx);
            }
            DevicesAction::ShareRemoteOpenOnHost(name) => {
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
            DevicesAction::OpenWorkspaceAppPicker => self.open_workspace_app_picker(ctx),
            DevicesAction::CloseWorkspaceAppPicker => self.close_workspace_app_picker(ctx),
            DevicesAction::SelectWorkspaceApp(app) => {
                self.select_workspace_app(app.clone(), ctx);
            }
            DevicesAction::WorkspaceAppSearchEdit(edit) => {
                self.edit_workspace_app_search(edit, ctx);
            }
            DevicesAction::FocusWorkspaceAppSearch => {
                self.workspace_app_search_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            DevicesAction::DeviceSearchEdit(edit) => {
                self.edit_device_search(edit, ctx);
            }
            DevicesAction::FocusDeviceSearch => {
                self.device_search_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            DevicesAction::BlurDeviceSearch => {
                self.device_search_focused = false;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            DevicesAction::ConfirmWorkspaceAppOpen => self.confirm_workspace_app_open(ctx),
            DevicesAction::WorkspaceInstallAndOpen(app) => {
                self.workspace_install_and_open(app.clone(), ctx);
            }
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
                let mut changed = false;
                if self.hovered_node_id.as_ref() == Some(node_id) {
                    self.hovered_node_id = None;
                    changed = true;
                }
                // Leave the card → drop sticky selection so hover/selected cannot
                // paint two accent rings at once. Keep selection while a context
                // menu or delete modal is open for that node.
                let menu_holds = self
                    .device_context_menu
                    .as_ref()
                    .is_some_and(|(id, _, _)| id == node_id)
                    || self.delete_modal_node_id.as_ref() == Some(node_id);
                if !menu_holds && self.selected_node_id.as_ref() == Some(node_id) {
                    self.selected_node_id = None;
                    changed = true;
                }
                if changed {
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
                    self.status_flash = Some(wormhole_i18n::t("devices.chat.cannot_self"));
                    ctx.notify();
                    return;
                }
                let chat_opening = wormhole_i18n::t("devices.chat.opening");
                self.status_flash = Some(chat_opening.clone());
                ctx.notify();
                ctx.spawn(
                    async move {
                        tokio::time::sleep(Duration::from_millis(2600)).await;
                    },
                    move |view, _, ctx| {
                        if view.status_flash.as_deref() == Some(chat_opening.as_str()) {
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
                    self.status_flash = Some(wormhole_i18n::t("devices.rdp.cannot_self"));
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
                    self.status_flash = Some(wormhole_i18n::t("devices.toast.offline_no_rdp"));
                    ctx.notify();
                    return;
                }
                self.status_flash = Some(wormhole_i18n::t("devices.rdp.opening"));
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
                user_id: None,
                account_display_name: None,
                pending_handshake: false,
                handshake_error: None,
                share_volumes: volumes
                    .iter()
                    .map(|(id, name)| ShareVolumeRosterDto {
                        volume_id: (*id).into(),
                        name: (*name).into(),
                    })
                    .collect(),
                has_local_share_replicas: false,
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
    use super::{
        workspace_canonical_tool_id, workspace_capability_matches,
        workspace_source_runtime_tools_from_catalog, workspace_tool_supports_file,
    };
    use wormhole_desktop_core::toolbox_ui::{
        ToolCategory, ToolDescriptor, ToolExecutorKind, ToolInstallStage, ToolInstallStatus,
        ToolSummary,
    };

    fn sample_tool(id: &str, name: &str, extensions: &[&str]) -> ToolSummary {
        ToolSummary {
            descriptor: ToolDescriptor {
                id: id.into(),
                name: name.into(),
                description: format!("{name} desc"),
                executor: ToolExecutorKind::SourceRuntime,
                categories: vec![ToolCategory::Documents],
                icon: None,
                extensions: extensions.iter().map(|value| (*value).to_string()).collect(),
                requires_file: true,
                packages: Vec::new(),
            },
            status: ToolInstallStatus {
                tool_id: id.into(),
                stage: ToolInstallStage::Ready,
                detail: String::new(),
                bytes_downloaded: 0,
                bytes_total: 0,
                installed_version: Some("1".into()),
                available_version: Some("1".into()),
                entrypoint: None,
                last_error: None,
                updated_at: 0,
            },
        }
    }

    #[test]
    fn onlyoffice_catalog_entries_canonicalize_to_libreoffice() {
        assert_eq!(workspace_canonical_tool_id("onlyoffice"), "libreoffice");
        assert!(workspace_capability_matches("libreoffice", "onlyoffice"));
        let tools = workspace_source_runtime_tools_from_catalog(vec![
            sample_tool("onlyoffice", "ONLYOFFICE", &["docx"]),
            sample_tool("paint", "Paint", &["png"]),
        ]);
        assert_eq!(tools[0].descriptor.id, "libreoffice");
        assert_eq!(tools[0].descriptor.name, "LibreOffice");
        assert!(workspace_tool_supports_file(&tools[0], "report.docx"));
        assert!(!workspace_tool_supports_file(&tools[0], "photo.png"));
        assert!(workspace_tool_supports_file(&tools[1], "photo.png"));
    }

    #[test]
    fn empty_worker_capability_does_not_invent_apps() {
        // Intersection helper lives on DevicesView; capability matching alone must not
        // treat empty declarations as universal support.
        assert!(!workspace_capability_matches("", "paint"));
        assert!(!workspace_capability_matches(" ", "libreoffice"));
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
