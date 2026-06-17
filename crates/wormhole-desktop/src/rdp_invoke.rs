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
        "ensure_host_with_auth_policy" => {
            let args: EnsureHostWithAuthPolicyArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .ensure_host_with_auth_policy(&args.settings, args.auth_policy)
                .await
            {
                Ok(()) => ok(Value::Null),
                Err(error) => err(error),
            }
        }
        "headless_host_endpoint" => match runtime.headless_host_endpoint().await {
            Ok(endpoint) => ok(serde_json::to_value(endpoint).unwrap_or(Value::Null)),
            Err(error) => err(error),
        },
        "node_id" => match runtime.node_id_string().await {
            Ok(value) => ok(json!(value)),
            Err(error) => err(error),
        },
        "remote_agent_submit_task" => {
            let args: RemoteAgentSubmitArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .submit_agent_task(&args.target_node, args.request)
                .await
            {
                Ok(result) => ok(result),
                Err(error) => err(error),
            }
        }
        "remote_agent_read_events" => {
            let args: RemoteAgentReadEventsArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .remote_agent_read_events(&args.target_node, &args.task_id, args.since)
                .await
            {
                Ok(page) => ok(page),
                Err(error) => err(error),
            }
        }
        "remote_agent_capabilities" => {
            let args: RemoteAgentPeerArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime.remote_agent_capabilities(&args.target_node).await {
                Ok(caps) => ok(caps),
                Err(error) => err(error),
            }
        }
        "remote_desktop_send_file" => {
            let args: SendFileArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .send_file_to_peer(&args.peer, &args.local_path, &args.transfer_id)
                .await
            {
                Ok(bytes) => ok(json!(bytes)),
                Err(error) => err(error),
            }
        }
        "remote_desktop_file_transfer_status" => {
            let args: FileTransferStatusArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime.file_transfer_status(&args.transfer_id) {
                Some(progress) => ok(serde_json::to_value(progress).unwrap_or(Value::Null)),
                None => err(format!("unknown transfer_id: {}", args.transfer_id)),
            }
        }
        "remote_desktop_open_tunnel" => {
            let args: TunnelArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .open_tunnel(&args.peer, &args.local_bind, &args.remote_host, args.remote_port)
                .await
            {
                Ok(()) => ok(Value::Null),
                Err(error) => err(error),
            }
        }
        "remote_desktop_run_terminal" => {
            let args: TerminalArgs = match serde_json::from_value(request.args) {
                Ok(args) => args,
                Err(error) => return err(format!("invalid args: {error}")),
            };
            match runtime
                .run_terminal_command(&args.peer, &args.command, &args.args)
                .await
            {
                Ok(messages) => ok(serde_json::to_value(messages).unwrap_or(Value::Null)),
                Err(error) => err(error),
            }
        }
        "remote_desktop_start_live" => {
            let args: wormhole_desktop_rdp::live_host::StartLiveHostArgs =
                match serde_json::from_value(request.args) {
                    Ok(args) => args,
                    Err(error) => return err(format!("invalid args: {error}")),
                };
            match runtime.start_live_host(args).await {
                Ok(session_id) => ok(Value::String(session_id)),
                Err(error) => err(error),
            }
        }
        "remote_desktop_viewer_set_audio_volume" => {
            let volume = request
                .args
                .get("volume")
                .and_then(|v| v.as_u64())
                .unwrap_or(100)
                .min(100) as u8;
            match runtime.set_viewer_audio_volume(volume).await {
                Ok(()) => ok(Value::Null),
                Err(error) => err(error),
            }
        }
        "remote_desktop_viewer_set_audio_muted" => {
            let muted = request
                .args
                .get("muted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match runtime.set_viewer_audio_muted(muted).await {
                Ok(()) => ok(Value::Null),
                Err(error) => err(error),
            }
        }
        "remote_desktop_viewer_audio_stats" => {
            let session_id = request
                .args
                .get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if session_id.is_empty() {
                return err("session_id is required".into());
            }
            match runtime.viewer_audio_stats(&session_id).await {
                Ok(stats) => ok(serde_json::to_value(stats).unwrap_or(Value::Null)),
                Err(error) => err(error),
            }
        }
        other => err(format!("unknown RDP command: {other}")),
    }
}

#[derive(Debug, Deserialize)]
struct EnsureHostWithAuthPolicyArgs {
    settings: settings::RdpSettings,
    auth_policy: display_server::rdp_auth::HostAuthPolicy,
}

#[derive(Debug, Deserialize)]
struct SendFileArgs {
    peer: String,
    local_path: String,
    transfer_id: String,
}

#[derive(Debug, Deserialize)]
struct FileTransferStatusArgs {
    transfer_id: String,
}

#[derive(Debug, Deserialize)]
struct TunnelArgs {
    peer: String,
    local_bind: String,
    remote_host: String,
    remote_port: u16,
}

#[derive(Debug, Deserialize)]
struct TerminalArgs {
    peer: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteAgentPeerArgs {
    target_node: String,
}

#[derive(Debug, Deserialize)]
struct RemoteAgentSubmitArgs {
    target_node: String,
    request: Value,
}

#[derive(Debug, Deserialize)]
struct RemoteAgentReadEventsArgs {
    target_node: String,
    task_id: String,
    #[serde(default)]
    since: Option<u64>,
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
