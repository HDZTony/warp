// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the_windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig};
use warp_core::AppId;
use wormhole_embed::{
    apply_embed_launch_env, codex_profile, is_embedded, preferred_agent, DATA_DIR_ENV, PreferredAgent,
};

fn main() -> Result<()> {
    if let Ok(data_dir_raw) = std::env::var(DATA_DIR_ENV) {
        let data_dir = std::path::PathBuf::from(data_dir_raw.trim());
        let codex_home = data_dir.join("codex");
        let codex_api_key = std::env::var("WORMHOLE_AGENT_API_KEY")
            .or_else(|_| std::env::var("WORMHOLE_DEEPSEEK_API_KEY"))
            .unwrap_or_default();
        let cursor_api_key = std::env::var("CURSOR_API_KEY").unwrap_or_default();
        let cursor_model = std::env::var("WORMHOLE_CURSOR_MODEL").unwrap_or_default();
        let codex_bin = std::env::var("WORMHOLE_CODEX_BIN")
            .ok()
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_file());
        let preferred = std::env::var("WORMHOLE_PREFERRED_AGENT")
            .ok()
            .map(|value| PreferredAgent::parse(&value))
            .unwrap_or(PreferredAgent::Codex);
        apply_embed_launch_env(
            &data_dir,
            &codex_home,
            &codex_api_key,
            codex_bin.as_deref(),
            &cursor_api_key,
            &cursor_model,
            preferred,
        );
    }

    let mut state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("dev", "wormhole", "WarpWormhole"),
            logfile_name: "warp-oss-wormhole.log".into(),
            server_config: WarpServerConfig::production(),
            oz_config: OzConfig::production(),
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    );
    if cfg!(debug_assertions) {
        state = state.with_additional_features(warp_core::features::DEBUG_FLAGS);
    }
    ChannelState::set(state);

    if is_embedded() {
        log::info!(
            "warp-oss-wormhole embedded mode (agent={}, codex profile={})",
            preferred_agent().as_str(),
            codex_profile()
        );
    }

    warp::run()
}
