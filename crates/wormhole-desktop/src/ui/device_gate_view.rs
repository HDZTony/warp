use std::sync::Arc;
use std::time::Duration;

use warpui::elements::{Align, Container, CrossAxisAlignment, Flex, MainAxisSize, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{Element, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_hint, status_line, tab_content_fill, StatusTone, SECTION_PADDING,
};
use crate::ui::theme;
use wormhole_desktop_core::cluster_commands::{
    cluster_status, cluster_status_hud, schedule_active_cluster_member_update_if_ready,
    schedule_cluster_control_plane_reconcile, ClusterStatusDto,
};
use wormhole_desktop_core::device_identity::{ensure_device_ready, is_device_ready};
use wormhole_desktop_core::state::AppState;

pub const DEVICE_GATE_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Default)]
pub struct DeviceGateStatus {
    pub loaded: bool,
    pub auth_required: bool,
    pub device_bootstrap_required: bool,
    pub error: Option<String>,
}

impl DeviceGateStatus {
    pub fn from_cluster(status: &ClusterStatusDto) -> Self {
        Self {
            loaded: true,
            auth_required: status.auth_required,
            device_bootstrap_required: status.device_bootstrap_required,
            error: status.device_bootstrap_error.clone(),
        }
    }

    pub fn blocking(&self) -> bool {
        self.loaded && (self.auth_required || self.device_bootstrap_required)
    }

    /// Keep polling while login or device bootstrap is still pending (e.g. `auth.json`
    /// restore on startup races the first UI fetch).
    pub fn needs_poll(&self) -> bool {
        self.auth_required || self.device_bootstrap_required
    }
}

/// Start device bootstrap in the background without blocking UI status fetches.
pub async fn kick_device_bootstrap(state: &AppState) {
    if state.skip_device_gate || is_device_ready(state).await {
        return;
    }
    if state.cloud_auth.read().await.access_token.is_none() {
        return;
    }
    let bg = state.clone();
    tokio::spawn(async move {
        if let Err(err) = ensure_device_ready(&bg).await {
            tracing::warn!("background device bootstrap: {err}");
        }
    });
}

/// Cluster status for terminal / gate views: read local snapshot immediately.
///
/// After device bootstrap is ready this **must not** await synchronous control-plane
/// reconcile. Reconcile can block on vault/P2P init; doing it inline made the cluster
/// page「重试」appear dead and left the UI stuck on「正在恢复设备身份」even after
/// `device_gate.ready`. Membership reconcile + gossip join run in the background.
pub async fn fetch_cluster_for_ui(state: &AppState) -> Result<ClusterStatusDto, String> {
    kick_device_bootstrap(state).await;
    if !is_device_ready(state).await {
        return cluster_status(state).await;
    }
    schedule_active_cluster_member_update_if_ready(state.clone());
    schedule_cluster_control_plane_reconcile(state);
    cluster_status_hud(state).await
}

pub async fn fetch_device_gate_status(state: &AppState) -> DeviceGateStatus {
    kick_device_bootstrap(state).await;
    match cluster_status_hud(state).await {
        Ok(status) => DeviceGateStatus::from_cluster(&status),
        Err(_) => DeviceGateStatus {
            loaded: true,
            auth_required: true,
            device_bootstrap_required: false,
            error: None,
        },
    }
}

pub async fn load_device_gate_status(state: &AppState) -> DeviceGateStatus {
    fetch_device_gate_status(state).await
}

type GateApply<V> = Arc<dyn Fn(&mut V, DeviceGateStatus) + Send + Sync>;

pub fn load_device_gate<V>(core: CoreHandle, ctx: &mut ViewContext<V>, apply: GateApply<V>)
where
    V: View + 'static,
{
    let core_for_poll = core.clone();
    ctx.spawn(
        async move {
            let state = core.runtime().state.clone();
            fetch_device_gate_status(&state).await
        },
        move |view, gate, ctx| {
            apply(view, gate.clone());
            if gate.needs_poll() {
                schedule_device_gate_poll(core_for_poll, ctx, apply.clone());
            }
            ctx.notify();
        },
    );
}

pub fn schedule_device_gate_poll<V>(core: CoreHandle, ctx: &mut ViewContext<V>, apply: GateApply<V>)
where
    V: View + 'static,
{
    let core_for_poll = core.clone();
    ctx.spawn(
        async move {
            tokio::time::sleep(DEVICE_GATE_POLL_INTERVAL).await;
            let state = core.runtime().state.clone();
            fetch_device_gate_status(&state).await
        },
        move |view, gate, ctx| {
            apply(view, gate.clone());
            if gate.needs_poll() {
                schedule_device_gate_poll(core_for_poll, ctx, apply.clone());
            }
            ctx.notify();
        },
    );
}

pub fn device_gate_screen(
    font: FamilyId,
    feature: &'static str,
    gate: &DeviceGateStatus,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);

    if !gate.loaded {
        col.add_child(status_line("加载账号状态…", font, StatusTone::Placeholder));
    } else if gate.auth_required {
        col.add_child(section_hint("ACCOUNT · 需要登录", font));
        col.add_child(status_line(
            format!("请登录以使用{feature}。"),
            font,
            StatusTone::Placeholder,
        ));
    } else if gate.device_bootstrap_required {
        col.add_child(section_hint("DEVICE · 正在恢复设备身份", font));
        if let Some(err) = &gate.error {
            col.add_child(status_line(err.clone(), font, StatusTone::Danger));
        } else {
            col.add_child(status_line(
                "登录成功，正在从云端恢复本机设备身份…",
                font,
                StatusTone::Placeholder,
            ));
        }
    }

    tab_content_fill(
        Container::new(Align::new(col.finish()).finish())
            .with_uniform_padding(SECTION_PADDING)
            .with_background(theme::panel())
            .finish(),
    )
}

pub fn wrap_with_device_gate(
    font: FamilyId,
    feature: &'static str,
    gate: &DeviceGateStatus,
    content: Box<dyn Element>,
) -> Box<dyn Element> {
    if gate.blocking() || !gate.loaded {
        device_gate_screen(font, feature, gate)
    } else {
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_poll_while_login_or_bootstrap_pending() {
        let waiting_login = DeviceGateStatus {
            loaded: true,
            auth_required: true,
            device_bootstrap_required: false,
            error: None,
        };
        assert!(waiting_login.needs_poll());

        let waiting_bootstrap = DeviceGateStatus {
            loaded: true,
            auth_required: false,
            device_bootstrap_required: true,
            error: None,
        };
        assert!(waiting_bootstrap.needs_poll());

        let ready = DeviceGateStatus {
            loaded: true,
            auth_required: false,
            device_bootstrap_required: false,
            error: None,
        };
        assert!(!ready.needs_poll());
    }
}
