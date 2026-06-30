use warpui::elements::{
    Align, Container, CrossAxisAlignment, Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::panel_primitives::{
    section_hint, status_line, tab_content_fill, StatusTone, SECTION_PADDING,
};
use crate::ui::theme;
use wormhole_desktop_core::cluster_commands::{cluster_status_fast, ClusterStatusDto};
use wormhole_desktop_core::state::AppState;

#[derive(Clone, Debug, Default)]
pub struct DeviceGateStatus {
    pub loaded: bool,
    pub auth_required: bool,
    pub device_bootstrap_required: bool,
}

impl DeviceGateStatus {
    pub fn from_cluster(status: &ClusterStatusDto) -> Self {
        Self {
            loaded: true,
            auth_required: status.auth_required,
            device_bootstrap_required: status.device_bootstrap_required,
        }
    }

    pub fn blocking(&self) -> bool {
        self.loaded && (self.auth_required || self.device_bootstrap_required)
    }
}

pub async fn load_device_gate_status(state: &AppState) -> DeviceGateStatus {
    match cluster_status_fast(state).await {
        Ok(status) => DeviceGateStatus::from_cluster(&status),
        Err(_) => DeviceGateStatus {
            loaded: true,
            auth_required: true,
            device_bootstrap_required: false,
        },
    }
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
            format!("请先在顶栏点击「登录」，再使用{feature}。"),
            font,
            StatusTone::Placeholder,
        ));
    } else if gate.device_bootstrap_required {
        col.add_child(section_hint("DEVICE · 正在恢复设备身份", font));
        col.add_child(status_line(
            "登录成功，正在从云端恢复本机设备身份…",
            font,
            StatusTone::Placeholder,
        ));
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
