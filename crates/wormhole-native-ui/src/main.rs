mod agent_events_view;
mod agent_terminal_view;
#[cfg(any(windows, target_os = "macos"))]
mod computer_use_view;
mod coordinator;
mod cursor_agent_view;
mod daemon;
mod rdp_extras_ui;
mod rdp_host_control_view;
mod rdp_invoke;
mod rdp_view;
mod shell_bridge;
mod ui_text;
mod workspace_rdp_view;
mod workspace_session_hud_view;

mod assets;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;
use coordinator::{CoordinatorState, CoordinatorView};
use tracing_subscriber::EnvFilter;
use warpui::platform::{AppBuilder, AppCallbacks};
use wormhole_native_ipc::session_file_path;

#[derive(Debug, Parser)]
#[command(name = "wormhole-native-ui", about = "Wormhole native UI (WarpUI) sidecar")]
struct Args {
    /// Run the localhost IPC daemon (started by Wormhole desktop).
    #[arg(long)]
    daemon: bool,
    /// Wormhole data directory (`%LOCALAPPDATA%\\Wormhole` on Windows).
    #[arg(long, value_name = "DIR")]
    data_dir: PathBuf,
}

fn default_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("WORMHOLE_DATA_DIR") {
        let path = PathBuf::from(dir.trim());
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(dir).join("Wormhole")
    } else if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        PathBuf::from(home).join(".wormhole")
    } else {
        PathBuf::from("./wormhole-data")
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("wormhole_native_ui=info".parse()?))
        .init();

    let args = Args::parse();
    let data_dir = if args.data_dir.as_os_str().is_empty() {
        default_data_dir()
    } else {
        args.data_dir
    };
    std::fs::create_dir_all(&data_dir)?;

    if !args.daemon {
        anyhow::bail!("wormhole-native-ui requires --daemon (spawned by Wormhole desktop)");
    }

    let coordinator = Arc::new(Mutex::new(CoordinatorState::new(data_dir.clone())));
    let ipc_state = coordinator.clone();
    std::thread::spawn(move || {
        if let Err(err) = daemon::run(ipc_state) {
            tracing::error!("native UI IPC daemon exited: {err:#}");
        }
    });

    tracing::info!(
        data_dir = %data_dir.display(),
        session = %session_file_path(&data_dir).display(),
        "wormhole-native-ui daemon starting WarpUI"
    );

    let app_builder = AppBuilder::new(AppCallbacks::default(), Box::new(assets::EmptyAssets), None);
    let _ = app_builder.run(move |ctx| {
        ctx.add_window(
            warpui::AddWindowOptions {
                title: Some("Wormhole Native UI".to_string()),
                ..Default::default()
            },
            move |view_ctx| CoordinatorView::new(view_ctx, coordinator.clone()),
        );
    });
    Ok(())
}
