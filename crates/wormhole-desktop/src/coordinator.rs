use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::ui::window_options::desktop_popout_window_options;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::elements::{ConstrainedBox, Rect};
use warpui::platform::TerminationMode;
use warpui::ui_automation::{format_outline, UiAutomationNode, UiTapSelector};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext, WindowId,
};
use warpui_core::assets::asset_cache::AssetCache;
use wormhole_desktop_rdp::RdpRuntime;

use crate::wormhole_native_ipc::AgentTerminalBackend;

use wormhole_desktop_core::warp_embed_prefs::{self, PreferredAgent};

use crate::agent_events_view::AgentEventsView;
#[cfg(any(windows, target_os = "macos"))]
use crate::computer_use_view::ComputerUseView;
use crate::rdp_host_control_view::RdpHostControlView;
use crate::rdp_view::RdpViewerView;
use crate::workspace_rdp_view::new_workspace_rdp_view;
use crate::workspace_session_hud_view::WorkspaceSessionHudView;

#[derive(Debug, Clone)]
pub enum UiCommand {
    OpenRdp {
        peer: String,
        title: String,
        reconnect: bool,
        window_key: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    },
    FocusRdp {
        window_key: String,
        reconnect: bool,
    },
    OpenAgent {
        window_key: String,
        title: String,
        session_key: String,
        backend: AgentTerminalBackend,
        cwd: Option<String>,
        profile: String,
        codex_home: Option<String>,
        api_key: String,
        codex_binary: Option<PathBuf>,
        node_binary: Option<PathBuf>,
        cursor_script: Option<PathBuf>,
        model: Option<String>,
        cursor_workdir: Option<PathBuf>,
    },
    FocusAgent {
        window_key: String,
    },
    OpenComputerUse {
        window_key: String,
        title: String,
        session_key: String,
    },
    FocusComputerUse {
        window_key: String,
    },
    OpenWorkspaceRdp {
        window_key: String,
        title: String,
        session_id: String,
        peer: String,
        password: String,
        clipboard_policy: Option<String>,
        watermark_text: Option<String>,
        fps: i32,
    },
    FocusWorkspaceRdp {
        window_key: String,
    },
    OpenLiveViewer {
        window_key: String,
        title: String,
        peer: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    },
    FocusLiveViewer {
        window_key: String,
    },
    OpenAgentEvents {
        window_key: String,
        title: String,
        target_node: String,
        task_id: String,
    },
    FocusAgentEvents {
        window_key: String,
    },
    OpenHostControl {
        window_key: String,
        title: String,
    },
    FocusHostControl {
        window_key: String,
    },
    OpenWorkspaceHud {
        window_key: String,
        title: String,
        session_id: String,
        app_name: Option<String>,
        status: Option<String>,
        status_detail: Option<String>,
    },
    FocusWorkspaceHud {
        window_key: String,
    },
    UiOutline {
        format: String,
        reply: mpsc::SyncSender<Result<UiOutlineResult, String>>,
    },
    UiTap {
        selector: String,
        reply: mpsc::SyncSender<Result<UiTapResult, String>>,
    },
    UiType {
        text: String,
        reply: mpsc::SyncSender<Result<(), String>>,
    },
    UiAppState {
        reply: mpsc::SyncSender<Result<UiAppStateResult, String>>,
    },
    RunWarpRemotePrompt {
        task_id: String,
        agent: PreferredAgent,
        prompt: String,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct UiOutlineResult {
    pub format: String,
    pub outline: String,
    pub nodes: Vec<UiAutomationNode>,
    pub window_id: usize,
}

#[derive(Debug, Clone)]
pub struct UiTapResult {
    pub alias: u32,
    pub label: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct UiAppStateResult {
    pub main_window_id: Option<usize>,
    pub target_count: usize,
}

pub struct CoordinatorState {
    data_dir: PathBuf,
    pending: Vec<UiCommand>,
    rdp_windows: HashMap<String, WindowId>,
    agent_windows: HashMap<String, WindowId>,
    computer_use_windows: HashMap<String, WindowId>,
    workspace_windows: HashMap<String, WindowId>,
    live_windows: HashMap<String, WindowId>,
    agent_events_windows: HashMap<String, WindowId>,
    host_control_window: Option<WindowId>,
    workspace_hud_windows: HashMap<String, WindowId>,
    rdp_runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    pending_warp_focus: Option<PreferredAgent>,
    main_shell_window: Option<WindowId>,
    /// Last `ui_outline` nodes for `@N` resolution until the next outline.
    last_ui_outline: Vec<UiAutomationNode>,
}

impl CoordinatorState {
    pub fn new(data_dir: PathBuf) -> Self {
        let rdp_data_dir = data_dir.join("native-rdp");
        Self {
            data_dir: data_dir.clone(),
            pending: Vec::new(),
            rdp_windows: HashMap::new(),
            agent_windows: HashMap::new(),
            computer_use_windows: HashMap::new(),
            workspace_windows: HashMap::new(),
            live_windows: HashMap::new(),
            agent_events_windows: HashMap::new(),
            host_control_window: None,
            workspace_hud_windows: HashMap::new(),
            rdp_runtime: Arc::new(tokio::sync::Mutex::new(RdpRuntime::new(rdp_data_dir))),
            pending_warp_focus: None,
            main_shell_window: None,
            last_ui_outline: Vec::new(),
        }
    }

    pub fn set_main_shell_window(&mut self, window_id: WindowId) {
        self.main_shell_window = Some(window_id);
    }

    pub fn main_shell_window(&self) -> Option<WindowId> {
        self.main_shell_window
    }

    pub fn is_main_shell_window(&self, window_id: WindowId) -> bool {
        self.main_shell_window == Some(window_id)
    }

    pub fn take_pending_warp_focus(&mut self) -> Option<PreferredAgent> {
        self.pending_warp_focus.take()
    }

    fn request_warp_focus(&mut self, agent: PreferredAgent) {
        self.pending_warp_focus = Some(agent);
    }

    pub fn focus_warp_agent(&mut self, agent: PreferredAgent) {
        let mut prefs = warp_embed_prefs::load_prefs(&self.data_dir);
        prefs.preferred_agent = agent;
        let _ = warp_embed_prefs::save_prefs(&self.data_dir, &prefs);
        self.request_warp_focus(agent);
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn enqueue(&mut self, command: UiCommand) {
        self.pending.push(command);
    }

    fn drain_commands(&mut self) -> Vec<UiCommand> {
        std::mem::take(&mut self.pending)
    }

    pub fn rdp_runtime(&self) -> Arc<tokio::sync::Mutex<RdpRuntime>> {
        self.rdp_runtime.clone()
    }
}

pub struct CoordinatorView {
    state: Arc<Mutex<CoordinatorState>>,
}

impl CoordinatorView {
    pub fn new(ctx: &mut ViewContext<Self>, state: Arc<Mutex<CoordinatorState>>) -> Self {
        AssetCache::handle(ctx).update(ctx, |_, _| {});
        ctx.focus_self();

        let view = Self { state };
        view.start_poll(ctx);
        view.process_pending(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(16));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::poll_once(ctx, tick_rx);
    }

    fn poll_once(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    view.process_pending(ctx);
                    ctx.notify();
                    Self::poll_once(ctx, tick_rx);
                }
            },
        );
    }

    fn process_pending(&self, ctx: &mut ViewContext<Self>) {
        let commands = {
            let mut guard = self.state.lock().expect("coordinator lock");
            guard.drain_commands()
        };
        for command in commands {
            match command {
                UiCommand::OpenRdp {
                    peer,
                    title,
                    reconnect: _,
                    window_key,
                    password,
                    totp_code,
                    fps,
                } => {
                    self.open_rdp_window(ctx, &window_key, &peer, &title, password, totp_code, fps)
                }
                UiCommand::FocusRdp {
                    window_key,
                    reconnect: _,
                } => self.focus_rdp_window(ctx, &window_key),
                UiCommand::OpenAgent { backend, .. } => self.focus_warp_for_agent(backend, ctx),
                UiCommand::FocusAgent { window_key: _ } => {
                    if let Ok(mut guard) = self.state.lock() {
                        let agent = warp_embed_prefs::load_prefs(&guard.data_dir).preferred_agent;
                        guard.focus_warp_agent(agent);
                    }
                }
                UiCommand::OpenComputerUse {
                    window_key,
                    title,
                    session_key: _,
                } => self.open_computer_use_window(ctx, &window_key, &title),
                UiCommand::FocusComputerUse { window_key } => {
                    self.focus_computer_use_window(ctx, &window_key)
                }
                UiCommand::OpenWorkspaceRdp {
                    window_key,
                    title,
                    session_id: _,
                    peer,
                    password,
                    clipboard_policy: _,
                    watermark_text,
                    fps,
                } => self.open_workspace_window(
                    ctx,
                    &window_key,
                    &title,
                    &peer,
                    password,
                    watermark_text,
                    fps,
                ),
                UiCommand::FocusWorkspaceRdp { window_key } => {
                    self.focus_workspace_window(ctx, &window_key)
                }
                UiCommand::OpenLiveViewer {
                    window_key,
                    title,
                    peer,
                    password,
                    totp_code,
                    fps,
                } => self.open_live_viewer_window(
                    ctx,
                    &window_key,
                    &title,
                    &peer,
                    password,
                    totp_code,
                    fps,
                ),
                UiCommand::FocusLiveViewer { window_key } => {
                    self.focus_live_viewer_window(ctx, &window_key)
                }
                UiCommand::OpenAgentEvents {
                    window_key,
                    title,
                    target_node,
                    task_id,
                } => {
                    self.open_agent_events_window(ctx, &window_key, &title, &target_node, &task_id)
                }
                UiCommand::FocusAgentEvents { window_key } => {
                    self.focus_agent_events_window(ctx, &window_key)
                }
                UiCommand::OpenHostControl { window_key, title } => {
                    self.open_host_control_window(ctx, &window_key, &title);
                }
                UiCommand::FocusHostControl { window_key } => {
                    self.focus_host_control_window(ctx, &window_key);
                }
                UiCommand::OpenWorkspaceHud {
                    window_key,
                    title,
                    session_id,
                    app_name,
                    status,
                    status_detail,
                } => self.open_workspace_hud_window(
                    ctx,
                    &window_key,
                    &title,
                    &session_id,
                    app_name,
                    status,
                    status_detail,
                ),
                UiCommand::FocusWorkspaceHud { window_key } => {
                    self.focus_workspace_hud_window(ctx, &window_key);
                }
                UiCommand::UiOutline { format, reply } => {
                    let _ = reply.send(self.handle_ui_outline(ctx, &format));
                }
                UiCommand::UiTap { selector, reply } => {
                    let _ = reply.send(self.handle_ui_tap(ctx, &selector));
                }
                UiCommand::UiType { text, reply } => {
                    let _ = reply.send(self.handle_ui_type(ctx, &text));
                }
                UiCommand::UiAppState { reply } => {
                    let _ = reply.send(self.handle_ui_app_state(ctx));
                }
                UiCommand::RunWarpRemotePrompt { agent, .. } => {
                    if let Ok(mut guard) = self.state.lock() {
                        guard.focus_warp_agent(agent);
                    }
                }
                UiCommand::Shutdown => {
                    ctx.terminate_app(TerminationMode::Cancellable, None);
                }
            }
        }
    }

    fn main_window_id(&self) -> Result<WindowId, String> {
        self.state
            .lock()
            .expect("coordinator lock")
            .main_shell_window()
            .ok_or_else(|| "main shell window not ready".into())
    }

    fn handle_ui_outline(
        &self,
        ctx: &mut ViewContext<Self>,
        format: &str,
    ) -> Result<UiOutlineResult, String> {
        let window_id = self.main_window_id()?;
        let nodes = ctx.ui_automation_snapshot(window_id)?;
        let outline = if format.eq_ignore_ascii_case("json") {
            serde_json::to_string_pretty(&nodes).map_err(|e| e.to_string())?
        } else {
            format_outline(&nodes)
        };
        {
            let mut guard = self.state.lock().expect("coordinator lock");
            guard.last_ui_outline = nodes.clone();
        }
        Ok(UiOutlineResult {
            format: if format.eq_ignore_ascii_case("json") {
                "json".into()
            } else {
                "text".into()
            },
            outline,
            nodes,
            window_id: window_id.to_usize(),
        })
    }

    fn handle_ui_tap(
        &self,
        ctx: &mut ViewContext<Self>,
        selector: &str,
    ) -> Result<UiTapResult, String> {
        let window_id = self.main_window_id()?;
        // Refresh snapshot so unlabeled layout changes are visible, but resolve
        // against the last outline when using @N (stale aliases must fail clearly).
        let fresh = ctx.ui_automation_snapshot(window_id)?;
        let sel = UiTapSelector::parse(selector)?;
        let node = {
            let guard = self.state.lock().expect("coordinator lock");
            let cached = if matches!(sel, UiTapSelector::Alias(_)) {
                &guard.last_ui_outline
            } else {
                &fresh
            };
            sel.resolve(cached)?.clone()
        };
        let center = node.center();
        ctx.ui_automation_click_at(window_id, center)?;
        {
            let mut guard = self.state.lock().expect("coordinator lock");
            guard.last_ui_outline = fresh;
        }
        Ok(UiTapResult {
            alias: node.alias,
            label: node.label,
            x: center.x(),
            y: center.y(),
        })
    }

    fn handle_ui_type(&self, ctx: &mut ViewContext<Self>, text: &str) -> Result<(), String> {
        let window_id = self.main_window_id()?;
        ctx.ui_automation_type_text(window_id, text)
    }

    fn handle_ui_app_state(&self, ctx: &mut ViewContext<Self>) -> Result<UiAppStateResult, String> {
        let main = self
            .state
            .lock()
            .expect("coordinator lock")
            .main_shell_window();
        let target_count = main
            .map(|wid| {
                ctx.ui_automation_snapshot(wid)
                    .map(|n| n.len())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        Ok(UiAppStateResult {
            main_window_id: main.map(|w| w.to_usize()),
            target_count,
        })
    }

    fn open_rdp_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        peer: &str,
        title: &str,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    ) {
        if let Some(window_id) = self.rdp_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let runtime = self.rdp_runtime();
        let peer = peer.to_string();
        let window_key = window_key.to_string();
        let state = self.state.clone();

        let options = desktop_popout_window_options(title, vec2f(1280., 820.));

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            RdpViewerView::new(view_ctx, runtime, peer, password, totp_code, fps)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.rdp_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn focus_warp_for_agent(&self, backend: AgentTerminalBackend, _ctx: &mut ViewContext<Self>) {
        let agent = match backend {
            AgentTerminalBackend::Cursor => PreferredAgent::Cursor,
            AgentTerminalBackend::Codex => PreferredAgent::Codex,
        };
        if let Ok(mut guard) = self.state.lock() {
            guard.focus_warp_agent(agent);
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn open_computer_use_window(&self, ctx: &mut ViewContext<Self>, window_key: &str, title: &str) {
        if let Some(window_id) = self.computer_use_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let window_key = window_key.to_string();
        let state = self.state.clone();
        let options = desktop_popout_window_options(title, vec2f(1300., 820.));

        let (window_id, _) = ctx.add_window(options, |view_ctx| ComputerUseView::new(view_ctx));

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.computer_use_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn open_computer_use_window(
        &self,
        _ctx: &mut ViewContext<Self>,
        _window_key: &str,
        _title: &str,
    ) {
    }

    fn computer_use_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.computer_use_windows.get(window_key).copied()
    }

    fn focus_computer_use_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.computer_use_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn open_workspace_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
        peer: &str,
        password: String,
        watermark_text: Option<String>,
        fps: i32,
    ) {
        if let Some(window_id) = self.take_workspace_window_id(window_key) {
            ctx.windows()
                .close_window(window_id, TerminationMode::ForceTerminate);
        }

        let runtime = self.rdp_runtime();
        let peer = peer.to_string();
        let window_key = window_key.to_string();
        let state = self.state.clone();

        let options = desktop_popout_window_options(title, vec2f(1280., 820.));

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            new_workspace_rdp_view(view_ctx, runtime, peer, password, fps, watermark_text)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.workspace_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn open_live_viewer_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
        peer: &str,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    ) {
        if let Some(window_id) = self.live_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let runtime = self.rdp_runtime();
        let peer = peer.to_string();
        let window_key = window_key.to_string();
        let state = self.state.clone();
        let fps = fps.max(1);

        let options = desktop_popout_window_options(title, vec2f(1280., 720.));

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            RdpViewerView::new_live(view_ctx, runtime, peer, password, totp_code, fps)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.live_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn open_agent_events_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
        target_node: &str,
        task_id: &str,
    ) {
        if let Some(window_id) = self.agent_events_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let runtime = self.rdp_runtime();
        let target_node = target_node.to_string();
        let task_id = task_id.to_string();
        let window_key = window_key.to_string();
        let state = self.state.clone();

        let options = desktop_popout_window_options(title, vec2f(1120., 760.));

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            AgentEventsView::new(view_ctx, runtime, target_node, task_id)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.agent_events_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn live_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.live_windows.get(window_key).copied()
    }

    fn agent_events_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.agent_events_windows.get(window_key).copied()
    }

    fn focus_live_viewer_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.live_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn focus_agent_events_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.agent_events_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn open_host_control_window(&self, ctx: &mut ViewContext<Self>, window_key: &str, title: &str) {
        if let Some(window_id) = self.host_control_window_id() {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let runtime = self.rdp_runtime();
        let state = self.state.clone();
        let data_dir = {
            let guard = state.lock().expect("coordinator lock");
            guard.data_dir().to_path_buf()
        };
        let coordinator = state.clone();
        let window_key = window_key.to_string();

        let options = desktop_popout_window_options(title, vec2f(760., 620.));

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            RdpHostControlView::new(view_ctx, data_dir, runtime, coordinator)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.host_control_window = Some(window_id);
            let _ = window_key;
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn host_control_window_id(&self) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.host_control_window
    }

    fn focus_host_control_window(&self, ctx: &mut ViewContext<Self>, _window_key: &str) {
        if let Some(window_id) = self.host_control_window_id() {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn open_workspace_hud_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
        session_id: &str,
        app_name: Option<String>,
        status: Option<String>,
        status_detail: Option<String>,
    ) {
        if let Some(window_id) = self.workspace_hud_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let state = self.state.clone();
        let coordinator = state.clone();
        let window_key = window_key.to_string();
        let session_id = session_id.to_string();
        let hud_title = title.to_string();

        let options = desktop_popout_window_options(title, vec2f(720., 480.));

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            WorkspaceSessionHudView::new(
                view_ctx,
                coordinator,
                session_id,
                hud_title,
                app_name,
                status,
                status_detail,
            )
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.workspace_hud_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn workspace_hud_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.workspace_hud_windows.get(window_key).copied()
    }

    fn focus_workspace_hud_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.workspace_hud_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn rdp_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.rdp_windows.get(window_key).copied()
    }

    fn agent_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.agent_windows.get(window_key).copied()
    }

    fn workspace_window_id(&self, window_key: &str) -> Option<WindowId> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.workspace_windows.get(window_key).copied()
    }

    fn take_workspace_window_id(&self, window_key: &str) -> Option<WindowId> {
        let mut guard = self.state.lock().expect("coordinator lock");
        guard.workspace_windows.remove(window_key)
    }

    fn focus_rdp_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.rdp_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn focus_agent_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.agent_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn focus_workspace_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        if let Some(window_id) = self.workspace_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }

    fn rdp_runtime(&self) -> Arc<tokio::sync::Mutex<RdpRuntime>> {
        let guard = self.state.lock().expect("coordinator lock");
        guard.rdp_runtime()
    }
}

impl Entity for CoordinatorView {
    type Event = ();
}

impl View for CoordinatorView {
    fn ui_name() -> &'static str {
        "CoordinatorView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        ConstrainedBox::new(
            Rect::new()
                .with_background_color(ColorU::new(0, 0, 0, 0))
                .finish(),
        )
        .with_width(1.0)
        .with_height(1.0)
        .finish()
    }
}

impl TypedActionView for CoordinatorView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}
