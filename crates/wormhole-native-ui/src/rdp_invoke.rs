use serde::Deserialize;
use serde_json::{json, Value};
use wormhole_desktop_rdp::{RdpRuntime, settings};
use wormhole_native_ipc::{InvokeRdpRequest, InvokeRdpResponse};

#[derive(Debug, Deserialize)]
struct StartViewerArgs {
    host: String,
    #[serde(default)]
    fps: Option<i32>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    totp_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AcceptHostArgs {
    #[serde(default)]
    fps: Option<i32>,
    #[serde(default)]
    use_vram: Option<bool>,
    #[serde(default)]
    monitor: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct PeerArgs {
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct SessionArgs {
    session_id: String,
}

pub async fn dispatch(runtime: &RdpRuntime, request: InvokeRdpRequest) -> InvokeRdpResponse {
    match request.command.as_str() {
        "remote_desktop_config" => match runtime.remote_desktop_config().await {
            Ok(value) => ok(serde_json::to_value(value).unwrap_or(Value::Null)),
            Err(error) => err(error),
        },
        "remote_desktop_list_sessions" => match runtime.list_sessions().await {
            Ok(value) => ok(serde_json::to_value(value).unwrap_or(Value::Null)),
            Err(error) => err(error),
        },
        "remote_desktop_start" => {
            let args: StartViewerArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            let fps = args.fps.unwrap_or(60);
            match runtime
                .start_viewer_session(
                    &args.host,
                    fps,
                    args.password.as_deref(),
                    args.totp_code.as_deref(),
                )
                .await
            {
                Ok(session_id) => ok(Value::String(session_id)),
                Err(error) => err(error),
            }
        }
        "remote_desktop_accept" => {
            let args: AcceptHostArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .accept_host(
                    args.fps.unwrap_or(60),
                    args.use_vram.unwrap_or(false),
                    args.monitor.unwrap_or(0),
                )
                .await
            {
                Ok(session_id) => ok(Value::String(session_id)),
                Err(error) => err(error),
            }
        }
        "remote_desktop_stop" => {
            let args: SessionArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime.stop_session(&args.session_id).await {
                Ok(()) => ok(Value::Null),
                Err(error) => err(error),
            }
        }
        "remote_desktop_stop_all" => match runtime.stop_all_sessions().await {
            Ok(()) => ok(Value::Null),
            Err(error) => err(error),
        },
        "remote_desktop_peer_live_status" => {
            let args: PeerArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime.peer_live_status(&args.node_id).await {
                Ok(value) => ok(serde_json::to_value(value).unwrap_or(Value::Null)),
                Err(error) => err(error),
            }
        }
        "ensure_unattended_host" => {
            let settings: settings::RdpSettings = match serde_json::from_value(request.args) {
                Ok(value) => value,
                Err(error) => return err(format!("invalid settings: {error}")),
            };
            match runtime.ensure_unattended_host(&settings).await {
                Ok(()) => ok(Value::Null),
                Err(error) => err(error),
            }
        }
        "node_id" => match runtime.node_id_string().await {
            Ok(value) => ok(json!(value)),
            Err(error) => err(error),
        },
        other => err(format!("unknown RDP command: {other}")),
    }
}

fn ok(result: Value) -> InvokeRdpResponse {
    InvokeRdpResponse {
        ok: true,
        result: Some(result),
        error: None,
    }
}

fn err(error: String) -> InvokeRdpResponse {
    InvokeRdpResponse {
        ok: false,
        result: None,
        error: Some(error),
    }
}
