use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::wormhole_native_ipc::{
    agent_events_window_key, agent_window_key, computer_use_window_key, host_control_window_key,
    live_viewer_window_key, session_file_path, workspace_hud_window_key, workspace_window_key,
    AgentTerminalBackend, ApiErrorBody, BridgeSessionFile, FocusAgentEventsWindowRequest,
    FocusAgentWindowRequest, FocusComputerUseWindowRequest, FocusHostControlWindowRequest,
    FocusLiveViewerWindowRequest, FocusRdpWindowRequest, FocusWorkspaceHudWindowRequest,
    FocusWorkspaceRdpWindowRequest, InvokeRdpRequest, InvokeRdpResponse,
    OpenAgentEventsWindowRequest, OpenAgentEventsWindowResponse, OpenAgentWindowRequest,
    OpenAgentWindowResponse, OpenComputerUseWindowRequest, OpenComputerUseWindowResponse,
    OpenHostControlWindowRequest, OpenHostControlWindowResponse, OpenLiveViewerWindowRequest,
    OpenLiveViewerWindowResponse, OpenRdpWindowRequest, OpenRdpWindowResponse,
    OpenWorkspaceHudWindowRequest, OpenWorkspaceHudWindowResponse, OpenWorkspaceRdpWindowRequest,
    OpenWorkspaceRdpWindowResponse, UiAppStateResponse, UiOutlineRequest, UiOutlineResponse,
    UiTapRequest, UiTapResponse, UiTypeRequest, FOCUS_AGENT_EVENTS_PATH, FOCUS_AGENT_PATH,
    FOCUS_COMPUTER_USE_PATH, FOCUS_HOST_CONTROL_PATH, FOCUS_LIVE_VIEWER_PATH, FOCUS_RDP_PATH,
    FOCUS_WORKSPACE_HUD_PATH, FOCUS_WORKSPACE_RDP_PATH, HEALTH_PATH, INVOKE_RDP_PATH,
    OPEN_AGENT_EVENTS_PATH, OPEN_AGENT_PATH, OPEN_COMPUTER_USE_PATH, OPEN_HOST_CONTROL_PATH,
    OPEN_LIVE_VIEWER_PATH, OPEN_RDP_PATH, OPEN_WORKSPACE_HUD_PATH, OPEN_WORKSPACE_RDP_PATH,
    SHUTDOWN_PATH, UI_APP_STATE_PATH, UI_OUTLINE_PATH, UI_TAP_PATH, UI_TYPE_PATH,
};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::coordinator::{CoordinatorState, UiCommand};
use crate::rdp_invoke;

pub fn run(coordinator: Arc<Mutex<CoordinatorState>>) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_async(coordinator))
}

async fn run_async(coordinator: Arc<Mutex<CoordinatorState>>) -> anyhow::Result<()> {
    let data_dir = {
        let guard = coordinator.lock().expect("coordinator lock");
        guard.data_dir().to_path_buf()
    };
    let token = Uuid::new_v4().to_string();
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let endpoint = format!("http://127.0.0.1:{port}");
    let session = BridgeSessionFile {
        endpoint: endpoint.clone(),
        token: token.clone(),
        pid: Some(std::process::id()),
    };
    std::fs::create_dir_all(&data_dir)?;
    std::fs::write(
        session_file_path(&data_dir),
        serde_json::to_vec_pretty(&session)?,
    )?;
    tracing::info!(%endpoint, "native UI IPC listening");

    let app = Router::new()
        .route(HEALTH_PATH, get(health))
        .route(OPEN_RDP_PATH, post(open_rdp))
        .route(FOCUS_RDP_PATH, post(focus_rdp))
        .route(OPEN_AGENT_PATH, post(open_agent))
        .route(FOCUS_AGENT_PATH, post(focus_agent))
        .route(OPEN_COMPUTER_USE_PATH, post(open_computer_use))
        .route(FOCUS_COMPUTER_USE_PATH, post(focus_computer_use))
        .route(OPEN_LIVE_VIEWER_PATH, post(open_live_viewer))
        .route(FOCUS_LIVE_VIEWER_PATH, post(focus_live_viewer))
        .route(OPEN_AGENT_EVENTS_PATH, post(open_agent_events))
        .route(FOCUS_AGENT_EVENTS_PATH, post(focus_agent_events))
        .route(OPEN_WORKSPACE_RDP_PATH, post(open_workspace_rdp))
        .route(FOCUS_WORKSPACE_RDP_PATH, post(focus_workspace_rdp))
        .route(OPEN_HOST_CONTROL_PATH, post(open_host_control))
        .route(FOCUS_HOST_CONTROL_PATH, post(focus_host_control))
        .route(OPEN_WORKSPACE_HUD_PATH, post(open_workspace_hud))
        .route(FOCUS_WORKSPACE_HUD_PATH, post(focus_workspace_hud))
        .route(INVOKE_RDP_PATH, post(invoke_rdp))
        .route(UI_OUTLINE_PATH, post(ui_outline))
        .route(UI_TAP_PATH, post(ui_tap))
        .route(UI_TYPE_PATH, post(ui_type))
        .route(UI_APP_STATE_PATH, post(ui_app_state))
        .route(SHUTDOWN_PATH, post(shutdown))
        .with_state(DaemonState { coordinator, token });

    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct DaemonState {
    coordinator: Arc<Mutex<CoordinatorState>>,
    token: String,
}

fn authorize(
    headers: &HeaderMap,
    expected: &str,
) -> Result<(), (StatusCode, axum::Json<ApiErrorBody>)> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(ApiErrorBody {
                    error: "missing Authorization".into(),
                }),
            )
        })?;
    let token = auth.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(ApiErrorBody {
                error: "expected Bearer token".into(),
            }),
        )
    })?;
    if token != expected {
        return Err((
            StatusCode::UNAUTHORIZED,
            axum::Json(ApiErrorBody {
                error: "invalid token".into(),
            }),
        ));
    }
    Ok(())
}

async fn health(
    State(state): State<DaemonState>,
    headers: HeaderMap,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    Ok("ok")
}

async fn open_rdp(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenRdpWindowRequest>,
) -> Result<axum::Json<OpenRdpWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let peer = body.peer.trim().to_string();
    if peer.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "peer is required".into(),
            }),
        ));
    }
    let window_key = crate::wormhole_native_ipc::rdp_window_key(&peer);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenRdp {
            peer,
            title: body.title,
            reconnect: body.reconnect,
            window_key: window_key.clone(),
            password: body.password,
            totp_code: body.totp_code,
            fps: body.fps.unwrap_or(60),
        });
    }
    Ok(axum::Json(OpenRdpWindowResponse { window_key }))
}

async fn focus_rdp(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusRdpWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let peer = body.peer.trim().to_string();
    if peer.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "peer is required".into(),
            }),
        ));
    }
    let window_key = crate::wormhole_native_ipc::rdp_window_key(&peer);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusRdp {
            window_key,
            reconnect: body.reconnect,
        });
    }
    Ok("ok")
}

async fn open_agent(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenAgentWindowRequest>,
) -> Result<axum::Json<OpenAgentWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_key = body.session_key.trim().to_string();
    if session_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_key is required".into(),
            }),
        ));
    }
    if body.api_key.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "api_key is required".into(),
            }),
        ));
    }
    let window_key = agent_window_key(&session_key);
    let backend = body.backend.clone();
    match backend {
        AgentTerminalBackend::Cursor => {
            let node = resolve_path(body.node_binary.as_deref(), "node binary")?;
            let script = resolve_path(body.cursor_script.as_deref(), "cursor script")?;
            let model = body
                .model
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "composer-2.5".to_string());
            let cursor_workdir = body
                .cursor_workdir
                .or_else(|| body.cwd.clone())
                .map(PathBuf::from);
            {
                let mut guard = state.coordinator.lock().expect("coordinator lock");
                guard.enqueue(UiCommand::OpenAgent {
                    window_key: window_key.clone(),
                    title: body.title,
                    session_key,
                    backend,
                    cwd: body.cwd,
                    profile: String::new(),
                    codex_home: None,
                    api_key: body.api_key,
                    codex_binary: None,
                    node_binary: Some(node),
                    cursor_script: Some(script),
                    model: Some(model),
                    cursor_workdir,
                });
            }
        }
        AgentTerminalBackend::Codex => {
            let codex_home = body.codex_home.filter(|p| !p.trim().is_empty()).ok_or((
                StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorBody {
                    error: "codex_home is required for codex backend".into(),
                }),
            ))?;
            let codex_binary = resolve_codex_binary(body.codex_binary.as_deref())?;
            let profile = body
                .profile
                .filter(|p| !p.trim().is_empty())
                .unwrap_or_else(|| "wormhole".to_string());
            {
                let mut guard = state.coordinator.lock().expect("coordinator lock");
                guard.enqueue(UiCommand::OpenAgent {
                    window_key: window_key.clone(),
                    title: body.title,
                    session_key,
                    backend,
                    cwd: body.cwd,
                    profile,
                    codex_home: Some(codex_home),
                    api_key: body.api_key,
                    codex_binary: Some(codex_binary),
                    node_binary: None,
                    cursor_script: None,
                    model: None,
                    cursor_workdir: None,
                });
            }
        }
    }
    Ok(axum::Json(OpenAgentWindowResponse { window_key }))
}

fn resolve_path(
    explicit: Option<&str>,
    label: &str,
) -> Result<PathBuf, (StatusCode, axum::Json<ApiErrorBody>)> {
    let path = explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorBody {
                    error: format!("{label} is required"),
                }),
            )
        })?;
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: format!("{label} not found: {}", path.display()),
            }),
        ));
    }
    Ok(path)
}

async fn open_computer_use(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenComputerUseWindowRequest>,
) -> Result<axum::Json<OpenComputerUseWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_key = body.session_key.trim().to_string();
    if session_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_key is required".into(),
            }),
        ));
    }
    let window_key = computer_use_window_key(&session_key);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenComputerUse {
            window_key: window_key.clone(),
            title: body.title,
            session_key,
        });
    }
    Ok(axum::Json(OpenComputerUseWindowResponse { window_key }))
}

async fn focus_computer_use(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusComputerUseWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_key = body.session_key.trim().to_string();
    if session_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_key is required".into(),
            }),
        ));
    }
    let window_key = computer_use_window_key(&session_key);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusComputerUse { window_key });
    }
    Ok("ok")
}

async fn open_live_viewer(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenLiveViewerWindowRequest>,
) -> Result<axum::Json<OpenLiveViewerWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let peer = body.peer.trim().to_string();
    if peer.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "peer is required".into(),
            }),
        ));
    }
    let window_key = live_viewer_window_key(&peer);
    let fps = body.fps.unwrap_or(30).max(1);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenLiveViewer {
            window_key: window_key.clone(),
            title: body.title,
            peer,
            password: body.password,
            totp_code: body.totp_code,
            fps,
        });
    }
    Ok(axum::Json(OpenLiveViewerWindowResponse { window_key }))
}

async fn focus_live_viewer(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusLiveViewerWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let peer = body.peer.trim().to_string();
    if peer.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "peer is required".into(),
            }),
        ));
    }
    let window_key = live_viewer_window_key(&peer);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusLiveViewer { window_key });
    }
    Ok("ok")
}

async fn open_agent_events(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenAgentEventsWindowRequest>,
) -> Result<axum::Json<OpenAgentEventsWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_key = body.session_key.trim().to_string();
    let target_node = body.target_node.trim().to_string();
    let task_id = body.task_id.trim().to_string();
    if session_key.is_empty() || target_node.is_empty() || task_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_key, target_node and task_id are required".into(),
            }),
        ));
    }
    let window_key = agent_events_window_key(&session_key);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenAgentEvents {
            window_key: window_key.clone(),
            title: body.title,
            target_node,
            task_id,
        });
    }
    Ok(axum::Json(OpenAgentEventsWindowResponse { window_key }))
}

async fn focus_agent_events(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusAgentEventsWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_key = body.session_key.trim().to_string();
    if session_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_key is required".into(),
            }),
        ));
    }
    let window_key = agent_events_window_key(&session_key);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusAgentEvents { window_key });
    }
    Ok("ok")
}

async fn focus_agent(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusAgentWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_key = body.session_key.trim().to_string();
    if session_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_key is required".into(),
            }),
        ));
    }
    let window_key = agent_window_key(&session_key);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusAgent { window_key });
    }
    Ok("ok")
}

async fn open_workspace_rdp(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenWorkspaceRdpWindowRequest>,
) -> Result<axum::Json<OpenWorkspaceRdpWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_id = body.session_id.trim().to_string();
    let peer = body.peer.trim().to_string();
    if session_id.is_empty() || peer.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_id and peer are required".into(),
            }),
        ));
    }
    if body.password.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "password is required".into(),
            }),
        ));
    }
    let window_key = workspace_window_key(&session_id);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenWorkspaceRdp {
            window_key: window_key.clone(),
            title: body.title,
            session_id,
            peer,
            password: body.password,
            clipboard_policy: body.clipboard_policy,
            watermark_text: body.watermark_text,
            fps: body.fps.unwrap_or(60),
        });
    }
    Ok(axum::Json(OpenWorkspaceRdpWindowResponse { window_key }))
}

async fn focus_workspace_rdp(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusWorkspaceRdpWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_id = body.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_id is required".into(),
            }),
        ));
    }
    let window_key = workspace_window_key(&session_id);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusWorkspaceRdp { window_key });
    }
    Ok("ok")
}

async fn open_host_control(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    _body: axum::Json<OpenHostControlWindowRequest>,
) -> Result<axum::Json<OpenHostControlWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let window_key = host_control_window_key().to_string();
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenHostControl {
            window_key: window_key.clone(),
            title: "Remote Desktop · Host".into(),
        });
    }
    Ok(axum::Json(OpenHostControlWindowResponse { window_key }))
}

async fn focus_host_control(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    _body: axum::Json<FocusHostControlWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let window_key = host_control_window_key().to_string();
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusHostControl { window_key });
    }
    Ok("ok")
}

async fn open_workspace_hud(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OpenWorkspaceHudWindowRequest>,
) -> Result<axum::Json<OpenWorkspaceHudWindowResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_id = body.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_id is required".into(),
            }),
        ));
    }
    let window_key = workspace_hud_window_key(&session_id);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::OpenWorkspaceHud {
            window_key: window_key.clone(),
            title: body.title,
            session_id,
            app_name: Some(body.app_name),
            status: Some(body.status),
            status_detail: Some(body.status_detail),
        });
    }
    Ok(axum::Json(OpenWorkspaceHudWindowResponse { window_key }))
}

async fn focus_workspace_hud(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<FocusWorkspaceHudWindowRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let session_id = body.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: "session_id is required".into(),
            }),
        ));
    }
    let window_key = workspace_hud_window_key(&session_id);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusWorkspaceHud { window_key });
    }
    Ok("ok")
}

async fn invoke_rdp(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<InvokeRdpRequest>,
) -> Result<axum::Json<InvokeRdpResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let runtime = {
        let guard = state.coordinator.lock().expect("coordinator lock");
        guard.rdp_runtime()
    };
    let runtime = runtime.lock().await;
    let response = rdp_invoke::dispatch(&runtime, body).await;
    Ok(axum::Json(response))
}

fn enqueue_and_wait<T: Send + 'static>(
    state: &DaemonState,
    build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, String>>) -> UiCommand,
) -> Result<T, (StatusCode, axum::Json<ApiErrorBody>)> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(build(tx));
    }
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody { error }),
        )),
        Err(_) => Err((
            StatusCode::GATEWAY_TIMEOUT,
            axum::Json(ApiErrorBody {
                error: "ui command timed out waiting for main thread".into(),
            }),
        )),
    }
}

async fn ui_outline(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<UiOutlineRequest>,
) -> Result<axum::Json<UiOutlineResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let format = body.format;
    let result = enqueue_and_wait(&state, |reply| UiCommand::UiOutline { format, reply })?;
    let nodes = result
        .nodes
        .iter()
        .filter_map(|n| serde_json::to_value(n).ok())
        .collect();
    Ok(axum::Json(UiOutlineResponse {
        format: result.format,
        outline: result.outline,
        nodes,
        window_id: result.window_id,
    }))
}

async fn ui_tap(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<UiTapRequest>,
) -> Result<axum::Json<UiTapResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let selector = body.selector;
    let result = enqueue_and_wait(&state, |reply| UiCommand::UiTap { selector, reply })?;
    Ok(axum::Json(UiTapResponse {
        alias: result.alias,
        label: result.label,
        x: result.x,
        y: result.y,
    }))
}

async fn ui_type(
    State(state): State<DaemonState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<UiTypeRequest>,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let text = body.text;
    enqueue_and_wait(&state, |reply| UiCommand::UiType { text, reply })?;
    Ok("ok")
}

async fn ui_app_state(
    State(state): State<DaemonState>,
    headers: HeaderMap,
) -> Result<axum::Json<UiAppStateResponse>, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    let result = enqueue_and_wait(&state, |reply| UiCommand::UiAppState { reply })?;
    Ok(axum::Json(UiAppStateResponse {
        main_window_id: result.main_window_id,
        pid: std::process::id(),
        target_count: result.target_count,
    }))
}

async fn shutdown(
    State(state): State<DaemonState>,
    headers: HeaderMap,
) -> Result<&'static str, (StatusCode, axum::Json<ApiErrorBody>)> {
    authorize(&headers, &state.token)?;
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::Shutdown);
    }
    Ok("ok")
}

fn resolve_codex_binary(
    explicit: Option<&str>,
) -> Result<PathBuf, (StatusCode, axum::Json<ApiErrorBody>)> {
    if let Some(path) = explicit.filter(|p| !p.trim().is_empty()) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err((
            StatusCode::BAD_REQUEST,
            axum::Json(ApiErrorBody {
                error: format!("codex binary not found: {}", path.display()),
            }),
        ));
    }
    if let Ok(path) = std::env::var("WORMHOLE_CODEX") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Ok(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["codex.exe", "codex"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
    }
    Err((
        StatusCode::BAD_REQUEST,
        axum::Json(ApiErrorBody {
            error: "codex binary not found".into(),
        }),
    ))
}
