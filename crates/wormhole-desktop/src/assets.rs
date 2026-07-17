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
    bundled_icon!("device-remove.svg"),
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
    bundled_icon!("cluster-refresh.svg"),
    bundled_icon!("agent-more.svg"),
    bundled_icon!("agent-menu-archive.svg"),
    bundled_icon!("agent-menu-organize.svg"),
    bundled_icon!("agent-menu-sort.svg"),
    bundled_icon!("agent-menu-created.svg"),
    bundled_icon!("agent-menu-updated.svg"),
    bundled_icon!("agent-new.svg"),
    bundled_icon!("agent-new-project.svg"),
    bundled_icon!("agent-edit.svg"),
    bundled_icon!("agent-plus.svg"),
    bundled_icon!("agent-send.svg"),
    bundled_icon!("agent-search.svg"),
    bundled_icon!("agent-folder.svg"),
    bundled_icon!("agent-attach.svg"),
    bundled_icon!("agent-goal.svg"),
    bundled_icon!("agent-plan.svg"),
    bundled_icon!("agent-warn.svg"),
    bundled_icon!("agent-chevron.svg"),
    bundled_icon!("agent-chevron-down.svg"),
    bundled_icon!("agent-user.svg"),
    bundled_icon!("agent-copy.svg"),
    bundled_icon!("chat-compose-attach.svg"),
    bundled_icon!("chat-compose-emoji.svg"),
    bundled_icon!("chat-compose-send.svg"),
    bundled_icon!("chat-compose-mic.svg"),
    bundled_icon!("chat-attach-media.svg"),
    bundled_icon!("chat-attach-document.svg"),
    bundled_icon!("chat-attach-location.svg"),
    bundled_icon!("chat-header-search.svg"),
    bundled_icon!("chat-header-phone.svg"),
    bundled_icon!("chat-header-rdp.svg"),
    bundled_icon!("chat-header-profile.svg"),
    bundled_icon!("chat-header-more.svg"),
    bundled_icon!("chat-sidebar-search.svg"),
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
    fn bundled_device_action_icons_resolve() {
        let assets = WormholeAssets;
        for name in [
            "share-folder.svg",
            "chat-header-rdp.svg",
            "device-remove.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(bytes.starts_with(b"<svg"), "{name} should be svg markup");
            let markup = std::str::from_utf8(&bytes).unwrap_or_else(|_| panic!("{name}: utf-8"));
            assert!(
                !markup.contains("currentColor"),
                "{name} must use a white alpha mask"
            );
            assert!(markup.contains("white"), "{name} must contain a white mask");
        }
    }

    #[test]
    fn bundled_chat_icons_resolve() {
        let assets = WormholeAssets;
        for name in [
            "chat-compose-attach.svg",
            "chat-compose-emoji.svg",
            "chat-compose-send.svg",
            "chat-compose-mic.svg",
            "chat-attach-media.svg",
            "chat-attach-document.svg",
            "chat-attach-location.svg",
            "chat-header-search.svg",
            "chat-header-phone.svg",
            "chat-header-rdp.svg",
            "chat-header-profile.svg",
            "chat-header-more.svg",
            "chat-sidebar-search.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(bytes.starts_with(b"<svg"), "{name} should be svg markup");
        }
    }

    #[test]
    fn bundled_chat_icons_use_white_alpha_mask_not_current_color() {
        let assets = WormholeAssets;
        for name in [
            "chat-compose-attach.svg",
            "chat-compose-emoji.svg",
            "chat-compose-send.svg",
            "chat-compose-mic.svg",
            "chat-attach-media.svg",
            "chat-attach-document.svg",
            "chat-attach-location.svg",
            "chat-header-search.svg",
            "chat-header-phone.svg",
            "chat-header-rdp.svg",
            "chat-header-profile.svg",
            "chat-header-more.svg",
            "chat-sidebar-search.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            let markup = std::str::from_utf8(&bytes).unwrap_or_else(|_| panic!("{name}: utf-8"));
            assert!(
                !markup.contains("currentColor"),
                "{name} must not use currentColor (warpui Icon alpha mask needs white strokes/fills)"
            );
            assert!(
                markup.contains("white"),
                "{name} must use white strokes/fills for warpui Icon alpha mask"
            );
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
            "cluster-refresh.svg",
            "agent-search.svg",
            "agent-attach.svg",
            "agent-goal.svg",
            "agent-plan.svg",
            "agent-more.svg",
            "agent-menu-archive.svg",
            "agent-menu-organize.svg",
            "agent-menu-sort.svg",
            "agent-menu-created.svg",
            "agent-menu-updated.svg",
            "agent-new-project.svg",
            "agent-edit.svg",
            "agent-plus.svg",
            "agent-send.svg",
            "agent-folder.svg",
            "agent-chevron.svg",
            "agent-chevron-down.svg",
            "agent-warn.svg",
            "agent-user.svg",
            "agent-copy.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(bytes.starts_with(b"<svg"), "{name} should be svg markup");
        }
    }

    #[test]
    fn bundled_agent_icons_use_white_alpha_mask_not_current_color() {
        let assets = WormholeAssets;
        for name in [
            "agent-search.svg",
            "agent-more.svg",
            "agent-menu-archive.svg",
            "agent-menu-organize.svg",
            "agent-menu-sort.svg",
            "agent-menu-created.svg",
            "agent-menu-updated.svg",
            "agent-new-project.svg",
            "agent-edit.svg",
            "agent-plus.svg",
            "agent-send.svg",
            "agent-folder.svg",
            "agent-chevron.svg",
            "agent-chevron-down.svg",
            "agent-attach.svg",
            "agent-goal.svg",
            "agent-plan.svg",
            "agent-warn.svg",
            "agent-user.svg",
            "agent-copy.svg",
        ] {
            let bytes = assets.get(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            let markup = std::str::from_utf8(&bytes).unwrap_or_else(|_| panic!("{name}: utf-8"));
            assert!(
                !markup.contains("currentColor"),
                "{name} must not use currentColor (warpui Icon alpha mask needs white strokes/fills)"
            );
            assert!(
                markup.contains("white"),
                "{name} must use white strokes/fills for warpui Icon alpha mask"
            );
        }
    }
}
