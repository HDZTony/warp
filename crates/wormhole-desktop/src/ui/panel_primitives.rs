use warpui::elements::{Border, ConstrainedBox, Container, CornerRadius, Flex, Radius};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::theme;
use crate::ui_text;

/// Matches `desktop-current.html` `--panel-pad` / `--radius`.
pub const SECTION_PADDING: f32 = 14.0;
pub const SECTION_GAP: f32 = 8.0;
pub const HUD_RADIUS: f32 = 2.0;
/// `.agent-row-item` / `.agent-search-wrap` in `desktop-current.html`.
pub const AGENT_ROW_RADIUS: f32 = 10.0;
/// `.agent-sidebar-icon-btn` border-radius.
pub const AGENT_ICON_BTN_RADIUS: f32 = 7.0;
/// `.agent-composer` max content width.
pub const AGENT_THREAD_MAX_WIDTH: f32 = 720.0;
/// `.agent-thread` bottom padding (composer overlay clearance).
pub const AGENT_THREAD_BOTTOM_PAD: f32 = 120.0;

pub fn truncate_middle(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let head = max_chars / 2;
    let tail = max_chars - head - 1;
    let chars: Vec<char> = text.chars().collect();
    let start: String = chars.iter().take(head).collect();
    let end: String = chars.iter().skip(char_count - tail).collect();
    format!("{start}…{end}")
}

pub fn section_card(inner: Box<dyn Element>) -> Box<dyn Element> {
    Container::new(inner)
        .with_background(theme::bg())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .with_uniform_padding(SECTION_PADDING)
        .finish()
}

pub fn section_title(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    ui_title(text, font)
}

/// Matches `.ui-title` in `desktop-current.html` (mono HUD heading).
pub fn ui_title(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    let label = text.into().to_uppercase();
    ui_text::hud_title(label, font)
        .with_color(theme::accent_cool())
        .finish()
}

/// Matches `.agent-sidebar-head-label` — sentence case, muted.
pub fn agent_sidebar_label(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    ui_text::body(text, font)
        .with_color(theme::muted())
        .finish()
}

/// `color-mix(in srgb, var(--panel) 38%, var(--canvas))`.
pub fn agent_sidebar_bg() -> pathfinder_color::ColorU {
    pathfinder_color::ColorU::new(21, 19, 27, 255)
}

/// `.agent-header` background: 55% panel + 45% canvas.
pub fn agent_header_bg() -> pathfinder_color::ColorU {
    pathfinder_color::ColorU::new(27, 25, 35, 255)
}

/// `.agent-row-item.active` background: 14% accent-cool + panel.
pub fn agent_row_active_bg() -> pathfinder_color::ColorU {
    pathfinder_color::ColorU::new(67, 66, 81, 255)
}

/// `.conv-device-status .dot` — online indicator.
pub fn online_dot() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_width(5.0)
            .with_height(5.0)
            .finish(),
    )
    .with_background(theme::success())
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
    .finish()
}

pub fn section_hint(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    ui_text::body(text, font)
        .with_color(theme::muted())
        .finish()
}

pub fn status_line(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
    tone: StatusTone,
) -> Box<dyn Element> {
    let color = match tone {
        StatusTone::Neutral => theme::text(),
        StatusTone::Muted => theme::muted(),
        StatusTone::Placeholder => theme::placeholder(),
        StatusTone::Success => theme::success(),
        StatusTone::Warn => theme::warn(),
        StatusTone::Danger => theme::danger(),
    };
    ui_text::device_meta(text, font).with_color(color).finish()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Neutral,
    Muted,
    Placeholder,
    Success,
    Warn,
    Danger,
}

#[cfg(test)]
mod tests {
    use super::truncate_middle;

    #[test]
    fn truncate_middle_short_unchanged() {
        assert_eq!(truncate_middle("abc", 10), "abc");
    }

    #[test]
    fn truncate_middle_long_path() {
        let path = "/very/long/path/to/some/deeply/nested/wormhole/data/directory";
        let out = truncate_middle(path, 24);
        assert!(out.contains('…'));
        assert!(out.chars().count() <= 24);
    }
}
