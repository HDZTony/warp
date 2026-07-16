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
use wormhole_desktop_core::MAIN_WINDOW_TITLE;
#[cfg(unix)]
use wormhole_desktop_core::{
    acquire_gui_instance_or_exit, acquire_headless_instance_or_exit, DesktopInstanceKind,
};

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
    /// Headless Linux sync daemon (FUSE + IPC); used by systemd user unit.
    #[arg(long)]
    headless: bool,
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

/// XDG desktop / Wayland `app_id` / X11 `WM_CLASS` — must match
/// `apps/desktop/linux/com.dongzhou.wormhole.desktop` and hicolor icon name.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const LINUX_APP_ID: &str = "com.dongzhou.wormhole";

/// Bundled Wormhole brand mark for Linux window / taskbar icon (128×128 RGBA PNG).
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const LINUX_APP_ICON_PNG: &[u8] =
    include_bytes!("../../../../../apps/desktop/bundle/icons/128x128.png");

/// Decode [`LINUX_APP_ICON_PNG`] to RGBA bytes for `winit::window::Icon::from_rgba`.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn decode_bundled_linux_app_icon() -> Result<(Vec<u8>, u32, u32)> {
    decode_png_rgba(LINUX_APP_ICON_PNG)
}

#[cfg(any(test, target_os = "linux", target_os = "freebsd"))]
fn decode_png_rgba(png: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(png)?.to_rgba8();
    let (width, height) = img.dimensions();
    Ok((img.into_raw(), width, height))
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

    if args.headless_rdp || wormhole_desktop_core::rdp_headless::is_headless_rdp_requested() {
        #[cfg(unix)]
        let _headless_rdp_lock =
            acquire_headless_instance_or_exit(DesktopInstanceKind::HeadlessRdp);
        #[cfg(windows)]
        if !wormhole_desktop_platform_windows::register_headless_rdp_single_instance() {
            tracing::info!("headless RDP companion already running; exiting duplicate instance");
            return Ok(());
        }
        wormhole_desktop_core::rdp_headless::run();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    if args.headless || wormhole_desktop_core::linux_headless::is_headless_requested() {
        let _headless_sync_lock =
            acquire_headless_instance_or_exit(DesktopInstanceKind::HeadlessSync);
        wormhole_desktop_core::linux_headless::run();
        return Ok(());
    }

    wormhole_desktop_core::rdp_headless::bootstrap_dev_env();
    std::fs::create_dir_all(&data_dir)?;

    #[cfg(windows)]
    let windows_deep_link = wormhole_desktop_platform_windows::DeepLinkState::default();
    #[cfg(windows)]
    {
        use wormhole_desktop_platform_windows::{
            desktop_process_entry, handle_startup_args, DesktopProcessRole,
        };
        let argv: Vec<String> = std::env::args().collect();
        if let Some(from_arg) = handle_startup_args(&argv, &windows_deep_link) {
            data_dir = from_arg;
        }
        match desktop_process_entry(&argv, &windows_deep_link, &data_dir) {
            DesktopProcessRole::SecondaryForwardedDeeplink
            | DesktopProcessRole::SecondaryDuplicate => return Ok(()),
            DesktopProcessRole::Primary => {}
        }
        let _ = windows_deep_link.take_pending_url();
        if let Err(err) =
            wormhole_desktop_core::deeplink_commands::sync_wormhole_protocol_registration(&data_dir)
        {
            tracing::warn!("无法同步 wormhole 协议注册: {err}");
        }
    }

    if wormhole_desktop_core::rdp_headless::spawn_headless_companion_requested() {
        match wormhole_desktop_core::rdp_headless::spawn_headless_companion(&data_dir) {
            Ok(()) => unsafe {
                std::env::set_var("WORMHOLE_SKIP_UNATTENDED_RDP", "1");
            },
            Err(err) => tracing::warn!("failed to spawn headless RDP companion: {err}"),
        }
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

    #[cfg(unix)]
    let _gui_instance_lock = acquire_gui_instance_or_exit();

    #[cfg(windows)]
    let tray = {
        use wormhole_desktop_platform_windows::TrayController;
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
        // WarpUI only tears down the winit window (and exits the event loop) when this
        // callback is registered. Wormhole does not use undo-close, but the hook is required.
        callbacks.on_window_will_close = Some(Box::new(|_closed_window, _ctx| {}));

        let core_for_terminate = core.clone();
        callbacks.on_will_terminate = Some(Box::new(move |_ctx| {
            let runtime = core_for_terminate.runtime();
            if let Err(err) =
                core_for_terminate.block_on(shutdown_desktop(&runtime.state, &runtime.ctx))
            {
                tracing::warn!("desktop shutdown on terminate: {err:#}");
            }
        }));

        callbacks
    };

    let mut app_builder = AppBuilder::new(callbacks, Box::new(assets::WormholeAssets), None);
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        use warpui::platform::linux::AppBuilderExt;
        app_builder.set_window_class(LINUX_APP_ID.into());
        match decode_bundled_linux_app_icon() {
            Ok((rgba, width, height)) => {
                if let Err(err) = app_builder.set_window_icon(rgba, width, height) {
                    tracing::warn!("linux window icon: {err}");
                }
            }
            Err(err) => tracing::warn!("linux window icon decode failed: {err:#}"),
        }
    }
    let coordinator_for_shell = coordinator.clone();
    let core_for_shell = core.clone();
    #[cfg(windows)]
    let tray_for_shell = tray.clone();

    #[cfg(unix)]
    {
        let core_for_signal = core.clone();
        std::thread::Builder::new()
            .name("wormhole-signal".into())
            .spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(err) => {
                        tracing::warn!("signal handler runtime failed: {err}");
                        return;
                    }
                };
                rt.block_on(async {
                    #[cfg(unix)]
                    {
                        use tokio::signal::unix::{signal, SignalKind};
                        let mut sigterm = signal(SignalKind::terminate()).ok();
                        let mut sigint = signal(SignalKind::interrupt()).ok();
                        tokio::select! {
                            _ = async {
                                if let Some(stream) = sigterm.as_mut() {
                                    stream.recv().await
                                } else {
                                    std::future::pending().await
                                }
                            } => tracing::info!("received SIGTERM; shutting down wormhole-desktop"),
                            _ = async {
                                if let Some(stream) = sigint.as_mut() {
                                    stream.recv().await
                                } else {
                                    std::future::pending().await
                                }
                            } => tracing::info!("received SIGINT; shutting down wormhole-desktop"),
                        }
                    }
                    let runtime = core_for_signal.runtime();
                    if let Err(err) = shutdown_desktop(&runtime.state, &runtime.ctx).await {
                        tracing::warn!("signal shutdown: {err:#}");
                    }
                    std::process::exit(0);
                });
            })?;
    }

    let _ = app_builder.run(move |ctx| {
        crate::ui::fonts::warm_up_font_cache(ctx);
        #[cfg(windows)]
        ctx.add_singleton_model(crate::ui::window_chrome::WindowsSymbolFontState::new);
        ctx.add_window(
            ui::window_options::desktop_window_options(MAIN_WINDOW_TITLE, vec2f(1280.0, 840.0)),
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

#[cfg(test)]
mod app_icon_tests {
    use super::decode_png_rgba;

    #[test]
    fn bundled_128_png_decodes_to_rgba() {
        let png = include_bytes!("../../../../../apps/desktop/bundle/icons/128x128.png");
        let (rgba, width, height) =
            decode_png_rgba(png).expect("128x128.png should decode");
        assert_eq!((width, height), (128, 128));
        assert_eq!(rgba.len(), 128 * 128 * 4);
    }

    #[test]
    fn application_desktop_file_has_wormhole_icon_and_wmclass() {
        let desktop =
            include_str!("../../../../../apps/desktop/linux/com.dongzhou.wormhole.desktop");
        assert!(
            desktop.contains("Icon=com.dongzhou.wormhole"),
            "desktop Icon= must be com.dongzhou.wormhole"
        );
        assert!(
            desktop.contains("StartupWMClass=com.dongzhou.wormhole"),
            "desktop StartupWMClass must match set_window_class"
        );
    }

    #[test]
    fn autostart_desktop_file_has_wormhole_icon_and_wmclass() {
        let desktop = include_str!(
            "../../../../../apps/desktop/linux/autostart/com.dongzhou.wormhole.desktop"
        );
        assert!(desktop.contains("Icon=com.dongzhou.wormhole"));
        assert!(desktop.contains("StartupWMClass=com.dongzhou.wormhole"));
    }
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
