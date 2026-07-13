use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const HEALTH_PATH: &str = "/health";
pub const OPEN_RDP_PATH: &str = "/open-rdp";
pub const FOCUS_RDP_PATH: &str = "/focus-rdp";
pub const OPEN_AGENT_PATH: &str = "/open-agent";
pub const FOCUS_AGENT_PATH: &str = "/focus-agent";
pub const OPEN_COMPUTER_USE_PATH: &str = "/open-computer-use";
pub const FOCUS_COMPUTER_USE_PATH: &str = "/focus-computer-use";
pub const OPEN_LIVE_VIEWER_PATH: &str = "/open-live-viewer";
pub const FOCUS_LIVE_VIEWER_PATH: &str = "/focus-live-viewer";
pub const OPEN_AGENT_EVENTS_PATH: &str = "/open-agent-events";
pub const FOCUS_AGENT_EVENTS_PATH: &str = "/focus-agent-events";
pub const OPEN_WORKSPACE_RDP_PATH: &str = "/open-workspace-rdp";
pub const FOCUS_WORKSPACE_RDP_PATH: &str = "/focus-workspace-rdp";
pub const OPEN_HOST_CONTROL_PATH: &str = "/open-host-control";
pub const FOCUS_HOST_CONTROL_PATH: &str = "/focus-host-control";
pub const OPEN_WORKSPACE_HUD_PATH: &str = "/open-workspace-hud";
pub const FOCUS_WORKSPACE_HUD_PATH: &str = "/focus-workspace-hud";
pub const INVOKE_RDP_PATH: &str = "/invoke-rdp";
pub const SHUTDOWN_PATH: &str = "/shutdown";

pub fn session_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("native-ui-session.json")
}

pub fn rdp_window_key(peer: &str) -> String {
    format!("rdp:{peer}")
}

pub fn agent_window_key(session_key: &str) -> String {
    format!("agent:{session_key}")
}

pub fn computer_use_window_key(session_key: &str) -> String {
    format!("computer-use:{session_key}")
}

pub fn live_viewer_window_key(peer: &str) -> String {
    format!("live-viewer:{peer}")
}

pub fn agent_events_window_key(session_key: &str) -> String {
    format!("agent-events:{session_key}")
}

pub fn workspace_window_key(session_id: &str) -> String {
    format!("workspace-rdp:{session_id}")
}

pub fn host_control_window_key() -> &'static str {
    "host-control"
}

pub fn workspace_hud_window_key(session_id: &str) -> String {
    format!("workspace-hud:{session_id}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSessionFile {
    pub endpoint: String,
    pub token: String,
    #[serde(default)]
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentTerminalBackend {
    Codex,
    Cursor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRdpWindowRequest {
    pub peer: String,
    pub title: String,
    #[serde(default)]
    pub reconnect: bool,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub totp_code: Option<String>,
    #[serde(default)]
    pub fps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRdpWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusRdpWindowRequest {
    pub peer: String,
    #[serde(default)]
    pub reconnect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAgentWindowRequest {
    pub session_key: String,
    pub title: String,
    pub backend: AgentTerminalBackend,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub codex_home: Option<String>,
    pub api_key: String,
    #[serde(default)]
    pub codex_binary: Option<String>,
    #[serde(default)]
    pub node_binary: Option<String>,
    #[serde(default)]
    pub cursor_script: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cursor_workdir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAgentWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusAgentWindowRequest {
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenComputerUseWindowRequest {
    pub session_key: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenComputerUseWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusComputerUseWindowRequest {
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenLiveViewerWindowRequest {
    pub peer: String,
    pub title: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub totp_code: Option<String>,
    #[serde(default)]
    pub fps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenLiveViewerWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusLiveViewerWindowRequest {
    pub peer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAgentEventsWindowRequest {
    pub session_key: String,
    pub title: String,
    pub target_node: String,
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAgentEventsWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusAgentEventsWindowRequest {
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorkspaceRdpWindowRequest {
    pub session_id: String,
    pub peer: String,
    pub password: String,
    pub title: String,
    #[serde(default)]
    pub clipboard_policy: Option<String>,
    #[serde(default)]
    pub watermark_text: Option<String>,
    #[serde(default)]
    pub fps: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorkspaceRdpWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusWorkspaceRdpWindowRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenHostControlWindowRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHostControlWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FocusHostControlWindowRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorkspaceHudWindowRequest {
    pub session_id: String,
    pub title: String,
    pub app_name: String,
    pub status: String,
    pub status_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorkspaceHudWindowResponse {
    pub window_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusWorkspaceHudWindowRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRdpRequest {
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRdpResponse {
    pub ok: bool,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::rdp_window_key;

    #[test]
    fn rdp_window_key_is_stable_per_peer() {
        assert_eq!(rdp_window_key("abc123"), "rdp:abc123");
        assert_eq!(rdp_window_key("abc123"), rdp_window_key("abc123"));
        assert_ne!(rdp_window_key("a"), rdp_window_key("b"));
    }
}
