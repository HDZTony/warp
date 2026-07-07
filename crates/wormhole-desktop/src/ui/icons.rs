//! SVG icons from `docs/design/desktop-current.html` (bundled under `assets/svg/`).

use pathfinder_color::ColorU;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Expanded, Flex, Icon,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, Stack,
};
use warpui::Element;

use crate::ui::app_shell::AppTab;
use crate::ui::cluster_layout::CARD_MIN_WIDTH;
use crate::ui::hud_effects::DeviceEnergyLines;
use crate::ui::spinning_icon;
use crate::ui::theme;

pub const TAB_ICON_SIZE: f32 = 18.0;
pub const SHARE_ICON_SIZE: f32 = 18.0;
pub const SHARE_NAV_BTN_SIZE: f32 = 28.0;
pub const SHARE_NAV_ICON_SIZE: f32 = 14.0;
pub const AGENT_ICON_SIZE: f32 = 15.0;
pub const AGENT_COMPOSER_ICON_SIZE: f32 = 16.0;
pub const CHAT_HEADER_ICON_SIZE: f32 = 18.0;
pub const CHAT_COMPOSE_ICON_SIZE: f32 = 20.0;
pub const CHAT_COMPOSE_BTN: f32 = 40.0;
pub const DEVICE_ICON_WIDTH: f32 = 56.0;
pub const DEVICE_ICON_HEIGHT: f32 = 44.0;
pub const DEVICE_THUMB_HEIGHT: f32 = 132.0;
pub const CLUSTER_REFRESH_BTN_SIZE: f32 = 32.0;
pub const CLUSTER_REFRESH_ICON_SIZE: f32 = 16.0;
pub const CLUSTER_REFRESH_ICON_PATH: &str = "cluster-refresh.svg";

pub fn icon(path: &'static str, size: f32, color: ColorU) -> Box<dyn Element> {
    ConstrainedBox::new(Icon::new(path, color).finish())
        .with_width(size)
        .with_height(size)
        .finish()
}

pub fn tab_icon(tab: AppTab, color: ColorU) -> Box<dyn Element> {
    let path = match tab {
        AppTab::Devices | AppTab::WDrive | AppTab::Sync | AppTab::Display => "tab-devices.svg",
        AppTab::Chat => "tab-chat.svg",
        AppTab::Warp => "tab-agent.svg",
        AppTab::Toolbox => "tab-toolbox.svg",
        AppTab::Settings => "tab-settings.svg",
    };
    icon(path, TAB_ICON_SIZE, color)
}

pub fn tab_button_content(
    tab: AppTab,
    expand_label: bool,
    color: ColorU,
    label: &str,
    font: warpui::fonts::FamilyId,
) -> Box<dyn Element> {
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Container::new(tab_icon(tab, color))
                .with_horizontal_margin(2.0)
                .finish(),
        );
    if expand_label {
        row.add_child(
            Container::new(
                crate::ui_text::hud_title(label.to_string(), font)
                    .with_color(color)
                    .finish(),
            )
            .with_horizontal_margin(6.0)
            .finish(),
        );
    }
    row.finish()
}

pub fn agent_icon(path: &'static str, color: ColorU) -> Box<dyn Element> {
    icon(path, AGENT_ICON_SIZE, color)
}

pub fn agent_composer_icon(path: &'static str, color: ColorU) -> Box<dyn Element> {
    icon(path, AGENT_COMPOSER_ICON_SIZE, color)
}

pub fn chat_header_icon(path: &'static str, color: ColorU) -> Box<dyn Element> {
    icon(path, CHAT_HEADER_ICON_SIZE, color)
}

pub fn chat_compose_icon(path: &'static str, color: ColorU) -> Box<dyn Element> {
    icon(path, CHAT_COMPOSE_ICON_SIZE, color)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceIconKind {
    Pc,
    Phone,
    Tablet,
}

pub fn device_icon_kind(os_label: &str) -> DeviceIconKind {
    let os = os_label.to_ascii_lowercase();
    if os.contains("ipad") {
        DeviceIconKind::Tablet
    } else if os.contains("ios") || os.contains("android") || os.contains("phone") {
        DeviceIconKind::Phone
    } else {
        DeviceIconKind::Pc
    }
}

pub fn device_icon(kind: DeviceIconKind, color: ColorU) -> Box<dyn Element> {
    device_icon_scaled(kind, color, CARD_MIN_WIDTH)
}

pub fn device_icon_scaled(
    kind: DeviceIconKind,
    color: ColorU,
    card_width: f32,
) -> Box<dyn Element> {
    let thumb_height = card_width * 0.75;
    let inner_w = (card_width - 24.0).max(1.0);
    let inner_h = (thumb_height - 24.0 - 40.0).max(1.0);
    let (path, base_w, base_h, max_w_frac, max_h_frac) = match kind {
        DeviceIconKind::Pc => ("device-pc.svg", 48.0_f32, 36.0_f32, 0.52, 0.52),
        DeviceIconKind::Phone => ("device-phone.svg", 28.0_f32, 44.0_f32, 0.30, 0.44),
        DeviceIconKind::Tablet => ("device-tablet.svg", 44.0_f32, 32.0_f32, 0.58, 0.46),
    };
    let max_w = inner_w * max_w_frac;
    let max_h = inner_h * max_h_frac;
    let scale = (max_w / base_w).min(max_h / base_h).max(1.0);
    let width = base_w * scale;
    let height = base_h * scale;
    ConstrainedBox::new(Icon::new(path, color).finish())
        .with_width(width)
        .with_height(height)
        .finish()
}

pub fn device_thumb(
    os_label: &str,
    online: bool,
    color: ColorU,
    mono: warpui::fonts::FamilyId,
    card_width: f32,
) -> Box<dyn Element> {
    let kind = device_icon_kind(os_label);
    let os_upper = os_label.to_ascii_uppercase();
    let thumb_height = card_width * 0.75;
    let mut stack = Stack::new();
    if online {
        stack.add_child(DeviceEnergyLines::live());
    }
    stack.add_child(
        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Expanded::new(
                        1.0,
                        Flex::column()
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_main_axis_alignment(MainAxisAlignment::Center)
                            .with_child(device_icon_scaled(kind, color, card_width))
                            .finish(),
                    )
                    .finish(),
                )
                .with_child(
                    Container::new(
                        crate::ui_text::device_os_label(os_upper, mono)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .with_uniform_padding(4.0)
                    .with_horizontal_margin(8.0)
                    .with_vertical_margin(8.0)
                    .with_background(theme::panel())
                    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                        crate::ui::panel_primitives::HUD_RADIUS,
                    )))
                    .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(12.0)
        .finish(),
    );
    Container::new(
        ConstrainedBox::new(stack.finish())
            .with_height(thumb_height)
            .finish(),
    )
    .with_background(theme::accent_cool_bg(16))
    .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
    .finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareIconKind {
    Folder,
    Txt,
    Md,
    Json,
    Pdf,
    Toml,
    Rs,
    Swift,
    Hevc,
    File,
}

pub fn share_icon_kind(name: &str, is_dir: bool) -> ShareIconKind {
    if is_dir || name.ends_with('/') || name.ends_with('\\') {
        return ShareIconKind::Folder;
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "txt" => ShareIconKind::Txt,
        "md" => ShareIconKind::Md,
        "json" => ShareIconKind::Json,
        "pdf" => ShareIconKind::Pdf,
        "toml" => ShareIconKind::Toml,
        "rs" => ShareIconKind::Rs,
        "swift" => ShareIconKind::Swift,
        "hevc" | "mp4" | "mov" => ShareIconKind::Hevc,
        _ => ShareIconKind::File,
    }
}

impl ShareIconKind {
    pub fn asset_path(self) -> &'static str {
        match self {
            ShareIconKind::Folder => "share-folder.svg",
            ShareIconKind::Txt => "share-file.svg",
            ShareIconKind::Md => "share-md.svg",
            ShareIconKind::Json => "share-json.svg",
            ShareIconKind::Pdf => "share-pdf.svg",
            ShareIconKind::Toml => "share-toml.svg",
            ShareIconKind::Rs => "share-rs.svg",
            ShareIconKind::Swift => "share-swift.svg",
            ShareIconKind::Hevc => "share-hevc.svg",
            ShareIconKind::File => "share-file.svg",
        }
    }

    pub fn color(self) -> ColorU {
        match self {
            ShareIconKind::Folder => theme::accent(),
            ShareIconKind::Pdf | ShareIconKind::Hevc => theme::accent(),
            ShareIconKind::Toml | ShareIconKind::File => theme::muted(),
            ShareIconKind::Txt
            | ShareIconKind::Md
            | ShareIconKind::Json
            | ShareIconKind::Rs
            | ShareIconKind::Swift => theme::accent_cool(),
        }
    }
}

pub fn share_file_icon(name: &str, is_dir: bool) -> Box<dyn Element> {
    let kind = share_icon_kind(name, is_dir);
    icon(kind.asset_path(), SHARE_ICON_SIZE, kind.color())
}

pub fn share_nav_icon(back: bool, color: ColorU) -> Box<dyn Element> {
    let path = if back {
        "share-nav-back.svg"
    } else {
        "share-nav-forward.svg"
    };
    icon(path, SHARE_NAV_ICON_SIZE, color)
}

pub fn share_sync_icon(syncing: bool) -> Box<dyn Element> {
    let color = if syncing {
        theme::muted()
    } else {
        theme::accent_cool()
    };
    icon("share-sync.svg", 12.0, color)
}

pub fn cluster_refresh_icon(spinning: bool, color: ColorU, opacity: f32) -> Box<dyn Element> {
    if spinning {
        spinning_icon::cluster_refresh_spin_icon(color, opacity, CLUSTER_REFRESH_ICON_SIZE)
    } else {
        icon(CLUSTER_REFRESH_ICON_PATH, CLUSTER_REFRESH_ICON_SIZE, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_kind_from_os_label() {
        assert_eq!(device_icon_kind("Windows 11"), DeviceIconKind::Pc);
        assert_eq!(device_icon_kind("iOS 18"), DeviceIconKind::Phone);
        assert_eq!(device_icon_kind("iPadOS 18"), DeviceIconKind::Tablet);
    }

    #[test]
    fn share_icon_kind_by_extension() {
        assert_eq!(share_icon_kind("readme.md", false), ShareIconKind::Md);
        assert_eq!(share_icon_kind("config.json", false), ShareIconKind::Json);
        assert_eq!(share_icon_kind("doc.pdf", false), ShareIconKind::Pdf);
        assert_eq!(share_icon_kind("Projects", true), ShareIconKind::Folder);
        assert_eq!(share_icon_kind("unknown.xyz", false), ShareIconKind::File);
    }

    #[test]
    fn share_icon_colors_match_html() {
        assert_eq!(ShareIconKind::Pdf.color(), theme::accent());
        assert_eq!(ShareIconKind::Json.color(), theme::accent_cool());
        assert_eq!(ShareIconKind::File.color(), theme::muted());
    }
}
