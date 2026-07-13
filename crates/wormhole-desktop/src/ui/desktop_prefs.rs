//! Desktop shell UI preferences (`{data_dir}/desktop-ui.json`).

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use wormhole_desktop_core::cloud_credits::CloudCreditLedgerEntryDto;

pub const PREFS_FILE: &str = "desktop-ui.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedeemHistoryEntry {
    pub time: String,
    pub code: String,
    #[serde(alias = "amount_cents")]
    pub amount_credits: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopUiPrefs {
    #[serde(default)]
    pub onboarding_dismissed: bool,
    /// Persisted [`AppTab`](crate::ui::app_shell::AppTab) id, e.g. `"chat"`.
    #[serde(default)]
    pub last_tab: Option<String>,
    /// Legacy field; redeem history is loaded from control plane ledger.
    #[serde(default)]
    pub redeem_history: Vec<RedeemHistoryEntry>,
    /// Maps server `cardKeyId` → user-entered redeem code for display.
    #[serde(default)]
    pub redeem_code_overrides: HashMap<String, String>,
}

impl Default for DesktopUiPrefs {
    fn default() -> Self {
        Self {
            onboarding_dismissed: false,
            last_tab: None,
            redeem_history: Vec::new(),
            redeem_code_overrides: HashMap::new(),
        }
    }
}

pub fn format_account_balance(credits_micro: i64) -> String {
    format!("¥{}", format_yuan_from_micro(credits_micro.max(0)))
}

/// Formats redeem history amounts (`1 credit = 1元 = 1_000_000 micro`) with a leading `+`.
pub fn format_redeem_amount(amount_credits_micro: i64) -> String {
    format!("+{}", format_account_balance(amount_credits_micro))
}

/// Prefer server-provided yuan string when present; otherwise format micro credits.
pub fn format_balance_display(amount_yuan: Option<&str>, credits_micro: i64) -> String {
    if let Some(raw) = amount_yuan.map(str::trim).filter(|value| !value.is_empty()) {
        if raw.starts_with('¥') || raw.starts_with('+') {
            return raw.to_string();
        }
        return format!("¥{raw}");
    }
    format_account_balance(credits_micro)
}

fn format_yuan_from_micro(micro: i64) -> String {
    const MICRO_PER_CREDIT: i64 = 1_000_000;
    let abs = micro.unsigned_abs();
    let whole = abs / MICRO_PER_CREDIT as u64;
    let frac = abs % MICRO_PER_CREDIT as u64;
    let whole_grouped = group_digits(&whole.to_string());
    if frac == 0 {
        return format!("{whole_grouped}.00");
    }
    let frac_str = format!("{frac:06}");
    let frac_trim = frac_str.trim_end_matches('0');
    if frac_trim.len() == 1 {
        return format!("{whole_grouped}.{frac_trim}0");
    }
    if frac_trim.len() == 2 {
        return format!("{whole_grouped}.{frac_trim}");
    }
    format!("{whole_grouped}.{frac_trim}")
}

pub fn redeem_history_from_ledger(
    entries: &[CloudCreditLedgerEntryDto],
    code_overrides: &HashMap<String, String>,
) -> Vec<RedeemHistoryEntry> {
    entries
        .iter()
        .filter(|entry| entry.source == "card_key_redeem" && entry.delta_credits > 0)
        .map(|entry| {
            let card_key_id = metadata_string(&entry.metadata, "cardKeyId");
            let code = card_key_id
                .as_ref()
                .and_then(|id| code_overrides.get(id))
                .cloned()
                .unwrap_or_else(|| ledger_entry_code_label(entry, card_key_id.as_deref()));
            RedeemHistoryEntry {
                time: ledger_entry_time(
                    entry.created_at,
                    metadata_string(&entry.metadata, "redeemedAt").as_deref(),
                ),
                code,
                amount_credits: entry.delta_credits,
            }
        })
        .collect()
}

pub fn redeem_history_time(redeemed_at: Option<&str>) -> String {
    if let Some(raw) = redeemed_at.map(str::trim).filter(|value| !value.is_empty()) {
        if raw.len() >= 19 {
            return raw[..19].replace('T', " ");
        }
        return raw.to_string();
    }
    local_timestamp()
}

pub fn ledger_entry_time(created_at: u64, redeemed_at: Option<&str>) -> String {
    if let Some(raw) = redeemed_at.map(str::trim).filter(|value| !value.is_empty()) {
        return redeem_history_time(Some(raw));
    }
    timestamp_from_unix(created_at)
}

fn ledger_entry_code_label(entry: &CloudCreditLedgerEntryDto, card_key_id: Option<&str>) -> String {
    if let Some(id) = card_key_id.filter(|value| !value.is_empty()) {
        return id.to_string();
    }
    entry
        .reference_id
        .strip_prefix("card_key:")
        .filter(|value| !value.is_empty())
        .unwrap_or(entry.reference_id.as_str())
        .to_string()
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn group_digits(digits: &str) -> String {
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

fn local_timestamp() -> String {
    let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return "—".to_string();
    };
    timestamp_from_unix(duration.as_secs())
}

fn timestamp_from_unix(secs: u64) -> String {
    let secs = secs as i64;
    let days = secs / 86_400;
    let hour = (secs / 3600) % 24;
    let minute = (secs / 60) % 60;
    let second = secs % 60;
    let year = 1970 + days / 365;
    let month = ((days % 365) / 30).clamp(1, 12);
    let day = ((days % 365) % 30).clamp(1, 28);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
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
    use wormhole_desktop_core::cloud_credits::CloudCreditLedgerEntryDto;

    #[test]
    fn roundtrip_last_tab() {
        let dir =
            std::env::temp_dir().join(format!("wormhole-desktop-ui-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let prefs = DesktopUiPrefs {
            onboarding_dismissed: true,
            last_tab: Some("warp".into()),
            ..Default::default()
        };
        save(&dir, &prefs).expect("save");
        assert_eq!(load(&dir), prefs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_account_balance_groups_digits() {
        assert_eq!(format_account_balance(37_550_000_000), "¥37,550.00");
        assert_eq!(format_account_balance(100_000_000_000), "¥100,000.00");
        assert_eq!(format_account_balance(0), "¥0.00");
        assert_eq!(format_account_balance(-5), "¥0.00");
        assert_eq!(format_account_balance(123_456), "¥0.123456");
        assert_eq!(format_account_balance(1_230_000), "¥1.23");
    }

    #[test]
    fn format_balance_display_prefers_server_yuan() {
        assert_eq!(format_balance_display(Some("12.345678"), 0), "¥12.345678");
        assert_eq!(format_balance_display(None, 5_000_000), "¥5.00");
    }

    #[test]
    fn format_redeem_amount_uses_yuan_credits() {
        assert_eq!(format_redeem_amount(5_000_000), "+¥5.00");
        assert_eq!(format_redeem_amount(5_000_000_000), "+¥5,000.00");
        assert_eq!(format_redeem_amount(0), "+¥0.00");
    }

    #[test]
    fn redeem_history_from_ledger_filters_and_maps() {
        let entries = vec![
            CloudCreditLedgerEntryDto {
                entry_id: "e1".into(),
                delta_credits: 5,
                balance_after: 5,
                source: "card_key_redeem".into(),
                reference_id: "card_key:abc123".into(),
                metadata: serde_json::json!({
                    "cardKeyId": "abc123",
                    "redeemedAt": "2026-07-02T18:24:06Z"
                }),
                created_at: 1_780_000_000,
            },
            CloudCreditLedgerEntryDto {
                entry_id: "e2".into(),
                delta_credits: -1,
                balance_after: 4,
                source: "agent_platform_usage".into(),
                reference_id: "usage:1".into(),
                metadata: serde_json::json!({}),
                created_at: 1_780_000_100,
            },
        ];
        let mut overrides = HashMap::new();
        overrides.insert("abc123".into(), "WORMHOLE-TEST".into());
        let history = redeem_history_from_ledger(&entries, &overrides);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].code, "WORMHOLE-TEST");
        assert_eq!(history[0].time, "2026-07-02 18:24:06");
        assert_eq!(history[0].amount_credits, 5);
    }

    #[test]
    fn redeem_history_from_ledger_falls_back_to_reference_id() {
        let entries = vec![CloudCreditLedgerEntryDto {
            entry_id: "e1".into(),
            delta_credits: 10,
            balance_after: 10,
            source: "card_key_redeem".into(),
            reference_id: "card_key:WH-2024".into(),
            metadata: serde_json::json!({}),
            created_at: 1_780_000_000,
        }];
        let history = redeem_history_from_ledger(&entries, &HashMap::new());
        assert_eq!(history[0].code, "WH-2024");
    }

    #[test]
    fn redeem_history_time_normalizes_iso() {
        assert_eq!(
            redeem_history_time(Some("2026-07-02T18:24:06Z")),
            "2026-07-02 18:24:06"
        );
    }
}
