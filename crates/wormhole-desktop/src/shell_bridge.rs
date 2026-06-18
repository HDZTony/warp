//! HTTP client for Tauri `mcp-bridge.json` (virtual camera and other shell-only APIs).

use std::path::Path;

use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize)]
struct BridgeSessionFile {
    endpoint: String,
    token: String,
}

fn load_session(data_dir: &Path) -> Option<BridgeSessionFile> {
    let raw = std::fs::read_to_string(data_dir.join("mcp-bridge.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn post_command(data_dir: &Path, command: &str, body: serde_json::Value) -> Result<(), String> {
    let session = load_session(data_dir)
        .ok_or_else(|| "mcp-bridge.json missing (start Wormhole Desktop)".to_string())?;
    let url = format!(
        "{}/commands/{}",
        session.endpoint.trim_end_matches('/'),
        command
    );
    let response = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", session.token))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| e.to_string())?;
    if response.status() >= 400 {
        return Err(format!("bridge HTTP {}", response.status()));
    }
    Ok(())
}

pub fn virtual_cam_config(data_dir: &Path) -> Option<(bool, String)> {
    let session = load_session(data_dir)?;
    let url = format!(
        "{}/commands/remote_desktop_virtual_cam_config",
        session.endpoint.trim_end_matches('/')
    );
    let response = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", session.token))
        .set("Content-Type", "application/json")
        .send_json(json!({}))
        .ok()?;
    if response.status() >= 400 {
        return None;
    }
    let body: serde_json::Value = response.into_json().ok()?;
    let available = body
        .get("available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let hint = body
        .get("hint")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some((available, hint))
}

pub fn set_virtual_cam(
    data_dir: &Path,
    enabled: bool,
    width: u32,
    height: u32,
) -> Result<(), String> {
    post_command(
        data_dir,
        "remote_desktop_set_virtual_cam",
        json!({
            "enabled": enabled,
            "width": width,
            "height": height,
            "fps": 30,
        }),
    )
}

pub fn push_virtual_cam_rgb(
    data_dir: &Path,
    width: u32,
    height: u32,
    rgb: &[u8],
) -> Result<(), String> {
    post_command(
        data_dir,
        "remote_desktop_virtual_cam_push_rgb",
        json!({
            "width": width,
            "height": height,
            "rgb": rgb,
        }),
    )
}
