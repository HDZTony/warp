#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent_events_view;
#[cfg(any(windows, target_os = "macos"))]
mod computer_use_view;
mod coordinator;
mod daemon;
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
use ui::core_handle::CoreHandle;
use warpui::platform::{AppBuilder, AppCallbacks};
use warpui_core::platform::app::ApproveTerminateResult;
use wormhole_desktop_core::bootstrap_desktop;
use wormhole_desktop_core::shutdown_desktop;

#[derive(Debug, Parser)]
#[command(name = "wormhole-desktop", about = "Wormhole desktop (Warp native UI)")]
struct Args {
    /// Wormhole data directory (`%LOCALAPPDATA%\\Wormhole` on Windows).
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    bridge_only: bool,
    #[arg(long)]
    headless_rdp: bool,
    #[cfg(windows)]
    #[arg(long)]
    mount_w_drive: bool,
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
    let args = Args::parse();
    let mut data_dir = args.data_dir.clone().unwrap_or_else(default_data_dir);
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::new("wormhole_desktop=info,noq_proto=error")
    };
    wormhole_desktop_core::init_desktop_tracing(&data_dir, "wormhole-desktop", env_filter);
    wormhole_desktop_core::install_observability_for_process(&data_dir, "wormhole-desktop");

    #[cfg(windows)]
    if args.mount_w_drive {
        std::fs::create_dir_all(&data_dir)?;
        wormhole_desktop_core::w_drive_headless::run_for_data_dir(data_dir);
        return Ok(());
    }

    if args.headless_rdp || wormhole_desktop_core::rdp_headless::is_headless_rdp_requested() {
        wormhole_desktop_core::rdp_headless::run();
        return Ok(());
    }

    wormhole_desktop_core::rdp_headless::bootstrap_dev_env();
    std::fs::create_dir_all(&data_dir)?;
    if wormhole_desktop_core::rdp_headless::spawn_headless_companion_requested() {
        match wormhole_desktop_core::rdp_headless::spawn_headless_companion(&data_dir) {
            Ok(()) => unsafe {
                std::env::set_var("WORMHOLE_SKIP_UNATTENDED_RDP", "1");
            },
            Err(err) => tracing::warn!("failed to spawn headless RDP companion: {err}"),
        }
    }

    #[cfg(windows)]
    if wormhole_desktop_core::w_drive_headless::is_mount_w_drive_requested() {
        wormhole_desktop_core::w_drive_headless::run_mount();
        return Ok(());
    }

    #[cfg(windows)]
    if wormhole_desktop_core::w_drive_headless::is_init_w_drive_requested() {
        wormhole_desktop_core::w_drive_headless::run_init();
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
    let tray = {
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
        // Consume any pending wormhole:// URL so it does not linger; provider import is removed.
        let _ = deep_link.take_pending_url();
        if let Err(err) =
            wormhole_desktop_core::deeplink_commands::sync_wormhole_protocol_registration(&data_dir)
        {
            tracing::warn!("无法同步 wormhole 协议注册: {err}");
        }
        Arc::new(TrayController::spawn("Wormhole")?)
    };
    std::fs::create_dir_all(&data_dir)?;

    let tokio = tokio::runtime::Runtime::new()?;
    let desktop_runtime = tokio.block_on(bootstrap_desktop(
        Some(data_dir.clone()),
        bundled_resource_dir(),
    ))?;
    let core = CoreHandle::new(desktop_runtime, tokio);

    let coordinator = Arc::new(Mutex::new(CoordinatorState::new(data_dir.clone())));
    {
        let coordinator = coordinator.clone();
        std::thread::Builder::new()
            .name("wormhole-native-ipc".into())
            .spawn(move || {
                if let Err(err) = daemon::run(coordinator) {
                    tracing::error!("native UI IPC failed: {err:#}");
                }
            })?;
    }

    tracing::info!(
        data_dir = %data_dir.display(),
        "wormhole-desktop starting WarpUI shell"
    );

    let callbacks = {
        let mut callbacks = AppCallbacks::default();
        #[cfg(windows)]
        {
            let coordinator_for_close = coordinator.clone();
            callbacks.on_should_close_window = Some(Box::new(move |window_id, ctx| {
                if ui::windows_shell::should_hide_main_window_to_tray(
                    window_id,
                    &coordinator_for_close,
                ) {
                    ui::windows_shell::hide_main_window(window_id, ctx);
                    ApproveTerminateResult::Cancel
                } else {
                    ApproveTerminateResult::Terminate
                }
            }));
        }
        callbacks
    };

    let app_builder = AppBuilder::new(callbacks, Box::new(assets::WormholeAssets), None);
    let coordinator_for_shell = coordinator.clone();
    let core_for_shell = core.clone();
    #[cfg(windows)]
    let tray_for_shell = tray.clone();
    let _ = app_builder.run(move |ctx| {
        crate::ui::fonts::warm_up_font_cache(ctx);
        #[cfg(windows)]
        ctx.add_singleton_model(crate::ui::window_chrome::WindowsSymbolFontState::new);
        ctx.add_window(
            ui::window_options::desktop_window_options("Wormhole", vec2f(1280.0, 840.0)),
            move |view_ctx| {
                AppShellView::new(
                    view_ctx,
                    core_for_shell,
                    coordinator_for_shell,
                    #[cfg(windows)]
                    tray_for_shell,
                )
            },
        );
    });
    let runtime = core.runtime();
    if let Err(err) = core.block_on(shutdown_desktop(&runtime.state, &runtime.ctx)) {
        tracing::warn!("desktop shutdown: {err:#}");
    }
    Ok(())
}

#[cfg(all(test, windows))]
mod windows_icon_tests {
    //! WarpUI taskbar icon is loaded as PE resource `0x101` (see `build.rs`).
    use windows_sys::Win32::Foundation::HINSTANCE;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE,
    };

    /// Same ID as `warpui::windowing::winit::window::IDI_ICON` / Warp `app/build.rs`.
    const IDI_ICON: usize = 0x101;

    #[test]
    fn pe_embeds_warpui_taskbar_icon_resource_0x101() {
        let module: HINSTANCE = unsafe { GetModuleHandleW(std::ptr::null()) };
        assert!(!module.is_null(), "GetModuleHandleW(null) failed");

        let icon = unsafe {
            LoadImageW(
                module,
                IDI_ICON as *const u16,
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE,
            )
        };
        assert!(
            !icon.is_null(),
            "missing RT_GROUP_ICON id 0x101 — wormhole-desktop build.rs must embed icon.ico with set_icon_with_id(..., \"257\") to match WarpUI"
        );
        unsafe {
            DestroyIcon(icon);
        }
    }
}
