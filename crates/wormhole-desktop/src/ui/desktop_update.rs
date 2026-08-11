//! Shared desktop auto-update session (About page + shell banner).
//!
//! Phase machine aligns with OpenHuman `useAppUpdate`:
//! `idle | checking | available | downloading | ready_to_install | installing | error`.
//! The banner shows only for ready / installing / error (download stays silent).

use std::sync::{Arc, Mutex};

use wormhole_desktop_core::{DesktopUpdateDownloadDto, DesktopUpdateStatusDto};

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
            DesktopUpdatePhase::ReadyToInstall
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
