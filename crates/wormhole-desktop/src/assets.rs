use std::borrow::Cow;

use anyhow::{anyhow, Result};
use warpui::AssetProvider;

macro_rules! bundled_icon {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../assets/svg/", $name)) as &[u8],
        )
    };
}

static BUNDLED_ASSETS: &[(&str, &[u8])] = &[
    bundled_icon!("tab-devices.svg"),
    bundled_icon!("tab-chat.svg"),
    bundled_icon!("tab-agent.svg"),
    bundled_icon!("tab-toolbox.svg"),
    bundled_icon!("tab-settings.svg"),
    bundled_icon!("device-pc.svg"),
    bundled_icon!("device-phone.svg"),
    bundled_icon!("device-tablet.svg"),
    bundled_icon!("share-folder.svg"),
    bundled_icon!("share-file.svg"),
    bundled_icon!("share-pdf.svg"),
    bundled_icon!("share-json.svg"),
    bundled_icon!("share-md.svg"),
    bundled_icon!("share-toml.svg"),
    bundled_icon!("share-rs.svg"),
    bundled_icon!("share-swift.svg"),
    bundled_icon!("share-hevc.svg"),
    bundled_icon!("share-sync.svg"),
    bundled_icon!("share-nav-back.svg"),
    bundled_icon!("share-nav-forward.svg"),
    bundled_icon!("agent-more.svg"),
    bundled_icon!("agent-new.svg"),
    bundled_icon!("agent-search.svg"),
    bundled_icon!("agent-folder.svg"),
    bundled_icon!("agent-attach.svg"),
    bundled_icon!("agent-warn.svg"),
    bundled_icon!("agent-chevron.svg"),
    bundled_icon!("agent-user.svg"),
    bundled_icon!("agent-copy.svg"),
    bundled_icon!("chat-compose-attach.svg"),
    bundled_icon!("chat-compose-emoji.svg"),
    bundled_icon!("chat-compose-send.svg"),
    bundled_icon!("chat-compose-mic.svg"),
];

pub struct WormholeAssets;

impl AssetProvider for WormholeAssets {
    fn get(&self, path: &str) -> Result<Cow<'_, [u8]>> {
        BUNDLED_ASSETS
            .iter()
            .find(|(asset_path, _)| *asset_path == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes))
            .ok_or_else(|| anyhow!("no bundled asset at {path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_tab_icons_resolve() {
        let assets = WormholeAssets;
        for name in [
            "tab-devices.svg",
            "tab-chat.svg",
            "tab-agent.svg",
            "tab-toolbox.svg",
            "tab-settings.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(bytes.starts_with(b"<svg"), "{name} should be svg markup");
        }
    }

    #[test]
    fn bundled_share_and_agent_icons_resolve() {
        let assets = WormholeAssets;
        for name in [
            "share-pdf.svg",
            "share-json.svg",
            "share-nav-back.svg",
            "share-nav-forward.svg",
            "agent-search.svg",
            "agent-attach.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(bytes.starts_with(b"<svg"), "{name} should be svg markup");
        }
    }
}
