//! Environment and window helpers when Warp runs embedded inside Wormhole desktop.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub mod prefs;
pub mod remote_queue;
pub mod observability;
pub mod slim;
pub use prefs::{PreferredAgent, WarpEmbedPrefs};
pub use slim::{apply_slim_feature_flags, warp_cloud_disabled};

static EMBED_BOOTSTRAPPED: AtomicBool = AtomicBool::new(false);

pub const EMBEDDED_ENV: &str = "WORMHOLE_EMBEDDED";
pub const DATA_DIR_ENV: &str = "WORMHOLE_DATA_DIR";
pub const PARENT_HWND_ENV: &str = "WORMHOLE_PARENT_HWND";
pub const CODEX_PROFILE_ENV: &str = "WORMHOLE_CODEX_PROFILE";
pub const SPAWN_CODEX_ENV: &str = "WORMHOLE_SPAWN_CODEX";
pub const SPAWN_CURSOR_ENV: &str = "WORMHOLE_SPAWN_CURSOR";
pub const PREFERRED_AGENT_ENV: &str = "WORMHOLE_PREFERRED_AGENT";
pub const CURSOR_API_KEY_ENV: &str = "CURSOR_API_KEY";
pub const CURSOR_MODEL_ENV: &str = "WORMHOLE_CURSOR_MODEL";

/// Whether Warp was launched by Wormhole as an embedded child process.
pub fn is_embedded() -> bool {
    std::env::var(EMBEDDED_ENV)
        .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

pub fn should_spawn_agent_tab() -> bool {
    is_embedded()
        && std::env::var(SPAWN_CODEX_ENV)
            .map(|v| matches!(v.trim(), "1" | "true" | "yes"))
            .unwrap_or(true)
}

pub fn should_spawn_codex_tab() -> bool {
    should_spawn_agent_tab()
}

pub fn preferred_agent() -> PreferredAgent {
    std::env::var(PREFERRED_AGENT_ENV)
        .ok()
        .map(|value| PreferredAgent::parse(&value))
        .unwrap_or_else(|| {
            data_dir()
                .map(|dir| prefs::load_prefs(&dir).preferred_agent)
                .unwrap_or_default()
        })
}

pub fn data_dir() -> Option<PathBuf> {
    std::env::var(DATA_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn codex_profile() -> String {
    std::env::var(CODEX_PROFILE_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "wormhole".to_string())
}

/// Shell command used to open Codex CLI in an embedded terminal tab.
pub fn default_codex_launch_command() -> String {
    let profile = codex_profile();
    if let Ok(bin) = std::env::var("WORMHOLE_CODEX_BIN") {
        let bin = bin.trim();
        if !bin.is_empty() {
            if bin.contains(' ') {
                return format!("\"{bin}\" --profile {profile}");
            }
            return format!("{bin} --profile {profile}");
        }
    }
    format!("codex --profile {profile}")
}

/// Shell command used to open Cursor CLI (`agent`) in an embedded terminal tab.
pub fn default_cursor_launch_command() -> String {
    if let Ok(model) = std::env::var(CURSOR_MODEL_ENV) {
        let model = model.trim();
        if !model.is_empty() {
            return format!("agent --model {model}");
        }
    }
    "agent".to_string()
}

pub fn default_launch_command() -> String {
    match preferred_agent() {
        PreferredAgent::Codex => default_codex_launch_command(),
        PreferredAgent::Cursor => default_cursor_launch_command(),
    }
}

/// Returns true only once per process when embedded agent bootstrap should run.
pub fn take_embed_bootstrap_slot() -> bool {
    should_spawn_agent_tab() && !EMBED_BOOTSTRAPPED.swap(true, Ordering::SeqCst)
}

pub fn parent_hwnd() -> Option<isize> {
    parse_hwnd_env(PARENT_HWND_ENV)
}

pub fn parse_hwnd_env(name: &str) -> Option<isize> {
    let raw = std::env::var(name).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        isize::from_str_radix(hex, 16).ok().filter(|v| *v != 0)
    } else {
        trimmed.parse().ok().filter(|v| *v != 0)
    }
}

/// Inject Codex-related environment for a Wormhole-managed child Warp process.
pub fn apply_codex_launch_env(
    data_dir: &Path,
    codex_home: &Path,
    api_key: &str,
    codex_bin: Option<&Path>,
) {
    std::env::set_var(EMBEDDED_ENV, "1");
    std::env::set_var(DATA_DIR_ENV, data_dir);
    std::env::set_var("CODEX_HOME", codex_home);
    if !api_key.trim().is_empty() {
        std::env::set_var("WORMHOLE_AGENT_API_KEY", api_key.trim());
        std::env::set_var("WORMHOLE_DEEPSEEK_API_KEY", api_key.trim());
    }
    std::env::set_var(CODEX_PROFILE_ENV, codex_profile());
    std::env::set_var(SPAWN_CODEX_ENV, "1");
    if let Some(bin) = codex_bin {
        if let Some(parent) = bin.parent() {
            prepend_path(parent);
        }
        std::env::set_var("WORMHOLE_CODEX_BIN", bin);
    }
}

/// Inject Codex + Cursor environment for embedded Warp.
pub fn apply_embed_launch_env(
    data_dir: &Path,
    codex_home: &Path,
    codex_api_key: &str,
    codex_bin: Option<&Path>,
    cursor_api_key: &str,
    cursor_model: &str,
    preferred: PreferredAgent,
) {
    apply_codex_launch_env(data_dir, codex_home, codex_api_key, codex_bin);
    if !cursor_api_key.trim().is_empty() {
        std::env::set_var(CURSOR_API_KEY_ENV, cursor_api_key.trim());
    }
    if !cursor_model.trim().is_empty() {
        std::env::set_var(CURSOR_MODEL_ENV, cursor_model.trim());
    }
    std::env::set_var(PREFERRED_AGENT_ENV, preferred.as_str());
    std::env::set_var(SPAWN_CURSOR_ENV, "1");
}

fn prepend_path(dir: &Path) {
    let dir_buf = dir.to_path_buf();
    match std::env::var_os("PATH") {
        Some(existing) => {
            let mut paths: Vec<PathBuf> = std::env::split_paths(&existing).collect();
            paths.insert(0, dir_buf);
            if let Ok(merged) = std::env::join_paths(paths) {
                std::env::set_var("PATH", merged);
            }
        }
        None => {
            if let Ok(merged) = std::env::join_paths([&dir]) {
                std::env::set_var("PATH", merged);
            }
        }
    }
}

/// Build environment map for spawning `warp-oss-wormhole`.
pub fn spawn_env_map(
    data_dir: &Path,
    codex_home: &Path,
    api_key: &str,
    codex_bin: Option<&Path>,
    parent_hwnd: Option<isize>,
    cursor_api_key: &str,
    cursor_model: &str,
    preferred: PreferredAgent,
) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    vars.push((EMBEDDED_ENV.to_string(), "1".to_string()));
    vars.push((DATA_DIR_ENV.to_string(), data_dir.display().to_string()));
    vars.push(("CODEX_HOME".to_string(), codex_home.display().to_string()));
    vars.push((CODEX_PROFILE_ENV.to_string(), codex_profile()));
    vars.push((SPAWN_CODEX_ENV.to_string(), "1".to_string()));
    vars.push((SPAWN_CURSOR_ENV.to_string(), "1".to_string()));
    vars.push((
        PREFERRED_AGENT_ENV.to_string(),
        preferred.as_str().to_string(),
    ));
    if !api_key.trim().is_empty() {
        vars.push((
            "WORMHOLE_AGENT_API_KEY".to_string(),
            api_key.trim().to_string(),
        ));
        vars.push((
            "WORMHOLE_DEEPSEEK_API_KEY".to_string(),
            api_key.trim().to_string(),
        ));
    }
    if let Some(bin) = codex_bin {
        vars.push(("WORMHOLE_CODEX_BIN".to_string(), bin.display().to_string()));
        if let Some(parent) = bin.parent() {
            vars.push((
                "WORMHOLE_CODEX_BIN_DIR".to_string(),
                parent.display().to_string(),
            ));
        }
    }
    if !cursor_api_key.trim().is_empty() {
        vars.push((
            CURSOR_API_KEY_ENV.to_string(),
            cursor_api_key.trim().to_string(),
        ));
    }
    if !cursor_model.trim().is_empty() {
        vars.push((
            CURSOR_MODEL_ENV.to_string(),
            cursor_model.trim().to_string(),
        ));
    }
    if let Some(hwnd) = parent_hwnd.filter(|v| *v != 0) {
        vars.push((PARENT_HWND_ENV.to_string(), format!("{hwnd}")));
    }
    vars
}

pub fn resolve_warp_child_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("WORMHOLE_WARP_BIN") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    for name in ["warp-oss-wormhole.exe", "warp-oss-wormhole"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(windows)]
pub mod win32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hwnd_decimal_and_hex() {
        std::env::set_var("TEST_HWND", "12345");
        assert_eq!(parse_hwnd_env("TEST_HWND"), Some(12345));
        std::env::set_var("TEST_HWND", "0xABC");
        assert_eq!(parse_hwnd_env("TEST_HWND"), Some(0xABC));
    }

    #[test]
    fn default_codex_command_uses_profile() {
        std::env::set_var("WORMHOLE_CODEX_PROFILE", "wormhole");
        let cmd = default_codex_launch_command();
        assert!(cmd.contains("--profile wormhole"));
    }

    #[test]
    fn default_cursor_command_uses_model_env() {
        std::env::set_var(CURSOR_MODEL_ENV, "composer-2.5");
        let cmd = default_cursor_launch_command();
        assert_eq!(cmd, "agent --model composer-2.5");
    }

    #[test]
    fn spawn_env_map_includes_cursor_and_preferred_agent() {
        let vars = spawn_env_map(
            Path::new("C:\\Wormhole"),
            Path::new("C:\\Wormhole\\codex"),
            "codex-key",
            None,
            None,
            "cursor-key",
            "composer-2",
            PreferredAgent::Cursor,
        );
        assert!(vars
            .iter()
            .any(|(k, v)| k == CURSOR_API_KEY_ENV && v == "cursor-key"));
        assert!(vars
            .iter()
            .any(|(k, v)| k == PREFERRED_AGENT_ENV && v == "cursor"));
    }
}
