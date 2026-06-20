//! Wormhole-local telemetry + crash capture for embedded Warp (replaces Warp Rudderstack / Sentry).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

static PANIC_HOOK: Once = Once::new();

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn telemetry_dir(data_dir: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("WORMHOLE_TELEMETRY_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    data_dir.join("telemetry")
}

pub fn crashes_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("crashes")
}

fn telemetry_file(data_dir: &Path) -> PathBuf {
    telemetry_dir(data_dir).join("events.ndjson")
}

pub fn install(data_dir: &Path) {
    let data_dir = data_dir.to_path_buf();
    let _ = std::fs::create_dir_all(telemetry_dir(&data_dir));
    let _ = std::fs::create_dir_all(crashes_dir(&data_dir));
    let hook_dir = data_dir.clone();
    PANIC_HOOK.call_once(move || {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = format!("{info}");
            let backtrace = std::backtrace::Backtrace::force_capture();
            let _ = write_crash_report(&hook_dir, &payload, &format!("{backtrace}"));
            record_event(
                &hook_dir,
                "warp_embed_panic",
                json!({ "message": payload }),
            );
            default(info);
        }));
    });
    record_event(
        &data_dir,
        "warp_embed_start",
        json!({ "component": "warp-oss-wormhole" }),
    );
}

#[derive(Serialize)]
struct TelemetryEvent<'a> {
    ts: u64,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
}

pub fn record_event(data_dir: &Path, name: &str, payload: Value) {
    let event = TelemetryEvent {
        ts: unix_now(),
        name,
        payload: Some(payload),
    };
    if let Ok(line) = serde_json::to_string(&event) {
        let _ = append_line(&telemetry_file(data_dir), &line);
    }
}

fn write_crash_report(data_dir: &Path, message: &str, backtrace: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(crashes_dir(data_dir)).map_err(|e| e.to_string())?;
    let path = crashes_dir(data_dir).join(format!(
        "warp-embed-crash-{}-{}.log",
        unix_now(),
        std::process::id()
    ));
    let body = format!("message:\n{message}\n\nbacktrace:\n{backtrace}\n");
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

fn append_line(path: &Path, line: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())
}
