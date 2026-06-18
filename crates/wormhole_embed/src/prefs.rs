//! User preferences shared between Wormhole desktop and embedded Warp (`warp-embed.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const PREFS_FILE: &str = "warp-embed.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreferredAgent {
    #[default]
    Codex,
    Cursor,
}

impl PreferredAgent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "cursor" => Self::Cursor,
            _ => Self::Codex,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct WarpEmbedPrefs {
    #[serde(default)]
    pub preferred_agent: PreferredAgent,
}

pub fn prefs_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PREFS_FILE)
}

pub fn load_prefs(data_dir: &Path) -> WarpEmbedPrefs {
    let path = prefs_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => WarpEmbedPrefs::default(),
    }
}

pub fn save_prefs(data_dir: &Path, prefs: &WarpEmbedPrefs) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let bytes = serde_json::to_vec_pretty(prefs)?;
    std::fs::write(prefs_path(data_dir), bytes)
}

pub fn set_preferred_agent(data_dir: &Path, agent: PreferredAgent) -> std::io::Result<()> {
    let mut prefs = load_prefs(data_dir);
    prefs.preferred_agent = agent;
    save_prefs(data_dir, &prefs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_agent_parses_cursor() {
        assert_eq!(PreferredAgent::parse("cursor"), PreferredAgent::Cursor);
        assert_eq!(PreferredAgent::parse("codex"), PreferredAgent::Codex);
    }
}
