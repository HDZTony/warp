//! Shared desktop auto-update session (About page + shell banner).
//!
//! Phase machine aligns with OpenHuman `useAppUpdate`:
//! `idle | checking | available | downloading | ready_to_install | installing | error`.
//! The banner shows for downloading / ready / installing / error (with live progress
//! while bytes stream).

use std::sync::{Arc, Mutex};

use wormhole_desktop_core::{
    DesktopUpdateDownloadDto, DesktopUpdateProgressDto, DesktopUpdateStatusDto,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopUpdatePhase {
    Idle,
    Checking,
    Available,
    Downloading,
    ReadyToInstall,
    Installing,
    Error,
}

#[derive(Debug, Clone)]
pub struct DesktopUpdateShared {
    pub phase: DesktopUpdatePhase,
    pub status: Option<DesktopUpdateStatusDto>,
    pub download: Option<DesktopUpdateDownloadDto>,
    pub progress: Option<DesktopUpdateProgressDto>,
    pub error: Option<String>,
    /// Session-only dismiss for the shell banner (About can still install).
    pub banner_dismissed: bool,
    /// Prevents overlapping check/download jobs.
    pub busy: bool,
}

impl Default for DesktopUpdateShared {
    fn default() -> Self {
        Self {
            phase: DesktopUpdatePhase::Idle,
            status: None,
            download: None,
            progress: None,
            error: None,
            banner_dismissed: false,
            busy: false,
        }
    }
}

impl DesktopUpdateShared {
    pub fn banner_visible(&self) -> bool {
        if self.banner_dismissed {
            return false;
        }
        matches!(
            self.phase,
            DesktopUpdatePhase::Downloading
                | DesktopUpdatePhase::ReadyToInstall
                | DesktopUpdatePhase::Installing
                | DesktopUpdatePhase::Error
        )
    }

    pub fn ready_version(&self) -> Option<String> {
        self.download
            .as_ref()
            .and_then(|d| d.version.clone())
            .or_else(|| {
                self.status
                    .as_ref()
                    .and_then(|s| s.latest_version.clone())
            })
    }
}

pub type DesktopUpdateHandle = Arc<Mutex<DesktopUpdateShared>>;

pub fn new_handle() -> DesktopUpdateHandle {
    Arc::new(Mutex::new(DesktopUpdateShared::default()))
}

/// Human-readable byte size for update progress labels.
pub fn format_update_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Detail line for an in-flight download (`12.3 MB / 338.9 MB (4%)` or indeterminate).
pub fn download_progress_detail(progress: Option<&DesktopUpdateProgressDto>) -> String {
    let Some(p) = progress else {
        return wormhole_i18n::t("shell.update.downloading_detail");
    };
    let downloaded = format_update_bytes(p.bytes_downloaded);
    if let (Some(total), Some(fraction)) = (p.bytes_total, p.fraction) {
        let pct = (fraction * 100.0).clamp(0.0, 100.0).round() as u32;
        let total_s = format_update_bytes(total);
        let pct_s = pct.to_string();
        return wormhole_i18n::t_args(
            "shell.update.downloading_progress",
            &[
                ("downloaded", downloaded.as_str()),
                ("total", total_s.as_str()),
                ("percent", pct_s.as_str()),
            ],
        );
    }
    wormhole_i18n::t_args(
        "shell.update.downloading_bytes",
        &[("downloaded", downloaded.as_str())],
    )
}
