//! Desktop shell UI preferences (`{data_dir}/desktop-ui.json`).

use std::path::Path;

use serde::{Deserialize, Serialize};

pub const PREFS_FILE: &str = "desktop-ui.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopUiPrefs {
    #[serde(default)]
    pub onboarding_dismissed: bool,
    /// Persisted [`AppTab`](crate::ui::app_shell::AppTab) id, e.g. `"chat"`.
    #[serde(default)]
    pub last_tab: Option<String>,
}

pub fn load(data_dir: &Path) -> DesktopUiPrefs {
    let path = data_dir.join(PREFS_FILE);
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => DesktopUiPrefs::default(),
    }
}

pub fn save(data_dir: &Path, prefs: &DesktopUiPrefs) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|err| format!("无法创建数据目录: {err}"))?;
    let bytes = serde_json::to_vec_pretty(prefs)
        .map_err(|err| format!("无法序列化桌面 UI 偏好: {err}"))?;
    std::fs::write(data_dir.join(PREFS_FILE), bytes)
        .map_err(|err| format!("无法写入桌面 UI 偏好: {err}"))
}

pub fn update(data_dir: &Path, mutate: impl FnOnce(&mut DesktopUiPrefs)) -> Result<(), String> {
    let mut prefs = load(data_dir);
    mutate(&mut prefs);
    save(data_dir, &prefs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_last_tab() {
        let dir = std::env::temp_dir().join(format!(
            "wormhole-desktop-ui-prefs-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let prefs = DesktopUiPrefs {
            onboarding_dismissed: true,
            last_tab: Some("warp".into()),
        };
        save(&dir, &prefs).expect("save");
        assert_eq!(load(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
