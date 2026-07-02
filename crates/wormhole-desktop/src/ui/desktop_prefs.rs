//! Desktop shell UI preferences (`{data_dir}/desktop-ui.json`).

use std::path::Path;

use serde::{Deserialize, Serialize};

pub const PREFS_FILE: &str = "desktop-ui.json";

/// Demo balance default matching [`desktop-current.html`](../../../../docs/design/desktop-current.html).
pub const DEFAULT_BALANCE_CENTS: i64 = 37_550;

fn default_balance_cents() -> i64 {
    DEFAULT_BALANCE_CENTS
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopUiPrefs {
    #[serde(default)]
    pub onboarding_dismissed: bool,
    /// Persisted [`AppTab`](crate::ui::app_shell::AppTab) id, e.g. `"chat"`.
    #[serde(default)]
    pub last_tab: Option<String>,
    /// Demo wallet balance in cents (¥375.50 = 37550).
    #[serde(default = "default_balance_cents")]
    pub balance_cents: i64,
}

impl Default for DesktopUiPrefs {
    fn default() -> Self {
        Self {
            onboarding_dismissed: false,
            last_tab: None,
            balance_cents: default_balance_cents(),
        }
    }
}

/// Formats cents as `¥1,234.56` (HTML `formatMoney` parity).
pub fn format_balance_yuan(cents: i64) -> String {
    let safe = cents.max(0);
    let yuan = format!("{:.2}", safe as f64 / 100.0);
    let (int_part, dec) = yuan.split_once('.').unwrap_or((&yuan, "00"));
    let mut grouped = String::new();
    for (i, ch) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let int_grouped: String = grouped.chars().rev().collect();
    format!("¥{int_grouped}.{dec}")
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
    let bytes =
        serde_json::to_vec_pretty(prefs).map_err(|err| format!("无法序列化桌面 UI 偏好: {err}"))?;
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
        let dir =
            std::env::temp_dir().join(format!("wormhole-desktop-ui-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let prefs = DesktopUiPrefs {
            onboarding_dismissed: true,
            last_tab: Some("warp".into()),
            balance_cents: DEFAULT_BALANCE_CENTS,
        };
        save(&dir, &prefs).expect("save");
        assert_eq!(load(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_balance_yuan_matches_html_default() {
        assert_eq!(format_balance_yuan(37_550), "¥375.50");
        assert_eq!(format_balance_yuan(100_000), "¥1,000.00");
        assert_eq!(format_balance_yuan(0), "¥0.00");
        assert_eq!(format_balance_yuan(-5), "¥0.00");
    }

    #[test]
    fn missing_balance_field_uses_default() {
        let dir = std::env::temp_dir().join(format!(
            "wormhole-desktop-ui-prefs-balance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join(PREFS_FILE),
            r#"{"onboarding_dismissed":false}"#,
        )
        .expect("write");
        assert_eq!(load(&dir).balance_cents, DEFAULT_BALANCE_CENTS);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
