use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;
use uuid::Uuid;
use wormhole_native_ipc::{
    session_file_path, ApiErrorBody, BridgeSessionFile, FocusRdpWindowRequest, InvokeRdpRequest,
    InvokeRdpResponse, OpenRdpWindowRequest, OpenRdpWindowResponse, FOCUS_RDP_PATH, HEALTH_PATH,
    INVOKE_RDP_PATH, OPEN_RDP_PATH, SHUTDOWN_PATH,
};

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
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
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
        .route(INVOKE_RDP_PATH, post(invoke_rdp))
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

fn authorize(headers: &HeaderMap, expected: &str) -> Result<(), (StatusCode, axum::Json<ApiErrorBody>)> {
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
    let window_key = wormhole_native_ipc::rdp_window_key(&peer);
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
    let window_key = wormhole_native_ipc::rdp_window_key(&peer);
    {
        let mut guard = state.coordinator.lock().expect("coordinator lock");
        guard.enqueue(UiCommand::FocusRdp {
            window_key,
            reconnect: body.reconnect,
        });
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
