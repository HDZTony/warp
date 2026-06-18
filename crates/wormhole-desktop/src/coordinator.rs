use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::elements::{ConstrainedBox, Rect};
use warpui::platform::{TerminationMode, WindowBounds};
use warpui::{
    AddWindowOptions, AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View,
    ViewContext, WindowId,
};
use warpui_core::assets::asset_cache::AssetCache;
use wormhole_desktop_rdp::RdpRuntime;

use wormhole_native_ipc::AgentTerminalBackend;

use crate::agent_events_view::AgentEventsView;
use crate::agent_terminal_view::CodexTerminalView;
#[cfg(any(windows, target_os = "macos"))]
use crate::computer_use_view::ComputerUseView;
use crate::cursor_agent_view::CursorAgentView;
use crate::rdp_view::RdpViewerView;
use crate::rdp_host_control_view::RdpHostControlView;
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
    Shutdown,
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
}

impl CoordinatorState {
    pub fn new(data_dir: PathBuf) -> Self {
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
            rdp_runtime: Arc::new(tokio::sync::Mutex::new(RdpRuntime::new(data_dir))),
        }
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
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(16));
                if tick_tx.send_blocking(()).is_err() {
                    break;
                }
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
                } => self.open_rdp_window(
                    ctx,
                    &window_key,
                    &peer,
                    &title,
                    password,
                    totp_code,
                    fps,
                ),
                UiCommand::FocusRdp {
                    window_key,
                    reconnect: _,
                } => self.focus_rdp_window(ctx, &window_key),
                UiCommand::OpenAgent {
                    window_key,
                    title,
                    session_key,
                    backend,
                    cwd,
                    profile,
                    codex_home,
                    api_key,
                    codex_binary,
                    node_binary,
                    cursor_script,
                    model,
                    cursor_workdir,
                } => self.open_agent_window(
                    ctx,
                    &window_key,
                    &title,
                    &session_key,
                    backend,
                    codex_binary,
                    codex_home,
                    api_key,
                    profile,
                    cwd,
                    node_binary,
                    cursor_script,
                    model,
                    cursor_workdir,
                ),
                UiCommand::FocusAgent { window_key } => self.focus_agent_window(ctx, &window_key),
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
                } => self.open_agent_events_window(
                    ctx,
                    &window_key,
                    &title,
                    &target_node,
                    &task_id,
                ),
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
                UiCommand::Shutdown => {
                    ctx.terminate_app(TerminationMode::Cancellable, None);
                }
            }
        }
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

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1280., 820.)),
            ..Default::default()
        };

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            RdpViewerView::new(view_ctx, runtime, peer, password, totp_code, fps)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.rdp_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn open_agent_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
        session_key: &str,
        backend: AgentTerminalBackend,
        codex_binary: Option<PathBuf>,
        codex_home: Option<String>,
        api_key: String,
        profile: String,
        cwd: Option<String>,
        node_binary: Option<PathBuf>,
        cursor_script: Option<PathBuf>,
        model: Option<String>,
        cursor_workdir: Option<PathBuf>,
    ) {
        if let Some(window_id) = self.agent_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let window_key = window_key.to_string();
        let state = self.state.clone();
        let data_dir = {
            let guard = state.lock().expect("coordinator lock");
            guard.data_dir().to_path_buf()
        };
        let session_key = session_key.to_string();
        let cwd = cwd.map(PathBuf::from);

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1120., 760.)),
            ..Default::default()
        };

        let window_id = match backend {
            AgentTerminalBackend::Cursor => {
                let node = node_binary.expect("node binary required for cursor");
                let script = cursor_script.expect("cursor script required");
                let workdir = cursor_workdir
                    .or(cwd.clone())
                    .unwrap_or_else(|| data_dir.clone());
                let model = model.unwrap_or_else(|| "composer-2.5".to_string());
                let (window_id, _) = ctx.add_window(options, move |view_ctx| {
                    CursorAgentView::new(
                        view_ctx,
                        data_dir,
                        session_key,
                        node,
                        script,
                        workdir,
                        api_key,
                        model,
                    )
                });
                window_id
            }
            AgentTerminalBackend::Codex => {
                let codex_binary = codex_binary.expect("codex binary required");
                let codex_home = codex_home.unwrap_or_else(|| data_dir.display().to_string());
                let (window_id, _) = ctx.add_window(options, move |view_ctx| {
                    CodexTerminalView::new(
                        view_ctx,
                        codex_binary,
                        PathBuf::from(codex_home),
                        api_key,
                        profile,
                        cwd,
                    )
                });
                window_id
            }
        };

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.agent_windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn open_computer_use_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
    ) {
        if let Some(window_id) = self.computer_use_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let window_key = window_key.to_string();
        let state = self.state.clone();
        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1300., 820.)),
            ..Default::default()
        };

        let (window_id, _) =
            ctx.add_window(options, |view_ctx| ComputerUseView::new(view_ctx));

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
        if let Some(window_id) = self.workspace_window_id(window_key) {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let runtime = self.rdp_runtime();
        let peer = peer.to_string();
        let window_key = window_key.to_string();
        let state = self.state.clone();

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1280., 820.)),
            ..Default::default()
        };

        let (window_id, _) = ctx.add_window(options, move |view_ctx| {
            new_workspace_rdp_view(
                view_ctx,
                runtime,
                peer,
                password,
                fps,
                watermark_text,
            )
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

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1280., 720.)),
            ..Default::default()
        };

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

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1120., 760.)),
            ..Default::default()
        };

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

    fn open_host_control_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        title: &str,
    ) {
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

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(760., 620.)),
            ..Default::default()
        };

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

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(720., 480.)),
            ..Default::default()
        };

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
            guard
                .workspace_hud_windows
                .insert(window_key, window_id);
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
