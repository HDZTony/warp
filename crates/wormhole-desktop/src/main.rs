mod agent_events_view;
#[cfg(any(windows, target_os = "macos"))]
mod computer_use_view;
mod coordinator;
mod rdp_extras_ui;
mod rdp_host_control_view;
mod rdp_invoke;
mod rdp_view;
mod shell_bridge;
mod ui;
mod ui_text;
mod workspace_rdp_view;
mod workspace_session_hud_view;
mod wormhole_native_ipc;

mod assets;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;
use coordinator::{CoordinatorState, CoordinatorView};
use pathfinder_geometry::vector::vec2f;
use tracing_subscriber::EnvFilter;
use ui::app_shell::AppShellView;
use ui::codex_provider_import_model::new_shared_import_model;
use ui::core_handle::CoreHandle;
use warpui::platform::{AppBuilder, AppCallbacks, WindowBounds};
use wormhole_desktop_core::bootstrap_desktop;

#[derive(Debug, Parser)]
#[command(name = "wormhole-desktop", about = "Wormhole desktop (Warp native UI)")]
struct Args {
    /// Wormhole data directory (`%LOCALAPPDATA%\\Wormhole` on Windows).
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    bridge_only: bool,
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

fn bundled_resource_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos_dir = exe.parent()?;
    if macos_dir.file_name().and_then(|name| name.to_str()) != Some("MacOS") {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name().and_then(|name| name.to_str()) != Some("Contents") {
        return None;
    }
    Some(contents_dir.join("Resources"))
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("wormhole_desktop=info".parse()?),
        )
        .init();

    let args = Args::parse();
    let mut data_dir = args.data_dir.unwrap_or_else(default_data_dir);

    if wormhole_desktop_core::rdp_headless::is_headless_rdp_requested() {
        wormhole_desktop_core::rdp_headless::run();
        return Ok(());
    }

    if args.bridge_only {
        std::fs::create_dir_all(&data_dir)?;
        let tokio = tokio::runtime::Runtime::new()?;
        let _desktop_runtime = tokio.block_on(bootstrap_desktop(
            Some(data_dir.clone()),
            bundled_resource_dir(),
        ))?;
        tracing::info!(
            data_dir = %data_dir.display(),
            "wormhole-desktop command bridge running without WarpUI shell"
        );
        loop {
            std::thread::park();
        }
    }

    #[cfg(windows)]
    let mut pending_deeplink: Option<String> = None;
    #[cfg(windows)]
    {
        use wormhole_desktop_platform_windows::{
            desktop_process_entry, handle_startup_args, DeepLinkState, DesktopProcessRole,
            TrayController,
        };
        let deep_link = DeepLinkState::default();
        let argv: Vec<String> = std::env::args().collect();
        if let Some(from_arg) = handle_startup_args(&argv, &deep_link) {
            data_dir = from_arg;
        }
        match desktop_process_entry(&argv, &deep_link, &data_dir) {
            DesktopProcessRole::SecondaryForwardedDeeplink
            | DesktopProcessRole::SecondaryDuplicate => return Ok(()),
            DesktopProcessRole::Primary => {}
        }
        pending_deeplink = deep_link.take_pending_url();
        if let Some(ref url) = pending_deeplink {
            wormhole_desktop_core::deeplink_commands::on_deeplink_received(&data_dir, url);
        }
        let _tray = TrayController::spawn("Wormhole")?;
    }
    #[cfg(not(windows))]
    let pending_deeplink: Option<String> = None;

    std::fs::create_dir_all(&data_dir)?;

    let tokio = tokio::runtime::Runtime::new()?;
    let desktop_runtime = tokio.block_on(bootstrap_desktop(
        Some(data_dir.clone()),
        bundled_resource_dir(),
    ))?;
    let core = CoreHandle::new(desktop_runtime, tokio);

    let coordinator = Arc::new(Mutex::new(CoordinatorState::new(data_dir.clone())));

    tracing::info!(
        data_dir = %data_dir.display(),
        "wormhole-desktop starting WarpUI shell"
    );

    let app_builder = AppBuilder::new(AppCallbacks::default(), Box::new(assets::EmptyAssets), None);
    let import_model = new_shared_import_model();
    let coordinator_for_shell = coordinator.clone();
    let core_for_shell = core.clone();
    let import_model_for_shell = import_model.clone();
    let pending_for_shell = pending_deeplink;
    let _ = app_builder.run(move |ctx| {
        ctx.add_window(
            warpui::AddWindowOptions {
                title: Some("Wormhole".to_string()),
                window_bounds: WindowBounds::ExactSize(vec2f(1280.0, 840.0)),
                ..Default::default()
            },
            move |view_ctx| {
                AppShellView::new(
                    view_ctx,
                    core_for_shell,
                    coordinator_for_shell,
                    import_model_for_shell,
                    pending_for_shell,
                )
            },
        );
    });
    Ok(())
}
