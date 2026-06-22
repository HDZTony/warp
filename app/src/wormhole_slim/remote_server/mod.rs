//! Remote-server stubs for `wormhole-slim` embed builds.
//!
//! Proto types and client helpers still come from the `remote_server` crate for editor
//! integration; only the live daemon/proxy manager is stubbed out.

pub mod manager;

#[cfg(not(target_family = "wasm"))]
#[path = "../../remote_server/codebase_index_model.rs"]
pub mod codebase_index_model;

#[path = "../../remote_server/diff_state_proto.rs"]
pub mod diff_state_proto;

pub use remote_server::{client, proto, HostId};

pub mod setup {
    pub use remote_server::setup::*;
}

#[cfg(unix)]
pub mod unix {
    pub fn launch_daemon(_identity_key: &str, _ctx: &mut warpui::AppContext) {}
}

pub fn wire_auth_token_rotation(_ctx: &mut warpui::AppContext) {}

#[cfg(unix)]
pub fn run_proxy(identity_key: String) -> anyhow::Result<()> {
    let _ = identity_key;
    anyhow::bail!("remote-server-proxy is disabled in wormhole-slim builds")
}

#[cfg(not(unix))]
pub fn run_proxy(_identity_key: String) -> anyhow::Result<()> {
    anyhow::bail!("remote-server-proxy is not supported on this platform")
}

#[cfg(unix)]
pub fn run_daemon(identity_key: String) -> anyhow::Result<()> {
    let _ = identity_key;
    anyhow::bail!("remote-server-daemon is disabled in wormhole-slim builds")
}

#[cfg(not(unix))]
pub fn run_daemon(_identity_key: String) -> anyhow::Result<()> {
    anyhow::bail!("remote-server-daemon is not supported on this platform")
}
